# Kokoro v1.0 — artifact revision 2026-09-08

This is a model-data release for Papercut, not an application update or a
Kokoro v1.1 upgrade. Do not mark it as the latest application release.

- Upstream: https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2
- Upstream asset ID: `549865473`, uploaded September 8, 2026.
- Archive: `kokoro-multi-lang-v1_0.tar.bz2`, **349,906,910 bytes**.
- SHA-256: `c5f7e2d2caf082bc1d20fb70334a61d99d20b484500aad32e7cf84c128ea3298`.
- The archive is mirrored byte-for-byte, including its upstream license and notices.
- The upstream README describes 54 voices: Spanish `em_santa` was appended at
  ID 53; speaker IDs 0–52 remain unchanged. Papercut retains its existing voices.
- The preparation workflow verifies the archive and runs Papercut's native
  Sherpa 1.13.4 loader and synthesis checks for all seven Kokoro language entries.

The attached manifest records the download metadata at validation time.
Keep this release immutable and retain it for supported clients. Future
artifact revisions require a new release tag; never replace these bytes.
