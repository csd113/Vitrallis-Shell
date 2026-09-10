#!/bin/sh
# Host gates. Native SDL2 development libraries and pkg-config are prerequisites.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
cargo test --locked --workspace --all-features
python3 -m unittest discover -s tests -p 'test_*.py'
cargo build --locked --release --workspace --all-features
SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test
sh -n scripts/run-pocketchip.sh scripts/build-pocketchip.sh
git diff --check
