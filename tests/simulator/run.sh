#!/bin/sh
set -eu
test -f /.dockerenv
collect_artifacts() {
    mkdir -p target/app-center-audit/docker
    if test -d /sim/artifacts; then cp -R /sim/artifacts/. target/app-center-audit/docker/; fi
    if test -f /sim/requests.jsonl; then cp /sim/requests.jsonl target/app-center-audit/docker/; fi
}
trap collect_artifacts 0
sh tests/simulator/start.sh
cargo build --locked --workspace --all-features
cargo test --locked --workspace --all-features
/usr/bin/python3 -m unittest discover -s tests -p 'test_*.py'
dbus-run-session -- /usr/bin/python3 tests/simulator/session.py
/usr/bin/python3 tests/simulator/lifecycle.py
cp /sim/artifacts/scenarios.json /sim/artifacts/scenarios-fixtures.json
if test "${VITRALLIS_TEST_PUBLISHED:-0}" = 1; then
    git config --global --add safe.directory /workspace/target/app-center-audit/published-repo
    /usr/bin/python3 tests/simulator/published.py
    cp /sim/artifacts/scenarios.json /sim/artifacts/scenarios-published.json
fi
