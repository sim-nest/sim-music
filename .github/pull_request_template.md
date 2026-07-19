## What this changes

<!-- One or two sentences on the change and why. -->

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo doc --workspace --no-deps` passes
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] `cargo run -p xtask -- simdoc --check` passes
- [ ] Tests added/updated for the behavior changed
- [ ] Source and Markdown are ASCII-only
- [ ] Commits are signed off (DCO: `git commit -s`)
