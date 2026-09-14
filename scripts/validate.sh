#!/bin/sh
# Host gates. Native SDL2 development libraries and pkg-config are prerequisites.
set -eu
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo check --workspace --locked
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
cargo test --locked --workspace --all-features
python3 -m unittest discover -s tests -p 'test_*.py'
cargo build --locked --release --workspace --all-features
native_target_dir=$(cargo metadata --no-deps --locked --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
VITRALLIS_RENDERER_BIN_DIR="$native_target_dir/release" python3 -m unittest discover -s tests -p 'test_native_renderer.py'
SDL_VIDEODRIVER=dummy cargo run --locked -- --smoke-test
for app in vitrallis vitrallis-terminal vitrallis-notepad vitrallis-files; do
    test -x "$native_target_dir/release/$app"
    "$native_target_dir/release/$app" --version
done
for app in vitrallis-terminal vitrallis-notepad vitrallis-files; do
    SDL_VIDEODRIVER=dummy "$native_target_dir/release/$app" --size 480x272 --smoke-test
    SDL_VIDEODRIVER=dummy "$native_target_dir/release/$app" --size 800x480 --smoke-test
done
for script in scripts/*.sh integrations/pocketchip/*.sh; do
    sh -n "$script"
done
python3 -m compileall -q scripts integrations
python3 scripts/check-doc-links.py
git diff --check
