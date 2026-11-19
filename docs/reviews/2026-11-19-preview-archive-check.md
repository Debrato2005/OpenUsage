# Preview archive check

November 19, 2026. L-6c needs verified inputs for the labelled Windows and
macOS builds. This check covers upstream archive integrity on Linux.

## Companion and scanner

The check downloaded six archives. Codex 0.163.0 matched the release asset
sizes and SHA-256 digests. The `ccusage` 20.0.48 payloads matched the scoped
npm records' SHA-512 integrity values. The check streamed each tar member.
It found the expected Mach-O architecture on Mac and x64 PE on Windows.
It hashed the complete executable members and removed the temporary archives.
It ran no downloaded code. [Codex release](https://github.com/openai/codex/releases/tag/rust-v0.163.0) · [npm package metadata](https://registry.npmjs.org/ccusage/20.0.48)

| Target | Archive | Archive bytes | Upstream digest | Executable member | Executable SHA-256 |
|---|---|---:|---|---|---|
| macOS arm64 | [Codex](https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-aarch64-apple-darwin.tar.gz) | 99,551,880 | `61c4d9a92521bdb42b0c1d9f87c673f6da628a06de5945cbdd9a0010632f6d10` | `codex-aarch64-apple-darwin` | `8813367285524ac798dd96d1b071aea30d58708c7226fd236cd275cf1cfdee31` |
| macOS arm64 | [ccusage](https://registry.npmjs.org/@ccusage/ccusage-darwin-arm64/-/ccusage-darwin-arm64-20.0.48.tgz) | 2,235,327 | `sha512-5nF3wIFO6IfJsVI3mc0/8iZtcfN8TEN4Dw1JyhToPQVBLVoaxbnBpNR16RWLQvS1yNcVVQfjeCDblmHe/Wt6/A==` | `package/bin/ccusage` | `e14a8136ff2433770c35c1ac4d5848db79280cdd489861b4a91569449b18cea8` |
| macOS x64 | [Codex](https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-x86_64-apple-darwin.tar.gz) | 108,640,069 | `562a1aea0e5686283a467687d268ae33705f02fd4d40f1bc4110a415c5c1c7b1` | `codex-x86_64-apple-darwin` | `73697f229103f463acaafd8659976caaa4892ec11e2c246c7fa97f93d523f90e` |
| macOS x64 | [ccusage](https://registry.npmjs.org/@ccusage/ccusage-darwin-x64/-/ccusage-darwin-x64-20.0.48.tgz) | 2,464,604 | `sha512-7P4dV2jPb3jyzfRncDyAyEnix50trmXYBnHxzhNzao69Rw4MEPGwlU5mkjwYX3/kTACQ5MDoomxHhFQ9Y8+Avg==` | `package/bin/ccusage` | `0e5f25da8a4cfa31a5aced8c457f99124e2e806491c32ca086519598a969f265` |
| Windows x64 | [Codex](https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-x86_64-pc-windows-msvc.exe.tar.gz) | 113,807,177 | `dbdd5ab5ecce1358e69fac8db1cb3538659c48e99498f3d2ea0e3e7ae878ef8c` | `codex-x86_64-pc-windows-msvc.exe` | `2692c8c7bb2cf8b0aefe638741fc90236f4b824c59578ce84994f9de58200993` |
| Windows x64 | [ccusage](https://registry.npmjs.org/@ccusage/ccusage-win32-x64/-/ccusage-win32-x64-20.0.48.tgz) | 2,379,395 | `sha512-4PCIs0BWmy3Mj8J93r0pX6XVAZpXQOYO4FxtgXrJnB0NJJ2jgGlsKEzwYPxZEc62Qq+ZKyyGB6c/6QnGw30LjQ==` | `package/bin/ccusage.exe` | `91e9fd1a3751ee11edbbca9777df6245f3871d3e8688127c0bf9242d689771f1` |

Release signing or provenance, runtime behavior, and Windows companion
helper requirements need separate checks. These hashes remain candidates
under the [compatibility matrix](../compatibility-matrix.md).

## Build tools

The check streamed twelve official archives and verified their SHA-256 digests.
The archive hashes came from the official Rust manifest, Node.js checksums,
and GitHub release metadata. It installed no build tools.
[Rust manifest](https://static.rust-lang.org/dist/channel-rust-1.99.1.toml) · [Node.js checksums](https://nodejs.org/dist/v22.23.2/SHASUMS256.txt) · [Bun release](https://github.com/oven-sh/bun/releases/tag/bun-v1.3.14) · [Gitleaks release](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1)

| Target | Archive | Bytes | Verified SHA-256 |
|---|---|---:|---|
| macOS arm64 | [rust-1.99.1-aarch64-apple-darwin.tar.xz](https://static.rust-lang.org/dist/2026-10-15/rust-1.99.1-aarch64-apple-darwin.tar.xz) | 156,101,588 | `30976378b6aaefd1861ba475905c6ba22a44c981418ce52285006af26337f98c` |
| macOS x64 | [rust-1.99.1-x86_64-apple-darwin.tar.xz](https://static.rust-lang.org/dist/2026-10-15/rust-1.99.1-x86_64-apple-darwin.tar.xz) | 140,792,464 | `13b237d0294bad3bb7b18d559517947ed8348ea25812fa3b468c904996f80e34` |
| Windows x64 | [rust-1.99.1-x86_64-pc-windows-msvc.tar.xz](https://static.rust-lang.org/dist/2026-10-15/rust-1.99.1-x86_64-pc-windows-msvc.tar.xz) | 204,203,204 | `4048445c5fa6fae8d5ca1a956e42c37ac2bdd1456c5f56a1139c0b074a7d1f6c` |
| macOS arm64 | [node-v22.23.2-darwin-arm64.tar.gz](https://nodejs.org/dist/v22.23.2/node-v22.23.2-darwin-arm64.tar.gz) | 50,068,815 | `61130f394c1630d211dd50aecc4353d379480f36d3ac913cd85dbba1aed585c6` |
| macOS x64 | [node-v22.23.2-darwin-x64.tar.gz](https://nodejs.org/dist/v22.23.2/node-v22.23.2-darwin-x64.tar.gz) | 51,246,936 | `58e99022c2ff89395576cc7fd4d98cea24bb68081475d5f88b801ee8729fb026` |
| Windows x64 | [node-v22.23.2-win-x64.zip](https://nodejs.org/dist/v22.23.2/node-v22.23.2-win-x64.zip) | 35,683,585 | `1177b4137ba5adaa56354ae40f1080c7450e8ae09cecb47da459d1c52ac99f97` |
| macOS arm64 | [bun-darwin-aarch64.zip](https://github.com/oven-sh/bun/releases/download/bun-v1.3.14/bun-darwin-aarch64.zip) | 23,586,433 | `d8b96221828ad6f97ac7ac0ab7e95872341af763001e8803e8267652c2652620` |
| macOS x64 | [bun-darwin-x64.zip](https://github.com/oven-sh/bun/releases/download/bun-v1.3.14/bun-darwin-x64.zip) | 26,509,109 | `4183df3374623e5bab315c547cfa0974533cd457d86b73b639f7a87974cd6633` |
| Windows x64 | [bun-windows-x64.zip](https://github.com/oven-sh/bun/releases/download/bun-v1.3.14/bun-windows-x64.zip) | 38,366,737 | `0a0620930b6675d7ba440e81f4e0e00d3cfbe096c4b140d3fff02205e9e18922` |
| macOS arm64 | [gitleaks_8.30.1_darwin_arm64.tar.gz](https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_arm64.tar.gz) | 7,897,593 | `b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5` |
| macOS x64 | [gitleaks_8.30.1_darwin_x64.tar.gz](https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_darwin_x64.tar.gz) | 8,359,235 | `dfe101a4db2255fc85120ac7f3d25e4342c3c20cf749f2c20a18081af1952709` |
| Windows x64 | [gitleaks_8.30.1_windows_x64.zip](https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip) | 8,438,883 | `d29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e` |

Preview CI still needs a locked build and checks on each target. Installation
and runtime acceptance need the systems listed in the
[compatibility matrix](../compatibility-matrix.md).
