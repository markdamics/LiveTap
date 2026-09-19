# LiveTap — Tech Stack, Architecture & Roadmap

A privacy-first webhook inspector / API testing tool. Desktop-first,
cross-platform, with a secondary mobile target. Built on a shared Rust
core with native UI per platform, self-hosted relay, local-first storage,
no Google dependencies.

---

## 1. Priorities & Constraints

- **Desktop**: primary target, cross-platform (Windows, macOS, Linux) —
  must render its own UI consistently rather than depending on a native
  toolkit that doesn't exist uniformly across Linux desktops (GNOME vs KDE)
- **Mobile**: secondary target, cross-platform (iOS, Android)
- **Languages available**: Rust, Java, JS/TS
- **No third-party services** beyond an explicit, opt-in GitHub sync — no
  Google/Firebase anywhere in the default path
- **Data privacy is a first-class requirement**, not an afterthought:
  captured webhook payloads can contain real secrets (API keys, signing
  secrets, auth tokens) and must be treated accordingly

---

## 2. Tech Stack

### Shared core (used by desktop and both mobile platforms)

| Layer | Choice | Notes |
|---|---|---|
| Core logic | **Rust** (`livetap-core` crate) | SQLite access, WebSocket client, GitHub sync, session/rule handling, captured-request data models — written once, used everywhere |
| Local DB | **SQLite via `rusqlite`/`sqlx`**, encrypted with **SQLCipher** | Encrypted at rest; DB key stored in OS keychain, not on disk |
| Secure storage | **`keyring` crate** | Holds the SQLCipher key + GitHub PAT, backed by OS credential store (Keychain / Credential Manager / Secret Service) |
| Realtime client | **`tokio-tungstenite`** | WebSocket connection to the relay, shared by all platforms |
| GitHub sync (optional) | **`octocrab`** or `reqwest` + GitHub Contents API | Fine-grained PAT, scoped to one repo; syncs only saved requests/environments/rules — never captured payloads |
| Data model | `serde` structs | Shared between core, relay, and (potentially) UI layers |
| Mobile FFI bridge | **`uniffi-rs`** | Generates idiomatic Kotlin and Swift bindings directly from the Rust core — same pattern Mozilla uses for Firefox Android/iOS |

### Desktop UI

| Layer | Choice | Notes |
|---|---|---|
| Framework | **iced** | Rust-native, Elm-architecture GUI toolkit; paints its own widgets via `wgpu`/`tiny-skia` — no dependency on GTK or Qt, so it renders consistently across GNOME, KDE, Windows, and macOS instead of picking one native toolkit and looking foreign on the others |
| Windowing | `winit` (used internally by iced) | Native window chrome (title bar, resize, multi-monitor) is genuinely native; widgets inside the window are iced's own custom-drawn controls |
| Design approach | Custom theme via iced's theming system | Since no widget is OS-native, invest early in a coherent design system rather than imitating Fluent/Aqua piecemeal |

### Mobile UI (secondary priority)

| Layer | Choice | Notes |
|---|---|---|
| Android UI | **Kotlin + Jetpack Compose** | Calls into `livetap-core` via `uniffi`-generated bindings; leans on existing Java/JVM experience |
| iOS UI | **Swift + SwiftUI** | Calls into `livetap-core` via `uniffi`-generated bindings |
| Why not iced on mobile | iced's Android/iOS support is experimental — depends on `winit`'s under-maintained mobile backends. Not suitable to build a privacy-focused tool on top of today | Revisit if/when iced's mobile story matures |

### Relay backend (self-hosted) — unchanged

