# Live Catch — Tech Stack, Architecture & Roadmap

A privacy-first webhook inspector / API testing tool. Desktop-first,
cross-platform, with a secondary mobile target. Built on Rust + TypeScript,
self-hosted relay, local-first storage, no Google dependencies.

---

## 1. Priorities & Constraints

- **Desktop**: primary target, cross-platform (Windows, macOS, Linux)
- **Mobile**: secondary target, cross-platform (iOS, Android)
- **Languages available**: Rust, Java, JS/TS
- **No third-party services** beyond an explicit, opt-in GitHub sync — no
  Google/Firebase anywhere in the default path
- **Data privacy is a first-class requirement**, not an afterthought:
  captured webhook payloads can contain real secrets (API keys, signing
  secrets, auth tokens) and must be treated accordingly

---

## 2. Tech Stack

### App (desktop-primary, mobile-secondary)

| Layer | Choice | Notes |
|---|---|---|
| Framework | **Tauri 2.0** | Rust core + native OS webview; desktop is Tauri's most mature target, mobile (Android/iOS) is supported but less polished — matches our priority order |
| App core / business logic | **Rust** | SQLite access, WebSocket client, GitHub sync, session/rule handling |
| UI layer | **TypeScript** + React or Svelte | Renders in the OS's native webview (WebView2 / WKWebView / WebView) — no bundled Chromium, small binaries |
| Local DB | **SQLite via `rusqlite`/`sqlx`**, encrypted with **SQLCipher** | Encrypted at rest; DB key stored in OS keychain, not on disk |
| Secure storage | **`keyring` crate** | Holds the SQLCipher key + GitHub PAT, backed by OS credential store (Keychain / Credential Manager / Secret Service) |
| Realtime client | **`tokio-tungstenite`** (Rust) | WebSocket connection to the relay |
| GitHub sync (optional) | **`octocrab`** or `reqwest` + GitHub Contents API | Fine-grained PAT, scoped to one repo; syncs only saved requests/environments/rules — never captured payloads |
| JSON tree / diff (UI) | `react-json-view` / `svelte-json-tree` + `jsondiffpatch` | Collapsible tree, syntax highlighting, side-by-side diff |
| Virtualized feed list | `@tanstack/virtual` | Handles hundreds/thousands of captured requests without jank |
| QR export | any JS QR lib (e.g. `qrcode`) rendered in the webview | For sharing the capture URL to a test device |

### Relay backend (self-hosted)

