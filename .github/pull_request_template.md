## What changed

<!-- Brief description and motivation. -->

## How it was verified

- [ ] `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --exclude pulsebench-desktop -- -D warnings`
- [ ] `cargo test --workspace --exclude pulsebench-desktop`
- [ ] `npm run typecheck && npm run test:ui`
- [ ] UI changes: screenshots attached (`npm run e2e -w apps/desktop`)
- [ ] Rust type changes: `npm run gen:types && npm run gen:schema` committed

## Benchmark content (if applicable)

- [ ] Suite `version` bumped, `pulsebench suite validate` passes, lock file refreshed, `CHANGELOG.md` updated

## Not verified

<!-- Be explicit: e.g. "not tested on Windows", "no NVIDIA GPU available". -->
