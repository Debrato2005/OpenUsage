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
