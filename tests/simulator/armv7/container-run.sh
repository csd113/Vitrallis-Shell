#!/bin/sh
# Executed inside the ARMv7 runtime container. The cross-built binaries are
# mounted read-only at /opt/vitrallis; the checkout is at /workspace (read-only)
# for the Python installer tests; results are written to /results.
#
# This exercises the real production Rust/Python code under ARMv7 Linux
# emulation. It is not physical PocketCHIP hardware validation.
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
stage=${VITRALLIS_STAGE:-/opt/vitrallis}
results=${VITRALLIS_RESULTS:-/results}
version=${VITRALLIS_VERSION:?missing VITRALLIS_VERSION}
mkdir -p "$results/logs" "$results/frames"
: >"$results/failures.txt"
failures=0
passes=0
if [ -d "$stage/native-fixtures" ]; then
    export VITRALLIS_QA_NATIVE_FIXTURES="$stage/native-fixtures"
fi

as_chip() {
    # umask first (devices commonly use 002), then run the command as the
    # unprivileged account with an isolated home and the dummy SDL driver.
    umask_value=$1
    shift
    runuser -u chip -- /bin/sh -c "umask $umask_value; exec \"\$@\"" sh "$@"
}
record() {
    label=$1
    log=$2
    if shift 2 && "$@" >"$log" 2>&1; then
        passes=$((passes + 1))
        echo "PASS  $label" >>"$results/summary.txt"
    else
        failures=$((failures + 1))
        echo "FAIL  $label  (see $(basename "$log"))" >>"$results/summary.txt"
        echo "$label" >>"$results/failures.txt"
    fi
}

: >"$results/summary.txt"
{
    echo "environment: $(uname -m) $(getconf GNU_LIBC_VERSION)"
    echo "distribution: $(cat /etc/os-release | sed -n 's/^PRETTY_NAME=//p')"
    echo "python: $(python3 --version 2>&1)"
    dpkg-query -W -f='package: ${Package} ${Version}\n' libsdl2-2.0-0 libc6 2>/dev/null || true
} >"$results/environment.txt"

# 1. Architecture, dynamic linkage and ABI of the produced executables.
{
    for name in vitrallis vitrallis-terminal vitrallis-notepad vitrallis-files arti; do
        if [ -x "$stage/release/$name" ]; then
            file "$stage/release/$name"
        fi
    done
    echo
    echo "--- ldd vitrallis"
    ldd "$stage/release/vitrallis" || true
    echo "--- readelf header (vitrallis)"
    readelf -h "$stage/release/vitrallis" | sed -n '1,20p'
    echo "--- readelf NEEDED"
    readelf -d "$stage/release/vitrallis" | sed -n '/NEEDED/p'
    if [ -x "$stage/release/arti" ]; then
        echo "--- ldd arti"
        ldd "$stage/release/arti" || true
        echo "--- readelf NEEDED (arti)"
        readelf -d "$stage/release/arti" | sed -n '/NEEDED/p'
    fi
} >"$results/architecture.txt" 2>&1

# 2. Version probes for every bundled executable, as the device user.
for name in vitrallis vitrallis-terminal vitrallis-notepad vitrallis-files; do
    record "version $name" "$results/logs/version-$name.log" \
        as_chip 002 "$stage/release/$name" --version
done
if [ -x "$stage/release/arti" ]; then
    record "version arti" "$results/logs/version-arti.log" \
        as_chip 002 "$stage/release/arti" --version
fi

# 3. Headless SDL startup at the PocketCHIP resolution (and the 800x480 test
#    size) with an unprivileged user and no display server.
for size in 480x272 800x480; do
    record "shell frame $size" "$results/logs/shell-$size.log" \
        as_chip 002 env SDL_VIDEODRIVER=dummy "$stage/release/vitrallis" \
        --demo --size "$size" --screenshot "$results/frames/vitrallis-$size.bmp"
