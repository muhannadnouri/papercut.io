# SILMA Worker

Desktop-only Stage 0 worker for the SILMA TTS sidecar spike.

This is not wired into Tauri yet. It is a standalone JSONL process used to prove
that the official Python SILMA runtime can load once and write chunk WAV files.

## Setup

Use Python 3.10+.

```bash
python -m venv .venv-silma
. .venv-silma/bin/activate
pip install -r sidecars/silma/requirements.txt
```

SILMA requires `ffmpeg` to be available on the system path for the official
runtime path documented by SILMA.

## Self-Test

This does not import or download SILMA.

```bash
python sidecars/silma/silma_worker.py --self-test
```

## Manual Smoke

This imports SILMA, loads the model through the official `SilmaTTS` API, and
writes one WAV. The first run may download model files into the provided cache
directory.

```bash
python sidecars/silma/silma_worker.py <<'JSONL'
{"id":"1","op":"health"}
{"id":"2","op":"load_model","model_dir":"./.cache/silma-tts"}
{"id":"3","op":"synthesize","text":"أنا نموذج سلمى لتحويل النص إلى كلام.","output_wav":"./.cache/silma-tts-smoke.wav","speed":1.0,"seed":1234}
{"id":"4","op":"shutdown"}
JSONL
```

The worker writes protocol responses to stdout and runtime logs to stderr.
