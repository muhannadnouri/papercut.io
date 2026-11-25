"""Regenerate the synthetic DS-002 bundle with Python's standard library."""
import io
import json
from pathlib import Path
import wave

title = "Lantern Field Notes"
passage = "The blue lantern marks the northern footbridge."
html = f"<html><head><title>{title}</title></head><body><h1>{title}</h1><p>{passage}</p></body></html>".encode()
audio = io.BytesIO()
with wave.open(audio, "wb") as wav:
    wav.setparams((1, 2, 8000, 0, "NONE", "not compressed"))
    wav.writeframes(bytes(1600))  # 100 ms of silence, usable offline on both platforms.

payload = bytearray()
files = []
for name, role, mime, data in [
    ("source.html", "sourceHtml", "text/html", html),
    ("chunks/0.wav", "chunkWav", "audio/wav", audio.getvalue()),
    ("audio.wav", "singleTrackWav", "audio/wav", audio.getvalue()),
]:
    entry = dict(path=name, role=role, contentType=mime, bytes=len(data), payloadOffset=len(payload))
    if role == "chunkWav":
        entry["chunkIndex"] = 0
    files.append(entry)
    payload.extend(data)

manifest = dict(
    version=2, kind="papercut-audiobook-bundle", sourceDocumentUrl="/ds-002.html",
    title=title, modelId="sherpa-onnx/kokoro-multi-lang-v1_0", voice="af_heart",
    speed=1, dtype="native", cacheVersion="native-save-v4-segmented",
    chunks=[dict(id="chunk-1", text=passage)], files=files,
    audio=dict(format="wav", singleTrack=True, durationSec=0.1, bytes=len(audio.getvalue())),
)
header = json.dumps(manifest, separators=(",", ":")).encode()
Path(__file__).with_name(title + ".papercut-audiobook").write_bytes(
    b"PAPERCUT_AUDIOBOOK_BUNDLE_V2\n" + len(header).to_bytes(8, "little") + header + payload
)
