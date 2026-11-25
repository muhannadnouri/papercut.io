"""Regenerate the offline desktop playback fixtures using only the standard library."""
import io
import json
from pathlib import Path
import wave

sample_rate = 8000


def wav(seconds):
    buf = io.BytesIO()
    with wave.open(buf, "wb") as audio:
        audio.setparams((1, 2, sample_rate, 0, "NONE", "not compressed"))
        audio.writeframes(bytes(seconds * sample_rate * 2))
    return buf.getvalue()


def write_bundle(title, source_url, passages, durations, source_paragraphs):
    payload = bytearray()
    files = []

    def append_file(path, role, mime, data, **metadata):
        files.append(dict(path=path, role=role, contentType=mime, bytes=len(data),
                          payloadOffset=len(payload), **metadata))
        payload.extend(data)

    html = "<html><head><title>" + title + "</title></head><body>" + "".join(f"<p>{p}</p>" for p in source_paragraphs) + "</body></html>"
    append_file("source.html", "sourceHtml", "text/html", html.encode())
    chunks = []
    for index, (text, seconds) in enumerate(zip(passages, durations)):
        chunks.append(dict(id=f"chunk-{index + 1}", text=text, sourceSpan=dict(
            startSegmentIndex=index, endSegmentIndex=index, startOffset=0, endOffset=len(text))))
        append_file(f"chunks/{index}.wav", "chunkWav", "audio/wav", wav(seconds), chunkIndex=index)
    track = wav(sum(durations))
    append_file("audio.wav", "singleTrackWav", "audio/wav", track)
    manifest = dict(version=2, kind="papercut-audiobook-bundle", sourceDocumentUrl=source_url,
                    title=title, modelId="sherpa-onnx/kokoro-multi-lang-v1_0", voice="af_heart",
                    speed=1, dtype="native", cacheVersion="native-save-v4-segmented",
                    chunks=chunks, files=files, audio=dict(format="wav", singleTrack=True,
                    durationSec=sum(durations), bytes=len(track)))
    metadata = {key: manifest[key] for key in ("title", "modelId", "voice", "speed", "dtype", "chunks", "audio")}
    append_file("metadata.json", "metadata", "application/json", json.dumps(metadata).encode())
    header = json.dumps(manifest, separators=(",", ":")).encode()
    Path(__file__).with_name(title + ".papercut-audiobook").write_bytes(
        b"PAPERCUT_AUDIOBOOK_BUNDLE_V2\n" + len(header).to_bytes(8, "little") + header + payload)


passages = ["A ferry crosses the outer harbor.", "Two gulls circle the lighthouse.", "Amber beacons guide the pilot home."]
write_bundle("Harbor Signals", "/ds-006.html", passages, [18, 6, 12], passages)

# Model a newer source edition with a revised closing note. The opening audio and span still match.
notes = ["The harbor office opens at dawn.", "Original printing by the port authority."]
write_bundle("Harbor Office Notes", "/ds-014.html", notes, [12, 2], [notes[0], "Printed by the harbor office."])
