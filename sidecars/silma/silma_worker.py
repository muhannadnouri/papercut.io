#!/usr/bin/env python3
"""JSONL worker for SILMA TTS desktop sidecar experiments.

Stdout is protocol only. Logs go to stderr.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import tempfile
import time
import wave
from dataclasses import dataclass
from pathlib import Path
from typing import Any

VERSION = "0.1.0"
DEFAULT_REF_TEXT = (
    "ويدقق النظر في القرآن الكريم وسائر الكتب السماوية ويتبع مسالك الرسل "
    "العظام عليهم الصلاة والسلام."
)
DEFAULT_SMOKE_TEXT = "أنا نموذج سلمى لتحويل النص إلى كلام."


@dataclass
class LoadedModel:
    engine: Any
    model_dir: str | None
    sample_rate: int


class Worker:
    def __init__(self) -> None:
        self.loaded: LoadedModel | None = None

    def handle(self, request: dict[str, Any]) -> dict[str, Any]:
        """Dispatch one JSONL protocol request without letting exceptions escape."""
        request_id = str(request.get("id", ""))
        try:
            op = request["op"]
            if op == "health":
                return self.ok(request_id, version=VERSION, loaded=self.loaded is not None)
            if op == "load_model":
                return self.load_model(request_id, request)
            if op == "write_probe_wav":
                return self.write_probe_wav(request_id, request)
            if op == "synthesize":
                return self.synthesize(request_id, request)
            if op == "shutdown":
                return self.ok(request_id, shutdown=True)
            return self.err(request_id, f"unsupported op: {op!r}")
        except Exception as exc:  # noqa: BLE001 - protocol boundary must not crash on bad input.
            return self.err(request_id, str(exc))

    def load_model(self, request_id: str, request: dict[str, Any]) -> dict[str, Any]:
        """Load the official SILMA runtime once, optionally using an app-owned HF cache."""
        model_dir = request.get("model_dir")
        model_dir_str = str(model_dir) if model_dir else None

        SilmaTTS = import_silma_tts()

        kwargs: dict[str, Any] = {}
        if model_dir_str:
            kwargs["hf_cache_dir"] = model_dir_str
        if "enable_normalizer" in request:
            kwargs["enable_normalizer"] = bool(request["enable_normalizer"])
        if "force_tashkeel" in request:
            kwargs["force_tashkeel"] = bool(request["force_tashkeel"])

        started = time.perf_counter()
        engine = SilmaTTS(**kwargs)
        load_ms = round((time.perf_counter() - started) * 1000)
        sample_rate = int(getattr(engine, "target_sample_rate", 24000))
        self.loaded = LoadedModel(engine=engine, model_dir=model_dir_str, sample_rate=sample_rate)
        return self.ok(
            request_id,
            model_dir=model_dir_str,
            sample_rate=sample_rate,
            load_ms=load_ms,
        )

    def synthesize(self, request_id: str, request: dict[str, Any]) -> dict[str, Any]:
        """Run one SILMA inference and write the WAV exactly where Rust asked."""
        if self.loaded is None:
            raise ValueError("model is not loaded")

        text = required_str(request, "text").strip()
        output_wav = Path(required_str(request, "output_wav"))
        output_wav.parent.mkdir(parents=True, exist_ok=True)
        ref_file = request.get("ref_file")
        ref_text = request.get("ref_text", DEFAULT_REF_TEXT)

        if not ref_file:
            from importlib.resources import files

            ref_file = str(files("silma_tts").joinpath("infer/ref_audio_samples/ar.ref.24k.wav"))

        started = time.perf_counter()
        _wav, sr, _spec = self.loaded.engine.infer(
            ref_file=str(ref_file),
            ref_text=str(ref_text),
            gen_text=text,
            file_wave=str(output_wav),
            seed=request.get("seed"),
            speed=float(request.get("speed", 1.0)),
            normalize_numbers=bool(request.get("normalize_numbers", True)),
            force_tashkeel=bool(request.get("force_tashkeel", True)),
        )
        synthesis_ms = round((time.perf_counter() - started) * 1000)
        info = wav_info(output_wav)
        return self.ok(
            request_id,
            sample_rate=int(sr or info["sample_rate"]),
            audio_duration_sec=info["audio_duration_sec"],
            wav_bytes=info["wav_bytes"],
            synthesis_ms=synthesis_ms,
        )

    def write_probe_wav(self, request_id: str, request: dict[str, Any]) -> dict[str, Any]:
        """Write a tiny silent WAV so Tauri can test sidecar file access before model work."""
        output_wav = Path(required_str(request, "output_wav"))
        sample_rate = int(request.get("sample_rate", 24000))
        duration_sec = float(request.get("duration_sec", 0.25))
        frame_count = max(1, round(sample_rate * duration_sec))
        output_wav.parent.mkdir(parents=True, exist_ok=True)
        with wave.open(str(output_wav), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(sample_rate)
            wav.writeframes(b"\0\0" * frame_count)
        info = wav_info(output_wav)
        return self.ok(
            request_id,
            sample_rate=info["sample_rate"],
            audio_duration_sec=info["audio_duration_sec"],
            wav_bytes=info["wav_bytes"],
        )

    @staticmethod
    def ok(request_id: str, **fields: Any) -> dict[str, Any]:
        return {"id": request_id, "ok": True, **fields}

    @staticmethod
    def err(request_id: str, error: str) -> dict[str, Any]:
        return {"id": request_id, "ok": False, "error": error}


def required_str(request: dict[str, Any], key: str) -> str:
    """Read a required string field and return a protocol-friendly error if absent."""
    value = request.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"missing string field: {key}")
    return value


def import_silma_tts() -> Any:
    """Import SILMA with an actionable setup hint when the sidecar venv is missing."""
    try:
        from silma_tts.api import SilmaTTS
    except ModuleNotFoundError as exc:
        if exc.name == "silma_tts":
            raise RuntimeError(
                "SILMA Python package is not installed for this interpreter. "
                "Run: python3 -m venv .venv-silma && . .venv-silma/bin/activate && "
                "pip install -r sidecars/silma/requirements.txt"
            ) from exc
        raise
    return SilmaTTS


def wav_info(path: Path) -> dict[str, Any]:
    """Return the small WAV facts Rust expects from synthesis responses."""
    with wave.open(str(path), "rb") as wav:
        frames = wav.getnframes()
        sample_rate = wav.getframerate()
        channels = wav.getnchannels()
    return {
        "sample_rate": sample_rate,
        "channels": channels,
        "audio_duration_sec": frames / sample_rate if sample_rate else 0,
        "wav_bytes": path.stat().st_size,
    }


def run_jsonl() -> int:
    """Run the stdin/stdout JSONL loop used by Rust's sidecar supervisor."""
    worker = Worker()
    for line in sys.stdin:
        try:
            request = json.loads(line)
            if not isinstance(request, dict):
                raise ValueError("request must be a JSON object")
            response = worker.handle(request)
        except Exception as exc:  # noqa: BLE001 - malformed protocol input still returns JSON.
            response = {"id": "", "ok": False, "error": str(exc)}
        print(json.dumps(response, ensure_ascii=False), flush=True)
        if response.get("ok") and response.get("shutdown"):
            return 0
    return 0


