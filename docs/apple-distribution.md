# Apple Distribution Runbook

This file records what Papercut still needs before macOS releases stop showing Gatekeeper warnings and before CI can produce an iOS App Store build.

## Current Branch Audit

- Branch: `feature/macos-build`.
- Desktop CI already builds Linux, Windows, macOS Intel, and macOS Apple Silicon in `.github/workflows/ci.yml` and `.github/workflows/release.yml`.
- macOS release output now has proven CI plumbing for Developer ID signing, notarization, DMG stapling, and Gatekeeper verification.
- README, TTS docs, and site install notes now distinguish official signed/notarized releases from unsigned local/PR development artifacts.
- `src-tauri/tauri.macos.conf.json` stages native TTS dylibs and enables hardened runtime with `src-tauri/Entitlements.plist`.
- App bundle includes native dylibs in `Contents/Resources`: `libsherpa-onnx-c-api.dylib`, `libonnxruntime.dylib`, versioned ONNX Runtime dylibs such as `libonnxruntime.1.24.4.dylib`, and optionally `libsherpa-onnx-cxx-api.dylib`. The macOS copy helper signs these dylibs with the Developer ID identity and secure timestamp when `APPLE_SIGNING_IDENTITY` is present, and release CI now fails if required dylibs are missing or unsigned.
- Android CI currently creates a debug APK. That is unrelated to Apple work, but it is not a production Android release signing path.
- `src-tauri/gen/apple` has been generated and committed. Current iOS CI/release builds run it on GitHub `macos-26` runners so App Store uploads use the iOS 26 SDK or newer. The generated iOS target uses App Store display name `Papercut Offline`, Bundle ID `io.papercut.app`, the Papercut iOS app icons, background audio mode, and standard/non-exempt encryption set to false for the HTTPS-only model-download path.
- No certificate, key, provisioning profile, `.p12`, `.p8`, `.cer`, `.csr`, or keystore files are committed.
- Working tree had unrelated user changes when this audit was written. Do not mix Apple distribution work with those changes.
- Tags `v1.2.1` through `v1.2.6` were macOS release-pipeline validation tags, not product releases. `v1.3.0` was the first macOS release target, and `v1.3.3` supersedes the earlier macOS patch attempts because it bundles the complete dylib dependency closure and signs bundled dylibs for notarization.

## v1.3.3 Patch Release Guidance

Use `branch-release-v1.3.3` for the macOS dylib packaging patch release. Do not reuse the v1.3.0, v1.3.1, or v1.3.2 tags if they were already published or downloaded; ship v1.3.3 so users and GitHub Release assets have a clean immutable version.

Before publishing v1.3.3, confirm both macOS release jobs pass the `Verify macOS bundled runtime libraries` step. That step runs `scripts/verify-macos-bundle-libs.js`, checks required dylibs, checks the versioned `libonnxruntime.*.dylib` dependency, walks `otool -L` dependencies for the app and every bundled dylib, and verifies dylib signatures in the protected release job before DMG notarization/upload. After release upload, smoke-test the Apple Silicon DMG on MacInCloud by dragging `Papercut.app` out of the mounted DMG and launching it from Terminal.

## Sources

- Apple Developer ID certificates: https://developer.apple.com/help/account/certificates/create-developer-id-certificates/
- Apple CSR instructions: https://developer.apple.com/help/account/certificates/create-a-certificate-signing-request/
- Apple Developer ID / Gatekeeper / notarization overview: https://developer.apple.com/developer-id/
- App Store Connect API keys: https://developer.apple.com/help/app-store-connect/get-started/app-store-connect-api/
- Tauri macOS signing: https://v2.tauri.app/distribute/sign/macos/
- Tauri iOS signing: https://v2.tauri.app/distribute/sign/ios/
- Tauri App Store distribution: https://v2.tauri.app/distribute/app-store/

## Best Path

Use GitHub Releases for Linux, Windows, Android, and macOS. Use Apple App Store / TestFlight for iOS.

For macOS outside App Store, use:

1. Developer ID Application certificate.
2. Hardened runtime and correct entitlements.
3. Tauri/codesign signing on both macOS CI jobs.
4. Apple notarization.
5. Stapled notarization ticket on `.app` / `.dmg`.