done
for app in vitrallis-terminal vitrallis-notepad vitrallis-files; do
    for size in 480x272 800x480; do
        record "app $app $size" "$results/logs/app-$app-$size.log" \
            as_chip 002 env SDL_VIDEODRIVER=dummy "$stage/release/$app" \
            --size "$size" --smoke-test
    done
done

# 4. Cross-built test executables: the actual production filesystem, lock,
#    generation, update and restore code compiled for ARMv7.
shell_tests=$(awk -F"$(printf '\t')" '$2 == "vitrallis_shell" {print $1; exit}' "$stage/tests.manifest" 2>/dev/null || true)
# User-mode emulation differs from native ARM execution in two ways that these
# tests depend on: the binfmt handler execs qemu-arm, so /proc/<pid>/exe never
# matches the target binary, and exec of a missing file appears to start and
# then fails inside the loader. The tests below are valid on hardware but can
# never pass under emulation; any other failure still fails the run.
cat <<'EOF' >"$results/emulation-artifacts.txt"
app_center::native_tests::native_install_launch_process_detection_update_and_uninstall|Native app was not detected
app_center::tests::running_app_identity_is_rechecked_and_only_exact_script_is_closed|Test process not identified
app_center::tests::uninstall_refuses_a_running_app_without_removing_files|running app not detected
process::tests::failed_spawn_allows_retry|missing entry must not start
shortcuts::tests::vanished_executable_cwd_and_permissions_return_to_a_dismissible_error|left: Launching
EOF
emulation_marker_matches() {
    name=$1
    log=$2
    line=$(grep -F -- "$name|" "$results/emulation-artifacts.txt" || true)
    [ -n "$line" ] || return 1
    # Scope markers to the failing test's own output (stdout plus its panic
    # block) so an unrelated failure elsewhere in the suite cannot satisfy the
    # expected reason.
    section="$results/logs/.emulation-section"
    awk -v name="$name" '
        $0 == "---- " name " stdout ----" { capture = 1; next }
        capture && (/^---- / || /^failures:$/ || /^test result:/) { capture = 0 }
        capture { print }
    ' "$log" >"$section"
    old_ifs=$IFS
    IFS='|'
    for part in $line; do
        if [ "$part" != "$name" ] && [ -n "$part" ] && grep -qF -- "$part" "$section"; then
            IFS=$old_ifs
            rm -f "$section"
            return 0
        fi
    done
    IFS=$old_ifs
    rm -f "$section"
    return 1
}
run_tests() {
    binary=$1
    label=$2
    shift 2
    log="$results/logs/$label.log"
    if as_chip 002 "$binary" --test-threads=2 "$@" >"$log" 2>&1; then
        passes=$((passes + 1))
        echo "PASS  $label" >>"$results/summary.txt"
        return
    fi
    if [ "$label" = "rust-suite-$shell_tests" ]; then
        failed=$(awk '/^failures:$/{capture=1;next} /^test result:/{capture=0}
                      capture && /^[[:space:]]{4}[a-zA-Z0-9_:]+$/ {print $1}' "$log" | sort)
        if [ -n "$failed" ]; then
            unexpected=""
            for name in $failed; do
                if ! emulation_marker_matches "$name" "$log"; then
                    unexpected="$unexpected $name"
                fi
            done
            if [ -z "$unexpected" ]; then
                passes=$((passes + 1))
                {
                    echo "PASS  $label (only documented user-mode emulation artifacts)"
                    for name in $failed; do
                        echo "  emulation-only: $name"
                    done
                } >>"$results/summary.txt"
                return
            fi
        fi
    fi
    failures=$((failures + 1))
    echo "FAIL  $label  (see $(basename "$log"))" >>"$results/summary.txt"
    echo "$label" >>"$results/failures.txt"
}
if [ -f "$stage/tests.manifest" ]; then
    while IFS="$(printf '\t')" read -r name _target _directory; do
        [ -n "$name" ] || continue
        run_tests "$stage/tests/$name" "rust-suite-$name"
    done <"$stage/tests.manifest"
else
    echo "no cross-built test manifest" >>"$results/failures.txt"
    failures=$((failures + 1))
fi

# 5. Focused generation/restore/update/lock tests under both common umasks,
#    reusing the same production test executables.
if [ -n "$shell_tests" ]; then
    for umask_value in 002 022; do
        for filter in platform::update updater:: app_center::storage; do
            safe=$(printf '%s' "$filter" | sed 's/[^a-zA-Z0-9]/_/g')
            record "umask$umask_value $filter" "$results/logs/umask$umask_value-$safe.log" \
                as_chip "$umask_value" "$stage/tests/$shell_tests" "$filter"
        done
    done
    # The real five-executable ARM bundle through the production unpack,
    # verification and generation-commit path (skipped when Arti is disabled).
    bundle="$stage/release-assets/vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36-v2.vtrbundle"
    if [ -f "$bundle" ]; then
        record "release bundle upgrade probe" "$results/logs/bundle-probe.log" \
            as_chip 002 env \
            VITRALLIS_TEST_UPDATE_BUNDLE="$bundle" \
            VITRALLIS_TEST_UPDATE_VERSION="$version" \
            "$stage/tests/$shell_tests" --exact \
            platform::update::unix::tests::release_bundle_upgrade_probe --nocapture
    else
        echo "release bundle missing; probe skipped" >>"$results/summary.txt"
    fi
fi

# 6. The Python installer/updater suites on ARMv7 Debian. They use mocked
#    transport and session boundaries; this checks the device Python version
#    and architecture rather than contacting any network.
if [ -d /workspace/tests ]; then
    record "python installer tests (ARMv7)" "$results/logs/python-tests.log" \
        as_chip 002 env HOME=/tmp/chip-python PYTHONDONTWRITEBYTECODE=1 \
        /bin/sh -c 'cd /workspace && exec python3 -m unittest discover -s tests -p "test_*.py"'
fi

# 7. A persistent simulated generation layout owned by the device-like user.
#    The digest is computed exactly as the production bundle writer does, but
#    this hand-built tree is layout evidence only; production installation
#    logic is covered by the probe and test suites above.
python3 - "$stage" "$results" <<'PY'
import hashlib, os, shutil, struct, sys
from pathlib import Path
stage, results = Path(sys.argv[1]), Path(sys.argv[2])
home = Path("/home/chip/vitrallis")
shutil.rmtree(home, ignore_errors=True)
names = ("vitrallis", "vitrallis-terminal", "vitrallis-notepad", "vitrallis-files")
whole = hashlib.sha256()
whole.update(b"VITRALLIS-BUNDLE")
for name in names:
    data = (stage / "release" / name).read_bytes()
    whole.update(struct.pack("<Q", len(data)))
    whole.update(hashlib.sha256(data).digest())
    whole.update(data)
generation = home / "generations" / whole.hexdigest()
generation.mkdir(parents=True)
for name in names:
    shutil.copy2(stage / "release" / name, generation / name)
os.symlink(Path("generations") / generation.name, home / "current")
layout_note = results / "simulated-layout.txt"
layout_note.write_text(
    "hand-built layout (not production installer output)\n" + os.popen(f"ls -la {home} {generation}").read()
)
print(f"simulated generation: {generation.name}", file=sys.stderr)
PY
chown -R chip:chip /home/chip/vitrallis
if [ -f "$results/simulated-layout.txt" ]; then
    record "shell from simulated current pointer" "$results/logs/simulated-current.log" \
        as_chip 002 env HOME=/home/chip \
        /home/chip/vitrallis/current/vitrallis --version
fi

{
    echo "passes=$passes failures=$failures"
    cat "$results/failures.txt"
} >"$results/result.txt"
cat "$results/result.txt"
[ "$failures" -eq 0 ]