| Layer | Choice | Notes |
|---|---|---|
| Language | **Rust** | Same language as the shared core — consistent skillset across relay, desktop, and mobile business logic |
| HTTP + WebSocket server | **`axum`** (on `tokio`/`hyper`) | Wildcard route for arbitrary inbound webhook methods/paths; WebSocket upgrade endpoint |
| Session state (single instance) | `dashmap` | In-memory, no external dependency needed at small scale |
| Session state (multi-instance, later) | Redis pub/sub | Only if/when horizontal scaling is actually needed |
| Push (iOS) | `a2` crate → APNs | Generic alert payload only, never the actual captured data |
| Push (Android) | UnifiedPush (plain HTTPS POST to distributor) | No Google/FCM dependency |
| TLS | Reverse proxy (**Caddy**, automatic Let's Encrypt) in front, relay runs plain HTTP internally | Keeps cert management out of the Rust binary |
| Deployment | Docker, multi-stage build, `musl` target for a portable static-ish binary | Runs on any self-controlled VPS/container host |

### Explicitly excluded

- **Tauri** — reconsidered: WebKitGTK-based Linux rendering makes the app look GNOME-flavored on KDE and other non-GNOME desktops, an unavoidable consequence of delegating widget rendering to a native toolkit that isn't universal on Linux
- MongoDB Realm / Atlas Device SDK — reached end-of-life September 30, 2025, do not build on it
- Firebase / Google Cloud Messaging — replaced by UnifiedPush + direct APNs
- Google Drive / iCloud sync — replaced by opt-in GitHub sync only
- Cloudflare Workers (as a *required* dependency) — self-hosted Rust relay instead, to keep the "no third-party services" guarantee intact end-to-end

---

## 3. Architecture

```
                    ┌─────────────────────────────────────────────┐
                    │           Shared Rust Core                    │
                    │           (crate: livetap-core)                │
                    │  - SQLCipher-encrypted DB (rusqlite/sqlx)      │
                    │  - keyring-backed secure storage               │
                    │  - WebSocket client (tokio-tungstenite)        │
                    │  - GitHub sync (octocrab)                      │
                    │  - session/rule/request data models (serde)    │
                    └───────┬───────────────┬───────────────┬───────┘
                            │               │               │
                  (direct fn calls)  (uniffi bindings) (uniffi bindings)
                            │               │               │
              ┌─────────────▼───┐  ┌────────▼────────┐  ┌───▼─────────────┐
              │   Desktop UI      │  │   Android UI     │  │   iOS UI          │
              │   iced (Rust)      │  │   Kotlin+Compose │  │   Swift+SwiftUI   │
              │   - custom-painted │  │   - native Compose│  │   - native SwiftUI│
              │     widgets, no    │  │     widgets, full │  │     widgets, full │
              │     GTK/Qt dep     │  │     platform feel │  │     platform feel │
              └───────────────────┘  └───────────────────┘  └───────────────────┘
                            │               │               │
                            └───────────────┼───────────────┘
                                            │ wss:// (TLS, no plaintext)
                                            ▼
                    ┌─────────────────────────────────────────────┐
                    │            Self-Hosted Relay                  │
                    │            (Rust: axum + tokio)                │
                    │                                                 │
                    │  1. Mint unique session/URL                     │
                    │  2. Accept ANY inbound webhook                   │
                    │     (method, headers, raw body)                 │
                    │  3. Short-TTL in-memory buffer                   │
                    │  4. Forward over WS if connected                 │
                    │  5. Push (APNs/UnifiedPush) if not               │
                    │  6. Evaluate mock-response rules                 │
                    │  7. Purge on delivery / TTL expiry               │
                    └─────────────────────────────────────────────┘
                                            ▲
                                            │ arbitrary HTTP (Stripe, GitHub, curl…)
                                            │
                    ┌─────────────────────────────────────────────┐
                    │         Third-party webhook senders            │
                    └─────────────────────────────────────────────┘

     Optional, opt-in, separate path:
     Shared core ──── GitHub Contents API ──── user's own repo
     (workbench data only: collections, environments, mock rules —
      never captured payloads)
```

### Data classification & flow

| Data | Where it lives | Leaves the device? |
|---|---|---|
| Captured webhook payloads | SQLCipher-encrypted local DB | No — relay buffer is purged after delivery, never persisted durably |
| Saved requests / environments / mock rules | SQLCipher-encrypted local DB | Only if user enables GitHub sync |
| GitHub PAT | OS keychain via `keyring` | No — used locally to authenticate outbound API calls only |
| Push notification payloads | N/A (ephemeral) | Generic alert + session ID only, via APNs/UnifiedPush infra — actual data fetched afterward over encrypted WS |

### Security posture

- **Encryption at rest**: SQLCipher, key in OS credential store
- **Encryption in transit**: `wss://`/`https://` only, no plaintext fallback; certificate pinning worth considering for hostile-network resilience
- **Untrusted content handling**: captured payloads are attacker-controllable — since desktop UI is custom-painted by iced (not an HTML webview), there's no script-injection surface the way there would be rendering payloads inside a webview; mobile native UIs (Compose/SwiftUI) carry the same inherent protection
- **No telemetry**: audit any third-party crate/dependency added for phone-home behavior; skip or self-host any crash reporting
- **Least-privilege tokens**: fine-grained, single-repo-scoped GitHub PAT
- **Transparency**: open-sourcing the app and/or relay is the strongest form of the "we don't retain your data" claim — recommended, not required

---

## 4. Roadmap

### Phase 0 — Foundations
- Set up the `livetap-core` Rust library crate (no UI dependencies)
- Set up the relay project (Rust, `axum`), deployable via Docker to a VPS
- SQLCipher integration + `keyring`-backed key storage in the core
- Define the shared `serde` envelope for a captured request, used by core, relay, and (via `uniffi`) mobile UIs
- Set up `uniffi-rs` scaffolding early, even before mobile UI work starts, so the FFI boundary is validated from day one rather than retrofitted later

### Phase 1 — Core capture loop (relay)
- `POST /session` — mint a unique session/URL
- `ANY /hook/:session_id` — capture method, headers, raw body, IP, timestamp
- `GET /ws/:session_id` — WebSocket upgrade, live forwarding to connected clients
- TTL-based session cleanup
- Validate end-to-end with `curl` + `wscat` before touching any UI

### Phase 2 — LiveTap UI (desktop, iced)
- Feed screen: real-time card stream, color-coded method badges, WebSocket wired to the relay via `livetap-core`
- Deep Request Inspector: tabbed Headers / Body / Diff view, collapsible JSON tree, syntax highlighting, search — all custom iced widgets
- Local persistence of all captured requests into SQLCipher DB via the shared core
- Establish the iced theme/design system early, since every widget is custom-painted rather than OS-native

### Phase 3 — Buffering & reliability
- Short-lived ring buffer per session on the relay (survives brief disconnects)
- Flush-on-reconnect logic in the shared core
- Hard retention caps + purge-on-delivery, enforced on the relay

### Phase 4 — API Workbench (request builder, desktop)
- Method pills, header/param autosuggest, environment selector (Local/Staging/Production)
- Request/response history, saved into local DB via shared core
- "Resend as Request" (Instant Replay) from any captured card

### Phase 5 — Mock Response rule engine
- Relay-side rule storage per session (`PUT /session/:id/rules`)
- Simple rule matching: method + header-presence + path glob → status + body
- Evaluated server-side so it works even when no client app is connected

### Phase 6 — GitHub sync (opt-in)
- Fine-grained PAT auth flow, implemented once in `livetap-core`
- Push/pull buttons (manual sync, no automatic bidirectional merge for v1)
- Syncs collections/environments/rules only, as JSON/YAML files — explicit in-app copy confirming captured payloads never sync

### Phase 7 — Mobile UI (secondary priority)
- Generate and integrate `uniffi` Kotlin bindings; build the Android Compose UI (feed, inspector, workbench) reusing `livetap-core` for all logic
- Generate and integrate `uniffi` Swift bindings; build the iOS SwiftUI UI
- APNs integration (`a2`) on the relay for iOS push
- UnifiedPush integration on the relay for Android push
- Push payloads stay generic-alert-only; full data fetched over WS on wake

### Phase 8 — Hardening & polish
- Security review: dependency/crate audit for telemetry, cert pinning decision
- Multi-pane desktop layout for Workbench/Inspector (sidebar + editor + response) in iced
- Consider open-sourcing the app and/or relay
- Horizontal relay scaling (Redis pub/sub) — only if real load justifies it

---

## 5. Open Decisions

- **iced theming**: design the visual identity as its own coherent system rather than imitating Fluent (Windows) or Aqua (macOS) piecemeal, since no widget is OS-native regardless of platform
- **iced mobile maturity**: revisit whether iced's Android/iOS support has matured enough to reconsider before Phase 7 locks in the Compose/SwiftUI split — currently not recommended
- **Certificate pinning**: adds resilience against hostile networks/compromised CAs, but adds cert-rotation operational overhead — decide based on threat model
- **Open-sourcing**: strengthens the privacy claim but is a separate commitment from the architecture itself
- **Redis / horizontal scaling**: deliberately deferred until real usage data justifies the added complexity
