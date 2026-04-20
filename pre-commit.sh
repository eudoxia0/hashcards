#!/bin/sh

set -euo pipefail

cargo fmt --all -- --check
cargo check --locked
cargo clippy --locked -- -D warnings
cargo test --locked
cargo machete
cargo deny check licenses