| Layer | Choice | Notes |
|---|---|---|
| Language | **Rust** | Same language as the app core — one skillset covers app + relay |
| HTTP + WebSocket server | **`axum`** (on `tokio`/`hyper`) | Wildcard route for arbitrary inbound webhook methods/paths; WebSocket upgrade endpoint for the app |
| Session state (single instance) | `dashmap` | In-memory, no external dependency needed at small scale |
| Session state (multi-instance, later) | Redis pub/sub | Only if/when horizontal scaling is actually needed |
| Push (iOS) | `a2` crate → APNs | Generic alert payload only, never the actual captured data |
| Push (Android) | UnifiedPush (plain HTTPS POST to distributor) | No Google/FCM dependency |
| TLS | Reverse proxy (**Caddy**, automatic Let's Encrypt) in front, relay runs plain HTTP internally | Keeps cert management out of the Rust binary |
| Deployment | Docker, multi-stage build, `musl` target for a portable static-ish binary | Runs on any self-controlled VPS/container host |

### Explicitly excluded

- MongoDB Realm / Atlas Device SDK — reached end-of-life September 30, 2025, do not build on it
- Firebase / Google Cloud Messaging — replaced by UnifiedPush + direct APNs
- Google Drive / iCloud sync — replaced by opt-in GitHub sync only
- Cloudflare Workers (as a *required* dependency) — self-hosted Rust relay instead, to keep the "no third-party services" guarantee intact end-to-end

---

## 3. Architecture

```
                         ┌───────────────────────────────────┐
                         │           Tauri App                │
                         │  ┌───────────────┐ ┌─────────────┐ │
                         │  │   Rust core    │ │  Web UI     │ │
                         │  │  - SQLCipher DB│◄┤  (TS/React  │ │
                         │  │  - keyring     │ │   or Svelte)│ │
                         │  │  - WS client   │ │  - feed     │ │
                         │  │  - GitHub sync │ │  - inspector│ │
                         │  └───────┬────────┘ │  - workbench│ │
                         │          │           └─────────────┘ │
                         └──────────┼─────────────────────────┘
                                    │ wss:// (TLS, no plaintext)
                                    ▼
                         ┌───────────────────────────────────┐
                         │        Self-Hosted Relay           │
                         │        (Rust: axum + tokio)        │
                         │                                     │
                         │  1. Mint unique session/URL         │
                         │  2. Accept ANY inbound webhook       │
                         │     (method, headers, raw body)     │
                         │  3. Short-TTL in-memory buffer       │
                         │  4. Forward over WS if connected     │
                         │  5. Push (APNs/UnifiedPush) if not   │
                         │  6. Evaluate mock-response rules     │
                         │  7. Purge on delivery / TTL expiry   │
                         └───────────────────────────────────┘
                                    ▲
                                    │ arbitrary HTTP (Stripe, GitHub, curl…)
                                    │
                         ┌───────────────────────────────────┐
                         │     Third-party webhook senders     │
                         └───────────────────────────────────┘

     Optional, opt-in, separate path:
     App (Rust core) ──── GitHub Contents API ──── user's own repo
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
- **Untrusted content handling**: captured payloads are attacker-controllable — strict Tauri CSP, no remote script loading, bundled UI assets only, so a malicious payload rendered in the JSON viewer can't execute anything
- **No telemetry**: audit every Tauri plugin added for phone-home behavior; skip or self-host any crash reporting
- **Least-privilege tokens**: fine-grained, single-repo-scoped GitHub PAT
- **Transparency**: open-sourcing the app and/or relay is the strongest form of the "we don't retain your data" claim — recommended, not required

---

## 4. Roadmap

### Phase 0 — Foundations
- Set up Tauri 2.0 project scaffold (Rust core + TS/React or Svelte frontend)
- Set up the relay project (Rust, `axum`), deployable via Docker to a VPS
- SQLCipher integration + `keyring`-backed key storage in the app
- Define the shared JSON envelope (`serde` structs) for a captured request, reused conceptually between relay and app

### Phase 1 — Core capture loop (relay)
- `POST /session` — mint a unique session/URL
- `ANY /hook/:session_id` — capture method, headers, raw body, IP, timestamp
- `GET /ws/:session_id` — WebSocket upgrade, live forwarding to connected clients
- TTL-based session cleanup
- Validate end-to-end with `curl` + `wscat` before touching the app UI

### Phase 2 — Live Catch UI (desktop)
- Feed screen: real-time card stream, color-coded method badges, WebSocket wired to the relay
- Deep Request Inspector: tabbed Headers / Body / Diff view, collapsible JSON tree, syntax highlighting, search
- Local persistence of all captured requests into SQLCipher DB

### Phase 3 — Buffering & reliability
- Short-lived ring buffer per session on the relay (survives brief disconnects)
- Flush-on-reconnect logic in the app
- Hard retention caps + purge-on-delivery, enforced on the relay

### Phase 4 — API Workbench (request builder)
- Method pills, header/param autosuggest, environment selector (Local/Staging/Production)
- Request/response history, saved into local DB
- "Resend as Request" (Instant Replay) from any captured card

### Phase 5 — Mock Response rule engine
- Relay-side rule storage per session (`PUT /session/:id/rules`)
- Simple rule matching: method + header-presence + path glob → status + body
- Evaluated server-side so it works even when the app is offline

### Phase 6 — GitHub sync (opt-in)
- Fine-grained PAT auth flow
- Push/pull buttons (manual sync, no automatic bidirectional merge for v1)
- Syncs collections/environments/rules only, as JSON/YAML files — explicit in-app copy confirming captured payloads never sync

### Phase 7 — Push-to-Debug alerts & mobile build
- APNs integration (`a2`) on the relay
- UnifiedPush integration for Android
- Tauri mobile build (Android + iOS) — secondary priority, expect fewer polished plugins than desktop
- Generic-alert-only push payloads; full data fetched over WS on wake

### Phase 8 — Hardening & polish
- Security review: CSP audit, cert pinning decision, dependency/plugin telemetry audit
- Multi-pane desktop layout for Workbench/Inspector (sidebar + editor + response)
- Consider open-sourcing the app and/or relay
- Horizontal relay scaling (Redis pub/sub) — only if real load justifies it

---

## 5. Open Decisions

- **Frontend framework**: React vs Svelte for the Tauri UI layer — either works, Svelte may suit the highly interactive feed/tree UI with less overhead
- **Certificate pinning**: adds resilience against hostile networks/compromised CAs, but adds cert-rotation operational overhead — decide based on threat model
- **Open-sourcing**: strengthens the privacy claim but is a separate commitment from the architecture itself
- **Redis / horizontal scaling**: deliberately deferred until real usage data justifies the added complexity
