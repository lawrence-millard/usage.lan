# usage.lan

Self-hosted AI usage & spend dashboard. Aggregates token usage and cost across your local AI coding tools — OpenCode, Codex, and Cursor — and serves a live web dashboard.

## Features

- **Local data sources** — reads usage straight from your machine:
  - **OpenCode** — SQLite session DBs in `~/.local/share/opencode`
  - **Codex** — `state_*.sqlite` plus `rollout-*.jsonl` files in `~/.codex`
  - **Cursor** — dashboard usage CSV via your Cursor session
- **Cost estimation** — built-in model price table fills in cost when the tool doesn't report it (see `usage-core/src/pricing.rs`)
- **Aggregations** — by day, provider, model, sub-agent, and project; today / yesterday / 7d / 30d / all-time windows
- **Web UI** — a GPU-accelerated dashboard written in Rust with [open-gpui](https://github.com/lapce/open-gpui), compiled to WASM
- **Optional TLS** — serve over HTTPS with your own certificate

## Structure

```
dashboard/
├── Cargo.toml            # workspace
├── certs/                # local self-signed TLS certs (gitignored)
└── crates/
    ├── usage-core/       # shared models + pricing (used by server & UI)
    ├── usage-server/     # axum backend: reads local data, serves API + UI
    └── usage-ui/         # open-gpui web dashboard (built to WASM with trunk)
```

## Requirements

- Rust (see `rust-toolchain.toml` — the UI needs **nightly** + `wasm32-unknown-unknown`)
- [`trunk`](https://trunkrs.dev/) to build the UI

## Building

```sh
# 1. Add the wasm target (pinned in rust-toolchain.toml):
rustup target add wasm32-unknown-unknown

# 2. Build the UI:
cd crates/usage-ui
trunk build --release
cd ../..

# 3. Run the server (serves both the API and the built UI):
cargo run -p usage-server
```

The server serves the UI from `crates/usage-ui/dist` by default (override with `USAGE_DIST`).

## Usage

Open `http://localhost:PORT/` in a browser. You can filter by agent (All / OpenCode / Codex / Cursor) and by window (7 / 30 / 90 days / All time).

### API

```
GET /api/usage?agent=opencode&days=30
GET /api/health
```

`agent` ∈ `all | opencode | codex | cursor` (default `all`). `days` > 0 (default `30`), or `-1` for all time.

### Environment variables

| Var | Default | Purpose |
|-----|---------|---------|
| `PORT` / `USAGE_PORT` | `443` | Listen port |
| `USAGE_DIST` | `crates/usage-ui/dist` | Path to built UI |
| `USAGE_TLS_CERT` | — | PEM cert path (enables HTTPS when set with key) |
| `USAGE_TLS_KEY` | — | PEM key path |
| `CODEX_HOME` | `~/.codex` | Codex data location |

### TLS / HTTPS

The UI uses `SharedArrayBuffer`, which requires a secure context. Over HTTP it only works on `localhost`. To serve over the network, generate a cert and set `USAGE_TLS_CERT` / `USAGE_TLS_KEY`:

```sh
# self-signed, e.g. for usage.lan
openssl req -x509 -newkey rsa:2048 -keyout certs/usage.key -out certs/usage.crt \
  -days 825 -nodes -subj "/CN=usage.lan"
```

## How it works

`usage-server` reads each tool's local data on a short TTL (fast sources every 30s, Cursor up to 10 min), aggregates it into a `UsageReport` (`usage-core/models.rs`), and serves it as JSON. `usage-ui` fetches that JSON and renders it with open-gpui.

Cost is read from the source when available; otherwise `estimate_cost` in `usage-core/pricing.rs` prices tokens using the built-in table.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)