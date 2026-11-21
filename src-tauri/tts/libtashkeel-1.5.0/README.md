# Bundled Libtashkeel model

`model.onnx` is an **unchanged artifact**, not a vendored Rust package. It is
copied from `data/ort/model.onnx` in the published `libtashkeel_base` 1.5.0
crate. The application uses the official crate with its built-in ORT feature
disabled and supplies inference through its public interface.

- Published package checksum:
  `b1094c84063245a4460e710c8509be0cac055bac1043932f525318156db4803c`
- Model size: **4,788,213 bytes**.
- Model SHA-256:
  `52c40d42ca094ff9ff07a091d0a1eafd7baa5cf2388e60a9cc89bacd8e5e955f`.
- Source: https://crates.io/crates/libtashkeel_base/1.5.0
- Upstream license: https://github.com/mush42/libtashkeel/blob/main/LICENSE

`build.rs` verifies the model size and checksum for preprocessing-enabled
builds. `include_bytes!` puts it in the native binary/library, so installed,
offline and mobile builds need neither a Cargo cache nor an extra download or
resource-path lookup. Builds without preprocessing do not embed it. Its size
replaces the same asset previously embedded by the upstream ORT backend; it
does not add a second model copy to the executable.

The original MIT notice is retained in `LICENSE` and packaged as
`licenses/libtashkeel-1.5.0.txt`. No upstream Rust source or maps are copied here.

## Updating

Use normal Cargo dependency updates for upstream Rust fixes. If the model or
its maps change, treat them as a matched release: obtain the artifact from the
checksum-verified published package, replace this asset and provenance,
update the build checksum and versioned preprocessing ID, then run the native
preprocessing/speech checks and the shared-runtime check. Changing synthesis
behavior needs a new preprocessing ID to avoid reusing older generated audio.
An ORT-only update needs compatibility validation, not a copied upstream fork.

Linux shared builds and the Linux check of the linked-initializer code path
are validated separately; those do not establish an iOS device/static-archive
or another desktop platform result.