def run_self_test() -> int:
    """Exercise protocol behavior that does not require installing or loading SILMA."""
    worker = Worker()
    assert worker.handle({"id": "1", "op": "health"})["ok"] is True
    probe_path = Path(tempfile.gettempdir()) / "papercut-silma-worker-probe.wav"
    probe = worker.handle({"id": "2", "op": "write_probe_wav", "output_wav": str(probe_path)})
    assert probe["ok"] is True and probe["wav_bytes"] > 44
    probe_path.unlink(missing_ok=True)
    missing = worker.handle({"id": "3", "op": "synthesize", "text": "x", "output_wav": "x.wav"})
    assert missing["ok"] is False and "not loaded" in missing["error"]
    unknown = worker.handle({"id": "4", "op": "wat"})
    assert unknown["ok"] is False and "unsupported op" in unknown["error"]
    if importlib.util.find_spec("silma_tts") is None:
        load = worker.handle({"id": "5", "op": "load_model"})
        assert load["ok"] is False and "pip install -r sidecars/silma/requirements.txt" in load["error"]
    print("silma_worker self-test passed")
    return 0


def run_smoke(args: argparse.Namespace) -> int:
    """Load SILMA once, synthesize one WAV, and print a machine-readable summary."""
    worker = Worker()
    load_request: dict[str, Any] = {
        "id": "load",
        "op": "load_model",
        "model_dir": args.model_dir,
    }
    if args.disable_normalizer:
        load_request["enable_normalizer"] = False
    if args.disable_tashkeel:
        load_request["force_tashkeel"] = False

    synth_request: dict[str, Any] = {
        "id": "synthesize",
        "op": "synthesize",
        "text": args.text,
        "output_wav": args.output_wav,
        "speed": args.speed,
        "seed": args.seed,
        "normalize_numbers": not args.disable_normalizer,
        "force_tashkeel": not args.disable_tashkeel,
    }
    if args.ref_file:
        synth_request["ref_file"] = args.ref_file
    if args.ref_text:
        synth_request["ref_text"] = args.ref_text

    started = time.perf_counter()
    load = worker.handle(load_request)
    if not load.get("ok"):
        print(json.dumps({"ok": False, "stage": "load_model", "response": load}, ensure_ascii=False))
        return 1

    synth = worker.handle(synth_request)
    if not synth.get("ok"):
        print(json.dumps({"ok": False, "stage": "synthesize", "load": load, "response": synth}, ensure_ascii=False))
        return 1

    total_ms = round((time.perf_counter() - started) * 1000)
    audio_duration = float(synth.get("audio_duration_sec") or 0)
    synthesis_ms = int(synth.get("synthesis_ms") or 0)
    summary = {
        "ok": True,
        "version": VERSION,
        "model_dir": args.model_dir,
        "output_wav": args.output_wav,
        "load": load,
        "synthesis": synth,
        "total_ms": total_ms,
        "real_time_factor": synthesis_ms / (audio_duration * 1000) if audio_duration > 0 else None,
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


def main() -> int:
    """CLI entrypoint for either the lightweight self-test or worker mode."""
    parser = argparse.ArgumentParser(description="SILMA TTS JSONL sidecar worker")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--smoke", action="store_true", help="load SILMA and synthesize one WAV")
    parser.add_argument("--model-dir", default="./.cache/silma-tts")
    parser.add_argument("--output-wav", default="./.cache/silma-tts-smoke.wav")
    parser.add_argument("--text", default=DEFAULT_SMOKE_TEXT)
    parser.add_argument("--ref-file")
    parser.add_argument("--ref-text")
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--seed", type=int, default=1234)
    parser.add_argument("--disable-normalizer", action="store_true")
    parser.add_argument("--disable-tashkeel", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return run_self_test()
    if args.smoke:
        return run_smoke(args)
    return run_jsonl()


if __name__ == "__main__":
    raise SystemExit(main())
