# Beta trust model and hardening review

Vitrallis is a normal-user launcher, **not an application sandbox**. Installed
apps, their runtimes and locally reviewed manifests can access the same user's
files, display, environment and network. Store checksums verify source integrity
against a selected GitHub commit/blob; they are not independent signatures and
do not protect against a compromised trusted repository/account. Review new
catalogue entries and installer adapters before use. No root permission is
required for the launcher, Store patch or user session installer.

## Boundaries audited for this candidate

- App metadata: bounded 1 MiB JSON input, validated labels/paths/arguments/env,
  regular-file checks, bounded installer reads, malformed-entry diagnostics, last-valid catalogue retained
  on failed refresh. Duplicate ID validation uses an ordered set instead of quadratic scanning. PNG/BMP inputs have file/dimension/decoder limits; retained
  icon textures have a 16 MiB budget, after which placeholders are used.
- Execution: explicit argv and per-child process groups. Manifest values are not
  interpolated into a shell. App arguments are logged, so do not put secrets in
  them. Environment values are not logged; application/runtime inheritance is
  intentional for existing GUI/Tk compatibility, not isolation.
- Lifecycle: launch guards, running-tile identity, asynchronous bounded window
  resume, direct-child reaping and cleanup. Helper timeouts clean their process
  groups, including descendants holding stdout. System status runs off the UI
  thread with bounded queues/output and stale-data expiry.
- Install/patch: reviewed SHA-256 bundle, validation before replacing application
  files, same-directory atomic writes and directory fsync, backups, persistent
  incomplete markers and rerun repair. Concurrent launcher installers use a lock.
  Store uses its existing updater lock. Changed files are rechecked and rollback
  preserves detected concurrent edits. Markers including dangling symlinks prevent
  launcher execution. No archive extraction or general remote install-script
  execution is present in the owned launcher tooling.
- Files: symlink and hardlink rejection for mutation targets; log creation uses
  `O_NOFOLLOW`, descriptor type/link checks and private permissions. Two rotating
  128 KiB logs bound session output. No device keys, sysroot libraries, build
  caches or raw validation logs are release source.
- Session: optional user systemd unit, cgroup cleanup on launcher/supervisor
  crashes, temporary Awesome Home binding and restoration. Marshmallow's binary,
  original Home configuration and serial recovery remain available.

## Limits

These checks are not a security boundary against another hostile process
running as the same user. Some path checks and filesystem replacements remain
TOCTOU-sensitive to concurrent hostile ancestor-directory swaps; use private,
user-owned installation directories on a local filesystem. GUI focus class/title
fallbacks for the two reviewed Tk applications are compatibility hints, not
proof of process identity or permission to kill an unrelated process.

Catalogue refresh and image decoding still perform bounded local filesystem
work on the UI thread. Normal local files are validated; a stalled filesystem or
hostile path replacement can still stall those operations. Status polling and
resume commands are independently bounded/off-thread. An OS/kernel stall can
outlive a userspace timeout. There is no promise of zero latency or containment
for arbitrary malicious native code.

Direct manual binary invocation cannot contain descendants that escape process
groups or survive a forced launcher termination. Use the supervised installed
session for normal use; it owns the entire cgroup. The independent Marshmallow
process is outside that cgroup. User power operations still require existing
system authorization; Vitrallis does not install privilege rules.

RustSec reported no known advisories in the selected Rust graph on 2026-09-10.
See [dependency exceptions](dependencies.md) and [release validation](release-candidate.md)
for remaining constraints and actual evidence. Production readiness requires a
versioned signed package format, stronger install transaction recovery, a public
hardware SDK/identity protocol, additional target coverage and endurance testing.
