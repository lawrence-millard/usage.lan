# Contributing

Thanks for wanting to help out. This is a small, self-hosted project — keep things simple and in the spirit of the codebase.

## Development setup

1. Install the pinned toolchain and wasm target:

   ```sh
   rustup toolchain install nightly
   rustup target add --toolchain nightly wasm32-unknown-unknown
   ```

2. For the UI, install [trunk](https://trunkrs.dev/):

   ```sh
   cargo install trunk
   ```

3. Run the dev server for the UI with live reload:

   ```sh
   cd crates/usage-ui
   trunk serve
   ```

   The API is only available from the backend, so run it too if you need live data:

   ```sh
   cargo run -p usage-server
   ```

## What to work on

Good first issues: add a new data source, expand the pricing table (`usage-core/src/pricing.rs`), improve the dashboard UI in `crates/usage-ui/src/main.rs`.

## Code style

- Format with `cargo fmt` (nightly).
- Keep changes scoped to the crate they belong to — shared types and pricing live in `usage-core`.
- Don't add comments unless they explain something non-obvious.

## Pull requests

- Open the PR against `main`.
- Describe what changed and why, and note anything about your local setup that's relevant.
- Keep PRs small and reviewable. If a change is large, open an issue first to discuss it.

## Testing

There is no test suite yet. If you add logic that's non-trivial (e.g. a new aggregator or parser), add a small unit test alongside it.
