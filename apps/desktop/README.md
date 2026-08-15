# GixGiz Desktop

The GixGiz Windows desktop shell renders presentation state and user intentions. It launches the bundled Rust core through `CoreClient`, completes an authenticated typed handshake, and renders authoritative core version/readiness, hardware evidence, capability reports, local-runtime status, and durable local-model setup state. It contains no hardware collection, recommendation rules, installer, persistence, runtime implementation, model-management, or provider logic.

## Toolchain

- Windows 11 x64.
- Flutter `3.44.8` stable, framework revision `058e0af2c2b57e369d905a03ac9748b0ebf543c6`.
- Dart `3.12.2`, supplied by the pinned Flutter release.
- Visual Studio 2022 or Visual Studio Build Tools with Desktop development with C++ and a Windows SDK.

## Commands

From the repository root:

```powershell
Set-Location apps/desktop
flutter pub get
flutter gen-l10n
flutter analyze
flutter test
flutter run -d windows
flutter build windows
```

`flutter build windows` also builds the release Rust sidecar with Cargo and places `gixgiz-core.exe` beside `GixGiz.exe`. Cargo must be available through Rustup; the repository-pinned toolchain is selected automatically.

`flutter run -d windows` launches the Debug runner from `apps/desktop/build/windows/x64/runner/Debug/GixGiz.exe`. The default `flutter build windows` command writes the Release runner to `apps/desktop/build/windows/x64/runner/Release/GixGiz.exe`.

## Application identity

- Product display name: `GixGiz`.
- Dart package: `gixgiz_desktop`.
- Windows executable: `GixGiz.exe`.
- Reserved Windows package/application identifier: `ai.gixgiz.desktop`.

The unpackaged development runner does not register a Windows package identity. The identifier remains reserved until the packaging format is selected by a later ADR.

## Structure

- `lib/app/`: application identity, routes, theme, and composition.
- `lib/core/`: typed `CoreClient`, supervised sidecar connection, and safe transport mapping.
- `lib/core/generated/`: generated Dart wire models; do not edit by hand.
- `lib/features/`: Foundation and About presentation modules.
- `lib/l10n/`: localizable source strings and generated localization code.
- `lib/shared/`: shared shell and page presentation.
- `test/`: bootstrap, state, navigation, focus, semantics, and text-scaling tests.

## Contract bindings

Run from the repository root:

```powershell
cargo run -p gixgiz-contracts --example generate_bindings -- --write
cargo run -p gixgiz-contracts --example generate_bindings -- --check
```

The first command regenerates the Rust-owned JSON Schema and Dart bindings; the second fails when either committed artifact drifts. See [`../../docs/guides/flutter-rust-transport.md`](../../docs/guides/flutter-rust-transport.md) for bootstrap and security details.

The Foundation screen can request an authenticated capability report after a completed or partial hardware scan. Flutter sends only the typed `MachineProfile` and user preferences, then renders the Rust-owned recommended, fallback, larger, or no-plan result. Catalogue policy and user-facing terminology are documented in [`../../docs/guides/capability-recommendations.md`](../../docs/guides/capability-recommendations.md).

Runtime status, ownership, consent, lifecycle availability, and installed-model inventory also come from the Rust-owned `CoreClient` boundary. Flutter may present those values and send typed refresh, consent, lifecycle, cancellation, or inventory intentions. It never calls a provider endpoint, executes a provider command, infers compatibility, changes ownership, installs software, or downloads or deletes models.

The Foundation screen can turn a selected recommendation into a persisted setup plan, show its exact model, licence, provenance, destination, conservative size, warnings, and material effects, then send an explicit approval for that plan revision. Setup progress is consumed from cursor-based bounded events and recovered from Rust after desktop or core restart. Navigating away or closing the desktop detaches the event subscription without cancelling the durable job; only the confirmed Cancel action sends a cancellation intention. The UI never infers readiness from downloaded bytes: Ready is rendered only from the verified persisted setup state.
