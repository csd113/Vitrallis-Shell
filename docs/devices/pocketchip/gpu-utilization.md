# PocketCHIP GPU utilization setup

The validated Debian 13 CHIP kernel binds Lima to `1c40000.gpu`. Its working OPP
configuration is a single `opp-hz = /bits/ 64 <297000000>` in an
`operating-points-v2` table referenced by `/soc/gpu@1c40000`. This enables devfreq
without introducing voltage changes or an unvalidated GPU frequency range.
The exact overlay was inspected on the USB-connected device in
`/home/chip/gpu-opp-overlay.dts`; the running tree and boot DTB agreed.

`GPU-HW-ACCELERATION` is already an ancestor of main. Its SDL renderer, Mesa
software rejection and diagnostics are reused. It contained no OPP setup or
utilization reader. Debug lives in the separate Vitrallis Apps repository.

## Installation and boot ownership

The normal session installer validates the complete bundle and matching helpers,
then invokes `platform-setup.py --install-user <desktop-user>` through sudo.
The account must have a private primary group. The helper requires Debian's
`device-tree-compiler` tools and the inspected CHIP U-Boot/flash-kernel layout;
unknown boot selection fails with an actionable error instead of guessing.

The helper checks the CRC-validated boot script to find the selected version's
DTB. It inspects both the running tree and existing DTBs. An existing valid OPP
table is retained byte-for-byte. Missing OPP data is merged into the existing
DTB using the device-validated overlay. Every pre-existing property is checked
for byte equality before publishing the result, preserving the PocketCHIP DIP,
display, touch, clocks, regulators and other board configuration.

Original DTBs are saved by SHA-256 under
`/var/lib/vitrallis-pocketchip/dtb-backups/`. A durable DTB transaction journal
allows an interrupted write to be recovered, refusing to overwrite a subsequently
edited tree. Atomic rename and directory fsync publish replacements. A kernel
post-install hook patches that kernel's source DTB **before** `zz-flash-kernel`
copies it into `/boot`; it does not pin an old whole-board DTB across kernels.

System-owned files:

- `/usr/libexec/vitrallis-pocketchip-gpu.py`: self-contained privileged setup.
- `/etc/kernel/postinst.d/zz-fd-vitrallis-gpu`: OPP integration for kernel installs.
- `/etc/systemd/system/vitrallis-gpu-trace.service`: boot oneshot, no resident helper.
- `/var/lib/vitrallis-pocketchip/`: account binding, status, backups and recovery.
- `/run/vitrallis-gpu/trace_pipe`: one read-only bind mount, recreated at boot.

The installer and Settings → Updates distinguish a shell relaunch from a required
system reboot. The boot service refreshes status from the running tree; telemetry
failure remains nonfatal to desktop startup. Normal user uninstall retains this
system support. `sudo systemctl disable --now vitrallis-gpu-trace.service` stops
only Vitrallis tracing and removes its runtime mount; it preserves GPU OPP data.

## Narrow trace access

The service creates `/sys/kernel/tracing/instances/vitrallis-gpu`, disables its
other events, selects the `nop` tracer and `mono` clock, and enables only
`devfreq/devfreq_monitor`, filtered to `dev_name == "1c40000.gpu"`. Its buffer is
16 KiB per CPU and wakes readers on available records. No global tracing setting
is written, consumed, enabled or disabled.

This kernel's tracefs does not support POSIX ACLs. Granting traversal through its
root would expose unrelated readable global trace files. Instead, only the
instance's `trace_pipe` is bind-mounted read-only at `/run/vitrallis-gpu/trace_pipe`.
Root owns it; the private desktop group may read it. The desktop cannot traverse
the global tracefs tree or alter event/filter controls. No group login refresh,
world-writable access, privileged desktop, polling subprocess or daemon is needed.

Debug discovers the bound Lima device through DRM/platform sysfs, then matches
that device's devfreq entry. Its provider reads `cur_freq` and uses the trace
`load=` field for utilization. It accepts 0%, rejects malformed/unrelated/stale
records, bounds partial records and uses readiness waits in a cancellable reader.
A file lock prevents multiple collectors consuming different parts of the same
stream. Closing Debug wakes and joins the reader. The root service can retain the
dedicated instance across application exits; global tracing is untouched.

Missing GPU provider, Lima without devfreq, unreadable tracing and working
utilization are separate states. Hardware identity and current frequency survive
missing samples. Runtime-suspended Lima may emit no records; old utilization
expires rather than being displayed as a current value.

## Verification

From the normal desktop account:

```sh
readlink -f /sys/bus/platform/devices/1c40000.gpu/driver
cat /sys/class/devfreq/1c40000.gpu/cur_freq
cat /var/lib/vitrallis-pocketchip/gpu-status.json
systemctl status vitrallis-gpu-trace.service
~/.local/share/vitrallis/current/vitrallis --graphics-test --renderer hardware
```