For iOS, keep two validation gates:

1. PR-safe simulator build: no Apple secrets, no App Store upload, but compile the generated Tauri Apple project with the native TTS simulator static-library slice.
2. Signed/TestFlight native build: use the protected `apple-release` environment, App Store Connect provisioning, the verified sherpa-onnx iOS static archive, conservative iOS thread defaults, and real-device TestFlight validation before App Review. Use static linking/XCFramework-style archives instead of macOS-style dylib resource copying.

## macOS: Apple Work Before Repo Changes

### 1. Confirm Apple account access

Need Apple Developer Program membership active.

Need Account Holder role for Developer ID certificate creation. Apple allows up to five Developer ID Application certificates and five Developer ID Installer certificates.

### 2. Decide certificate type

Choose `Developer ID Application`.

Choose `G2 Sub-CA (Xcode 11.4.1 or later)`.

Do not choose `Previous Sub-CA` unless you must support ancient Xcode/macOS signing flows. Not needed for this project.

Developer ID Installer certificate is optional. Papercut ships `.dmg`, not signed `.pkg`, so start with Developer ID Application only.

### 3. Generate CSR without owning a Mac

Apple docs describe Keychain Access on Mac, but CSR is a standard certificate signing request. You can generate it with OpenSSL on Linux. Protect the private key; the downloaded Apple `.cer` is useless without it.

If Apple rejects this CSR for any reason, use a one-time GitHub Actions macOS workflow or a borrowed/trusted Mac to generate the CSR and immediately export the resulting `.p12`. Routine release builds can still happen on GitHub-hosted macOS runners; you do not need to own a Mac for every release.

Run outside repo, for example in a private temp folder:

```bash
mkdir -p ~/private-apple-signing/papercut-macos
cd ~/private-apple-signing/papercut-macos

openssl genrsa -out developer-id-application.key 2048

openssl req -new \
  -key developer-id-application.key \
  -out developer-id-application.certSigningRequest \
  -subj "/emailAddress=YOUR_APPLE_ID_EMAIL/CN=Papercut Developer ID Application/C=CA"
```

Upload `developer-id-application.certSigningRequest` in Apple Developer `Certificates, Identifiers & Profiles`.

Download Apple result, usually `developerID_application.cer`.

Convert and export a CI-ready `.p12`:

```bash
openssl x509 -inform DER \
  -in developerID_application.cer \
  -out developer-id-application.pem

openssl pkcs12 -export \
  -inkey developer-id-application.key \
  -in developer-id-application.pem \
  -out papercut-developer-id-application.p12 \
  -name "Developer ID Application: YOUR_NAME_OR_ORG (TEAMID)" \
  -keypbe PBE-SHA1-3DES \
  -certpbe PBE-SHA1-3DES \
  -macalg sha1
```

Use a strong unique export password. Store it in password manager.

Keep these offline:

- `developer-id-application.key`
- `papercut-developer-id-application.p12`
- `.p12` password

Do not commit them. Do not upload them as workflow artifacts.

### 4. Create App Store Connect API key for notarization

Prefer App Store Connect API key over Apple ID/app-specific password.

In App Store Connect:

1. Open `Users and Access`.
2. Open `Integrations`.
3. If needed, Account Holder requests API access.
4. Create a Team API key.
5. For macOS notarization, `Developer` access is normally enough.
6. Save `Issuer ID`.
7. Save `Key ID`.
8. Download the `.p8` private key once.

Store `.p8` in password manager. If lost or leaked, revoke and recreate it.

### 5. Create GitHub secrets for macOS

Recommended GitHub Environment: `apple-release`.

Protect it with required reviewer approval and restrict release workflow branches/tags.

The release workflow now has a separate `build-macos` job with `environment: apple-release`, so the macOS jobs can read environment secrets while Linux, Windows, and Android jobs cannot. Keep the environment protected with required reviewers and deployment branch/tag restrictions.

Secrets:

- `APPLE_DEVELOPER_ID_CERTIFICATE_BASE64`: base64 of `papercut-developer-id-application.p12`
- `APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD`: `.p12` export password
- `APPLE_SIGNING_IDENTITY`: exact identity, e.g. `Developer ID Application: Name (TEAMID)`. The release workflow now detects this from the imported certificate and exports it for Tauri, so this secret is mainly useful as a human reference.
- `APPLE_TEAM_ID`: Apple Team ID
- `APPLE_API_ISSUER`: App Store Connect Issuer ID
- `APPLE_API_KEY`: App Store Connect Key ID for Tauri notarization
- `APPLE_API_PRIVATE_KEY_BASE64`: base64 of `AuthKey_KEYID.p8`
- `APPLE_KEYCHAIN_PASSWORD`: random CI-only temporary keychain password

Create base64 values:

```bash
base64 -w 0 papercut-developer-id-application.p12 > developer-id.p12.base64
base64 -w 0 AuthKey_KEYID.p8 > authkey.p8.base64
```

macOS BSD base64 uses:

```bash
base64 -i papercut-developer-id-application.p12 | tr -d '\n' > developer-id.p12.base64
base64 -i AuthKey_KEYID.p8 | tr -d '\n' > authkey.p8.base64
```

Before rerunning a release, verify the `.p12` password locally with the exact password stored in `APPLE_DEVELOPER_ID_CERTIFICATE_PASSWORD`:

```bash
openssl pkcs12 \
  -in papercut-developer-id-application.p12 \
  -info \
  -noout \
  -passin pass:'YOUR_P12_EXPORT_PASSWORD'
```

If this prints `MAC verification failed`, the password is not the `.p12` export password for that file. Re-export the `.p12` or update the GitHub secret.

If OpenSSL verification passes but macOS `security import` fails with `MAC verification failed during PKCS12 import`, the `.p12` is probably using OpenSSL 3's modern PBES2/AES encryption, which OpenSSL can read but macOS Keychain import may reject. Re-export with the `PBE-SHA1-3DES` / `sha1` compatibility flags shown above, then regenerate `APPLE_DEVELOPER_ID_CERTIFICATE_BASE64`. The release workflow also converts the decoded `.p12` into this Keychain-compatible form before import.

If local verification passes but CI still fails before `security import`, regenerate `APPLE_DEVELOPER_ID_CERTIFICATE_BASE64` from the same verified `.p12` and paste it into GitHub again without quotes or extra spaces.

## macOS: Repo Work After Apple Work

Do this only after secrets exist. These repo changes are now started in this branch.

### 1. Add macOS entitlements

Added `src-tauri/Entitlements.plist`.

Likely needed for Tauri/WKWebView hardened runtime:

- `com.apple.security.cs.allow-jit`
- `com.apple.security.cs.allow-unsigned-executable-memory`

Keep entitlements minimal. Add more only if notarization or runtime testing proves need.

### 2. Add macOS bundle config

Updated `src-tauri/tauri.macos.conf.json` with:

- `bundle.macOS.signingIdentity` or use `APPLE_SIGNING_IDENTITY`.
- `bundle.macOS.entitlements`.
- `bundle.macOS.hardenedRuntime: true` if Tauri version/config supports it.
- `bundle.macOS.minimumSystemVersion` if needed.

### 3. Update release workflow

Release workflow now does this in the protected `build-macos` job for each macOS architecture:

1. Decode `.p12`.
2. Create temporary keychain.
3. Import certificate.
4. Add the temporary keychain to the user keychain search list.
5. Allow `/usr/bin/codesign` access.
6. Detect the imported `Developer ID Application` identity and export it as `APPLE_SIGNING_IDENTITY`.
7. Decode `.p8` into `$RUNNER_TEMP/private_keys/AuthKey_KEYID.p8` and export the absolute path as `APPLE_API_KEY_PATH`.
8. Export Tauri notarization env vars.
9. Run `npm run desktop`. The desktop helper pre-stages macOS dylibs before Tauri scans resources; the macOS dylib copy helper signs them with the Developer ID identity and secure timestamp; Tauri signs and notarizes the `.app` bundle.
10. Run `scripts/verify-macos-bundle-libs.js --require-signatures` to verify required dylibs, transitive `otool -L` dependencies, and `codesign --verify --strict` before DMG notarization/upload.
11. Submit each generated `.dmg` to `notarytool`, staple it, and validate the stapled ticket.
12. Verify app signatures and Gatekeeper assessment.
13. Verify DMG Gatekeeper assessment.
14. Upload signed/notarized `.dmg`.

