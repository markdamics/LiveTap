# LiveTap

A privacy-first webhook inspector / API testing tool. A shared Rust core
(`livetap-core`) backs a desktop UI built with `iced` (no GTK/Qt/webview —
renders consistently across GNOME, KDE, Windows, and macOS) and, later,
native Android/iOS UIs via `uniffi`-generated bindings, all talking to a
self-hosted Rust relay. See [`docs/livetap-plan.md`](docs/livetap-plan.md)
for the full architecture and roadmap.

There is no desktop or mobile UI yet — Phase 0 only sets up the shared
core, the relay, and the mobile FFI scaffolding. UI work starts in later
phases.

## Project layout

| Path | What it is |
|---|---|
| `crates/shared/` | `livetap-shared` — the `serde` JSON envelope types (e.g. `CapturedRequest`), used by the relay directly and re-exported through `livetap-core`'s `uniffi` bindings for mobile |
| `crates/core/` | `livetap-core` — the shared Rust core: SQLCipher-encrypted local storage, OS-keychain-backed encryption key, exposed to future desktop/mobile UIs both as a plain Rust API and as `uniffi` bindings |
| `relay/` | Self-hosted relay — `axum`/`tokio` HTTP + WebSocket server |

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs)
- A working OS credential store for `livetap-core`'s keychain-backed
  encryption key: GNOME Keyring, KDE Wallet/`ksecretd`, or another
  freedesktop Secret Service implementation on Linux; Keychain on macOS;
  Credential Manager on Windows (used automatically, nothing to install
  there)

## Running the relay

```sh
cd relay
cargo run
```

Listens on `0.0.0.0:8787` by default (override with `LIVETAP_RELAY_ADDR`).
Check it's up:

```sh
curl http://127.0.0.1:8787/health
```

To run it via Docker instead (build context is the repo root, since the
relay depends on the `crates/shared` workspace member):

```sh
docker compose -f relay/docker-compose.yml up --build
```

## Working on the shared core

`livetap-core` has no UI yet — exercise it via its test suite, or from a
`cargo` REPL/scratch binary. It also builds as a `cdylib`/`staticlib` for
`uniffi` mobile bindings:

```sh
# Generate Kotlin bindings (Android)
cargo build --package livetap-core
cargo run --package livetap-core --features uniffi-cli --bin uniffi-bindgen -- \
  generate --library target/debug/liblivetap_core.so --language kotlin --out-dir bindings/kotlin

# Generate Swift bindings (iOS) — swap --language swift, --out-dir bindings/swift
```

`bindings/` is generated output (gitignored), regenerated per target
platform rather than checked in.

## Tests

```sh
cargo test --workspace
```