Expect Lima, a devfreq frequency around 297000000 Hz, and a configured trace
service. The renderer report should say `opengles2`, `Mali400`, SDL backbuffer,
VSync enabled and GL swap interval 1 verified. These are API checks; they do not
alone prove that a windowed surface reaches scanout without tearing. The normal
session also logs `compositor=picom backend=xrender buffering=present-pixmaps`
and owns an effects-free Picom process in `vitrallis-session.service`. An existing
compositor is retained and identified as externally managed. Software fallback
explicitly reports that synchronization is unverified.

Open Vitrallis Debug's GPU panel and start GPU Pulse. The panel should show Lima,
Mali-400 / detected device, MHz, changing utilization and `devfreq_monitor`.
Close Debug before using the exclusive command-line verifier:

```sh
sudo python3 -I /usr/libexec/vitrallis-pocketchip-gpu.py --verify
```

Run a GPU workload while verifying; an idle, suspended GPU can correctly produce
no fresh sample. The verifier checks binding, devfreq, current frequency and a
recent real load record. A reboot-required result must be handled before claiming
that a newly installed device tree is active.

## Physical validation record

The USB-connected Debian 13 device runs kernel `6.12.107+deb13-chip`, Mesa
`25.0.7-2+deb13u1`, SDL2 `2.32.4`, Xorg `21.1.16`, and Picom `12.5`. The native
bundle was built from this worktree without a version bump. The normal installer
passed using bundle SHA-256
`5487971547cd91f527b61634b47c34288478f0ab9270d9a48b41732cd3ce7aca`.

- OPP setup preserved the already configured boot DTB and patched the matching
  kernel source DTB. Repeated setup kept both byte-identical at SHA-256
  `461df83541b59baac9d21bcd603374a51bcf7be15d65580cf43facea2f123da7`.
- Normal-user GPU collection observed 0% and changing workload samples up to
  89%, at 297 MHz. The staged Debug GPU Pulse showed changing real values
  (including 37%, 63%, 50%, 85%, 92%, 86%, 74%) and closed its reader cleanly.
- Native Terminal, Notepad and Files hardware smoke tests and the shell graphics
  test passed on the installed bundle. All identified Mesa Mali400/GLES2.
- A moving-rectangle test initially tore despite accepted EGL swap interval 1.
  With Picom XRender/VSync the user reported **no visible tearing**. A temporary
  bounded interceptor of `xcb_wait_for_special_event` recorded 95 `FLIP`
  completions; median UST/MSC ratio was 16,800 microseconds (59.52 Hz).
  Five earlier `COPY` events were captured with the monitor powered off by DPMS.
- A quiet 10-second session interval measured 0.00 CPU seconds for Picom
  (3,604 KiB RSS), 0.01 for the supervisor and 0.05 for the shell. Stopping the
  session removed its Picom process; relaunch created a new supervised process.
  This is a short idle check, not a battery or long-duration benchmark.
- Coordinated matched Debug Pulse runs drew 168 frames with its overlay versus
  171 without it in eight seconds; the overlay reported 95–98% GPU load and
  297 MHz. Other device provisioning was paused for this comparison.
- A real reboot restored the enabled root oneshot, private read-only pipe,
  Lima binding and 297 MHz devfreq automatically. Both DTB hashes remained
  unchanged. UID 1000 then collected changing 7–97% load and closed its reader;
  it still could not read the global trace pipe. The device's existing startup
  route launched the installed shell and a fresh supervised Picom process,
  with the Mali400 hardware renderer ready. No provisioning command was needed
  after reboot. Debian cleared `/tmp`, so staged Debug validation files were
  uploaded again for the post-boot reader check.
- `sh scripts/validate.sh` passed: formatting, strict workspace Clippy, 235 Rust
  tests, 98 Python tests (two conditional skips), release build, native dummy
  renderer tests/smokes, documentation links and whitespace. The explicit
  ignored idle-loop test also passed. ARM hard-float cross-build and release
  bundle/version validation passed.

Raw evidence: [installation](evidence/gpu-vsync/install.txt),
[installed native tests](evidence/gpu-vsync/installed-native.txt),
[Present completions](evidence/gpu-vsync/present.txt),
[idle sample](evidence/gpu-vsync/idle.json),
[renderer modes](evidence/gpu-vsync/graphics.txt),
[post-boot state](evidence/gpu-vsync/postboot.txt),
[post-boot reader](evidence/gpu-vsync/postboot-reader.txt),
[automated checks](evidence/gpu-vsync/checks.txt),
and [changed files](evidence/gpu-vsync/changed-files.txt).

Debug source changes live in the separate Vitrallis Apps repository and were
tested as a staged normal-user app. They have not been published as an App Center
catalog release. Physical results cover this PocketCHIP; other hardware and
externally managed compositors need their own scanout verification.