Verification commands on macOS runner:

```bash
codesign --verify --deep --strict --verbose=2 "src-tauri/target/release/bundle/macos/Papercut.app"
spctl --assess --type execute --verbose=4 "src-tauri/target/release/bundle/macos/Papercut.app"
spctl --assess --type open --context context:primary-signature --verbose=4 src-tauri/target/release/bundle/dmg/*.dmg
```

### 4. Keep public install docs current

README, `docs/kokoro-tts.md`, and `site/index.html` now describe signed/notarized release DMGs and reserve right-click Open guidance for unsigned development artifacts. After the first signed release succeeds, update any version-specific download filenames/sizes on the site.

## iOS: Apple Work Before Repo Changes

### 1. Decide final iOS Bundle ID

Desktop identifier remains:

```text
io.papercut.desktop
```

The iOS Bundle ID is now isolated in `src-tauri/tauri.ios.conf.json` so desktop/macOS/Android identity stays stable:

```text
io.papercut.app
```

Use `io.papercut.app` for the Apple Developer App ID, App Store Connect app record, provisioning profile, and generated Xcode project.

### 2. Create App Store Connect app record

In App Store Connect:

1. Apps > add new app.
2. Platform: iOS.
3. Name: `Papercut Offline`.
4. Bundle ID: `io.papercut.app`.
5. SKU: stable internal value, e.g. `papercut-ios`.
6. User Access: as needed.

### 3. Register App ID and capabilities

In Apple Developer:

1. Identifiers > App IDs > new app.
2. Bundle ID exactly matches the iOS Tauri identifier: `io.papercut.app`.
3. Enable only capabilities needed.
4. For iOS background audiobook playback, keep Background Modes enabled with the `audio` mode in the generated Xcode project.

Do not enable CloudKit, Push, Sign in with Apple, App Groups, etc. unless product needs them.

### 4. Choose signing mode

Best first CI path: manual signing.

Manual signing is more work upfront but deterministic in CI:

- Apple Distribution certificate as `.p12`.
- App Store Connect provisioning profile as `.mobileprovision`.
- App Store Connect API key for upload.

Automatic signing can work with App Store Connect API key, but failures are harder to debug on fresh runners.

### 5. Create Apple Distribution certificate

Create CSR same way as macOS, outside repo:

```bash
mkdir -p ~/private-apple-signing/papercut-ios
cd ~/private-apple-signing/papercut-ios

openssl genrsa -out apple-distribution.key 2048

openssl req -new \
  -key apple-distribution.key \
  -out apple-distribution.certSigningRequest \
  -subj "/emailAddress=YOUR_APPLE_ID_EMAIL/CN=Papercut Offline Apple Distribution/C=CA"
```

In Apple Developer Certificates, create `Apple Distribution` certificate with that CSR.

Download `.cer`, convert/export `.p12`:

```bash
openssl x509 -inform DER \
  -in distribution.cer \
  -out apple-distribution.pem

openssl pkcs12 -export \
  -inkey apple-distribution.key \
  -in apple-distribution.pem \
  -out papercut-apple-distribution.p12 \
  -name "Apple Distribution: YOUR_NAME_OR_ORG (TEAMID)"
```

### 6. Create App Store Connect provisioning profile

In Apple Developer:

1. Profiles > add.
2. Distribution type: `App Store Connect`.
3. Select the `io.papercut.app` App ID for `Papercut Offline`.
4. Select Apple Distribution certificate.
5. Name it `Papercut Offline iOS App Store`.
6. Download `.mobileprovision`.

### 7. Create App Store Connect API key for upload

You can reuse the existing `papercut.io` App Store Connect API key if its role can upload builds. A separate key is optional if you want cleaner rotation/auditing:

- `Papercut Offline CI Upload`
- Access: `Developer` if upload works; `Admin` if using automatic signing or if Apple tooling requires it.

Save:

- Issuer ID
- Key ID
- `.p8` private key

### 8. Create GitHub secrets for iOS

In protected `apple-release` environment:

- `IOS_CERTIFICATE`: base64 of `papercut-apple-distribution.p12`
- `IOS_CERTIFICATE_PASSWORD`: `.p12` password
- `IOS_MOBILE_PROVISION`: base64 of `.mobileprovision`
- `APPLE_API_ISSUER`: App Store Connect Issuer ID
- `APPLE_API_KEY`: App Store Connect Key ID for notarization and iOS upload
- `APPLE_API_PRIVATE_KEY_BASE64`: base64 of `AuthKey_KEYID.p8`
- `APPLE_TEAM_ID`: Team ID
- `APPLE_KEYCHAIN_PASSWORD`: random CI-only temporary keychain password

## iOS: Repo Work After Apple Work

### 1. Commit generated Tauri iOS project

`src-tauri/gen/apple` has now been generated and extracted into the repo. Review and commit these files because Tauri expects the generated Apple project to exist in source control before `npm run ios:ipa` can build on CI.

If the Apple project ever needs regeneration, use MacInCloud or temporarily restore a macOS GitHub Actions bootstrap workflow, run `npm run ios:init`, and replace `src-tauri/gen/apple` with the newly generated output.

Equivalent macOS command for MacInCloud/local regeneration:

```bash
npm ci
npm run ios:init
```

Do not commit certificates, provisioning profiles, private keys, decoded API keys, or generated build output.

### 2. Configure iOS app capabilities

Open/check generated Xcode project.

Required likely:

- Bundle identifier matches App Store Connect.
- Development Team set to Apple Team ID.
- Background Modes > Audio remains enabled through `UIBackgroundModes: audio` in the generated iOS project.
- App icons complete.
- `Info.plist` includes encryption export setting if needed.

Because Papercut uses standard HTTPS/TLS for model downloads and does not implement custom cryptography, the generated iOS plist sets `ITSAppUsesNonExemptEncryption` to false. Still confirm the App Store Connect export-compliance answers before submission.

### 3. Add iOS build script

Added guarded npm scripts:

```bash
npm run ios:init
npm run ios:ipa
npm run ios:ipa:native-tts
```

`npm run ios:ipa` runs `tauri ios build --export-method app-store-connect` on macOS after `src-tauri/gen/apple` exists. `npm run ios:ipa:native-tts` first prepares the official sherpa-onnx iOS static XCFramework archive, verifies SHA-256, prepares thin Cargo link archives, sets `SHERPA_ONNX_LIB_DIR` and `ORT_LIB_LOCATION`, and builds with `native-tts-ios`. Device builds use `ios-arm64`; the framework-wrapped ONNX Runtime archive and both upstream simulator archives are thinned to arm64 because Rust cannot link their Mach-O universal containers directly. The iOS feature includes static Libtashkeel preprocessing for Arabic Piper generation.

Native TTS on iOS must be validated through TestFlight on a real iPhone or iPad. The frontend recognizes iOS and the native audio plugin uses AVPlayer/Now Playing controls, but long audiobook generation should still be treated as foreground/resumable work; background audio support is for playback, not a guarantee that synthesis continues while iOS suspends the app.

### 4. PR-safe iOS CI check

Regular PR CI now includes a `build-ios` job on `macos-26`, with an explicit iOS SDK 26+ guard before building. This job does not use Apple secrets and does not upload to App Store Connect. It verifies the generated Apple project files, builds the frontend, installs iOS Rust targets, runs `npm run ios:ci -- --native-tts` for the arm64 simulator, and runs `npm run ios:ci:device`, which prepares iOS native TTS libs, builds the `aarch64-apple-ios` Rust static library directly with Cargo using the configured iOS minimum deployment target, stages Cargo's `libapp_lib.a` output as `libapp.a` where Xcode expects it, and runs an unsigned `xcodebuild build` for a release-class generic iPhoneOS device link check without archive/export or App Store signing.

This catches broken iOS project files, Rust/Tauri iOS simulator compile failures, iPhoneOS device link failures, missing frontend assets, Swift toolchain/Xcode integration issues, and stale SDK runners before release. It does not replace the protected signed release job, because App Store provisioning and upload require secrets from the `apple-release` environment.

### 5. Add release CI job

The release workflow now has a `build-ios` job on `macos-26`, with an explicit iOS SDK 26+ guard before App Store upload:

1. Checkout.
2. Setup Node/Rust.
3. Install Rust iOS target if needed.
4. `npm ci`.
5. Import Apple Distribution `.p12` into a temporary keychain.
6. Install the provisioning profile and verify it targets `io.papercut.app`.
7. Decode the existing `APPLE_API_PRIVATE_KEY_BASE64` into `~/.appstoreconnect/private_keys/AuthKey_$APPLE_API_KEY.p8` for `altool`.
8. Patch generated Xcode signing settings/ExportOptions at runtime.
9. Run `npm run ios:ipa:native-tts`.
10. Upload `.ipa` as a CI artifact named `ios-app-store-ipa`, which is intentionally excluded from GitHub Release assets.
11. Upload `.ipa` to App Store Connect/TestFlight with `xcrun altool` using `APPLE_API_KEY` and `APPLE_API_ISSUER`.

Expected output per Tauri docs:

```text
src-tauri/gen/apple/build/arm64/Papercut Offline.ipa
```

### 6. TestFlight gate

First CI success only means Apple accepted upload. Still need:

- TestFlight processing pass.
- Export compliance completed.
- App privacy labels completed.
- Internal tester smoke test.
- External beta review if using external testers.
- Full App Review for production release.

## Secret Security Rules

1. Never commit `.key`, `.p12`, `.p8`, `.mobileprovision`, `.provisionprofile`, `.cer`, `.certSigningRequest`, or generated `private_keys/`.
2. Treat `.csr` as non-secret but still keep it out of repo to avoid confusion.
3. Store original private keys and `.p12` files in a password manager or encrypted offline vault.
4. Use separate certificates for Developer ID Application and Apple Distribution.
5. Use separate App Store Connect API keys for notarization and iOS upload if you want clean rotation.
6. Use GitHub Environments with required approvals for Apple release jobs.
7. Restrict release workflow trigger and environment deployment rules to protected tags like `v*.*.*`. Be careful with `workflow_dispatch`: the input ref is checked out and its build scripts run with signing secrets, so only approve runs for trusted protected tags.
8. Do not print secrets or decoded file contents in CI logs.
9. Apple signing jobs intentionally avoid npm and Rust build caches so a cache entry cannot influence a signed or uploaded release artifact.
10. Create temporary keychains in CI and delete them at job end.
11. Rotate/revoke immediately if a `.p12`, private key, `.p8`, or GitHub secret leaks.
12. Keep certificate passwords unique and unrelated to Apple ID password.
13. Prefer App Store Connect API keys over Apple ID app-specific passwords.
14. Use absolute paths for decoded CI secret files because Tauri/notarytool may run from `src-tauri` instead of the repository root.

## Critical Risks

- No macOS notarization means users get Gatekeeper warnings. Signing alone is not enough for modern macOS distribution.
- Tauri notarization covers the `.app`, but the outer `.dmg` still needs its own notarization/stapling before GitHub Release upload.
- Missing hardened runtime or wrong entitlements can make notarization fail or app launch fail after signing.
- Native dylibs in Resources must be signed/notarized as part of bundle.
- iOS CI cannot build a signed IPA until the generated `src-tauri/gen/apple` files are committed and the Apple Distribution certificate/provisioning profile secrets exist in the protected `apple-release` environment.
- iOS native TTS uses the official `sherpa-onnx-v1.13.4-ios.tar.bz2` static XCFramework archive, matching the shared sherpa version used by desktop and Android build helpers. CI verifies the archive checksum before extraction. Real-device TestFlight validation is still required for model download, Supertonic generation, playback, background controls, and highlighting.
- Current App ID `io.papercut.desktop` may be accepted but is a poor long-term iOS identifier.
- Apple private keys are high-value secrets. Losing the private key used for CSR means the downloaded certificate cannot be used.

## Done Definition

macOS done:

- CI imports Developer ID cert from secrets.
- CI builds signed DMG for Intel and Apple Silicon.
- CI notarizes and staples.
- `codesign` and `spctl` checks pass on runner.
- README no longer tells users to bypass unsigned-app warning.

iOS done:

- `src-tauri/gen/apple` committed.
- App Store Connect app record exists.
- CI builds signed `.ipa`.
- CI uploads `.ipa` to App Store Connect.
- Build appears in TestFlight.
- Internal tester installs and launches on real iPhone/iPad.
