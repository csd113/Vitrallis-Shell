#!/usr/bin/env python3
"""Stage cross-built ARMv7 test executables for the emulated runtime container.

Reads `cargo test --no-run --message-format=json` output and copies every test
executable into /target/armv7-stage/tests plus a tab-separated manifest.
Only the cross target's artifacts are accepted.
"""
import json
import shutil
import sys
from pathlib import Path

STAGE = Path("/target/armv7-stage")
TARGET_MARKER = "/armv7-unknown-linux-gnueabihf/"


def main() -> int:
    stage = STAGE / "tests"
    stage.mkdir(parents=True, exist_ok=True)
    manifest = []
    seen = set()
    for line in sys.stdin:
        line = line.strip()
        if not line.startswith("{"):
            continue
        message = json.loads(line)
        if message.get("reason") != "compiler-artifact":
            continue
        profile = message.get("profile") or {}
        target = message.get("target") or {}
        executable = message.get("executable")
        if not executable or profile.get("test") is not True:
            continue
        if TARGET_MARKER not in executable:
            raise SystemExit(f"refusing non-ARMv7 test artifact: {executable}")
        source = Path(executable)
        package = message.get("package_id", "").split("#")[-1].split("@")[0].replace("/", "-")
        unique = f"{package}-{target.get('name', 'test')}"
        destination = stage / unique
        if unique in seen:
            continue
        seen.add(unique)
        shutil.copy2(source, destination)
        manifest.append((unique, target.get("name", ""), source.parent.name))
    if not manifest:
        raise SystemExit("no ARMv7 test executables were produced")
    lines = ["\t".join(row) for row in sorted(manifest)]
    (STAGE / "tests.manifest").write_text("\n".join(lines) + "\n")
    print(f"staged {len(manifest)} ARMv7 test executables")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
