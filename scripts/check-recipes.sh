#!/usr/bin/env sh
set -eu

# Workspace tests exercise every embedded recipe conformance module as well as
# the root foundry application's alternate-provider and failure-path tests.
cargo test --workspace --quiet
cargo run --quiet -p music-algorithm-foundry

printf 'check-recipes: OK (embedded music recipes + algorithm foundry application)\n'
