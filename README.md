# OpenUsage

A local desktop application for account usage and quota readings. Development uses Tauri 2, Rust, React, and SQLCipher 4 with WAL. See [implementation status](docs/implementation-status.md) for the current scope and executed checks.

## Linux build with LLD

With native prerequisites installed and dependencies cached, build the frontend and desktop binary:

```sh
npm run build
CARGO_BUILD_JOBS=1 cargo rustc --locked --offline -p openusage-desktop --bin openusage-desktop -- -C link-arg=-Wl,--threads=1
```

This Linux/LLD command limits linker workers as well as Cargo jobs. Cargo applies the additional flag only to the selected binary. See the [Cargo command documentation](https://doc.rust-lang.org/cargo/commands/cargo-rustc.html) and [LLD options](https://github.com/llvm/llvm-project/blob/main/lld/docs/ld.lld.1).

The observed build took 39 seconds and produced the same bytes as the native-tested default binary. This is one shared-host observation, not a performance benchmark. See the [build record](docs/evidence/linux-bounded-linker/verification.json).
## Linux companion build

A provisioned build includes the pinned official Codex companion and its license notices. Rust checks the companion before execution. Account sign-in keeps its own consent. Manual paths remain in Settings for troubleshooting.

Prepare the reviewed archive, then enable the companion configuration:

```sh
npm run build
npm run prepare:companion -- --offline
TAURI_CONFIG="$(cat apps/desktop/src-tauri/tauri.companion.conf.json)" \
  CARGO_BUILD_JOBS=1 cargo rustc --locked --offline \
  -p openusage-desktop --bin openusage-desktop -- -C link-arg=-Wl,--threads=1
```

The archive cache defaults to `.tools` in this checkout. Use `--cache-dir /absolute/cache` for a shared cache. Omit `--offline` only to download the exact pinned archive during build preparation. The script checks archive and executable hashes without launching the executable. It refuses unsupported targets.

The Tauri override stages `openusage-codex` beside the application and copies LICENSE and NOTICE under `licenses/codex`. Keep those files together. The ordinary development build can instead discover a supported installed version. It does not produce a companion bundle by default.

This path currently covers Linux x86_64 only. A native build does not prove an installer or another platform. See the [source review and architecture decision](docs/reviews/2026-10-21-onboarding-sources.md).
