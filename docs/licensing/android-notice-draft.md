# Android combined-distribution notice — DRAFT FOR OWNER REVIEW

Status: draft 2026-11-03. Not shipped anywhere yet; the repository root
LICENSE.md remains MIT. Decisions recorded in
[the Android guide](../android-production-packaging.md): the F-Droid metadata
declares GPL-3.0-only for the combined Android build, Kareem is kept with
disclosure, and Supertonic is kept with a NonFreeAssets anti-feature. License
texts themselves are copied from pinned sources at bundle-assembly time and are
not duplicated here.

## Combined work

This Android application incorporates eSpeak NG and Unicode tools through
Sherpa ONNX/Piper phonemization. These components carry GPLv3 terms, with
later-version permission where granted. The combined distribution is therefore
declared GPL-3.0-only in F-Droid metadata and must satisfy applicable GPL
terms. Papercut's original source retains MIT notices; other dependencies
retain their notices. Matching corresponding source and build instructions
accompany each release.

## Papercut original code

MIT. The root LICENSE.md and per-file headers remain unchanged; the GPL
declaration covers the combined Android distribution, not a license change of
the original source.

## Speech engine: Sherpa ONNX 1.13.4, Piper phonemization, eSpeak NG

- eSpeak NG: GPLv3 (later-version grants in source headers), including the
  Unicode character database (ucd) data it incorporates; notices and exact
  pinned source accompany each release.
- Piper phonemization: MIT, with uni-algo notices.
- Sherpa ONNX: Apache-2.0 project glue; its statically incorporated eSpeak/Piper
  archives are pinned (upstream `espeak-ng-for-piper.cmake`,
  `piper-phonemize.cmake`) and included in corresponding source.

## Native inference and libraries

- ONNX Runtime 1.27.0: Apache-2.0, distributed from the pinned Maven Central
  artifact plus its ThirdPartyNotices and cmake/deps.txt dependency listing.
- Sherpa's fetched native subprojects: Eigen (MPL2 with per-file
  BSD/Apache/MINPACK notices), OpenFst and Kaldi (Apache-2.0), KissFFT (BSD),
  nlohmann JSON (MIT), sentencepiece notices.
- CameraX camera-core: Apache-2.0 with libyuv BSD notices.
- The two local Rust plugins inherit the repository's original-code MIT scope.

## OCR

Tesseract: Apache-2.0. Leptonica: BSD-style. IJG JPEG, giflib MIT,
libpng/libtiff/libwebp notices, zlib, and openlibm's MIT/BSD/ISC/FDLIBM/
public-domain terms with its LGPL test notices. English and Arabic trained
data byte-match the pinned naptha/tessdata revision and carry its Apache-2.0
source license, stated separately from the npm wrappers' MIT metadata.

## PDF

PDF.js and the JBIG2/CCITT wrapper: Apache-2.0. OpenJPEG: BSD. PDFium:
BSD-3-Clause (its actual source license, notwithstanding the wrapper README's
Apache link). QuickJS: MIT. QCMS: the pinned source is MIT while the published
PDF.js notice is BSD-style; both evidence sets are retained until the
discrepancy is reconciled at asset staging.

## Rust, JavaScript and Maven dependencies

Per-package copyright and license texts from the pinned packages, including
MPL-covered source files, Unicode/CDLA data terms and Apache notices. The
resolved graph is regenerated with the standard refresh commands recorded in
the source inventory.

## Fonts and document data

Existing OFL/Apache font notices; PDF standard-font Foxit/Liberation notices.

## Models and voices

- E5 embeddings (Small/Base/Large): all pinned model cards declare MIT;
  hash-pinned ONNX/tokenizer assets keep per-file notices.
- Libtashkeel embedded model: unchanged model file with checksum and MIT
  provenance; its notice ships in APK assets today.
- Piper Kareem (Arabic voice): **disclosure** — the pinned archive's model card
  points dataset licensing to AliMokhammad/arabicttstrain, which cannot be
  retrieved, and records fine-tuning from the lessac voice; repository MIT
  metadata does not settle these rights. Kept as an optional download with this
  status disclosed in the listing, pending an upstream rights statement.
- Supertonic 3: the pinned archive distinguishes MIT sample code from
  OpenRAIL-M weights that carry use restrictions. Kept as an optional download
  and declared with the F-Droid NonFreeAssets anti-feature.
- Kokoro: Apache-2.0 archive; its bundled eSpeak data/dictionaries and
  CppJieba dictionaries are being mapped into the GPL and notice obligations
  of this bundle.

## Anti-features and corresponding source

The F-Droid listing declares the NonFreeAssets anti-feature for restricted
model weights. Each release ships the exact pinned source revisions, patches,
build scripts and license texts as a corresponding-source archive with build
instructions, so the GPL terms of the combined distribution are satisfied.
