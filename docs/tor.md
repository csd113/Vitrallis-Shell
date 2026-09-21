# Shared Tor service

Vitrallis owns Tor policy, Settings and launch readiness. **Arti 2.6.0 is a
separate executable**, never a dependency linked into Shell. One supervised Arti
serves all applications through **127.0.0.1:9150**. Nothing listens on a LAN
address. Existing applications without networking metadata keep their behavior.

## Manifest and launch contract

An installed `app.toml` can contain:

```toml
[network]
tor = "required"
```

The other values are `preferred` and `none` (the default when the table is absent).
Invalid values, types, misspellings and duplicate keys fail manifest validation.
The `network` table is the extension point for future networking capabilities;
unknown capabilities are rejected until the Shell implements them.

- **required:** request the shared service, wait for bootstrap and launch through
  Bubblewrap's fresh user, PID and network namespaces. The namespace has only
  loopback; a bounded relay connects its local SOCKS endpoint to the shared Arti
  through a private Unix socket. IPv4, IPv6 and UDP/DNS cannot route directly to
  the host network. Missing isolation, disabled Tor or failed bootstrap fails
  closed. There is no direct-exec fallback. Exported desktop launchers enforce
  the same isolation and refuse launch unless the shared service is ready.
- **preferred:** request and wait for Tor, then provide availability and proxy
  environment variables. If Tor fails, the application receives availability 0
  and can apply its own fallback policy. Shell does not implement that policy.
- **none:** no Tor service lease or networking changes.

Applications must actually support SOCKS5 and resolve destination names through
SOCKS (`socks5h`, not a local DNS lookup). This design protects against accidental
clearnet fallback. It is not a hostile-application sandbox: applications share a
user account, writable files and the X11 desktop. It does not promise isolation
from malicious same-user processes or anonymity against application-level leaks.

An application launched with a Tor requirement receives:

| Environment variable | Meaning |
| --- | --- |
| `VITRALLIS_TOR_API` | Contract version `1` |
| `VITRALLIS_TOR_AVAILABLE` | `1` only when connected at launch |
| `VITRALLIS_TOR_SOCKS_HOST` | `127.0.0.1` |
| `VITRALLIS_TOR_SOCKS_PORT` | `9150` |
| `VITRALLIS_TOR_STATE` | Lowercase service state at launch |
| `VITRALLIS_TOR_STATUS_SOCKET` | Live local status socket |

When connected, Shell supplies `ALL_PROXY`, `HTTP_PROXY`, `HTTPS_PROXY`, their
lowercase forms, and empty `NO_PROXY`/`no_proxy`. These are convenience settings;
the required-app network namespace provides the clearnet barrier.

Connect to the status Unix socket to receive one newline-terminated JSON object
(up to 1024 bytes). For example:

```json
{"api":1,"available":true,"socks_host":"127.0.0.1","socks_port":9150,"state":"connected","progress":100}
```

An absent/disconnected socket means unavailable. Environment values are a
launch-time snapshot; do not treat them as a permanent health guarantee. Arti's
functional-proxy notification plus a SOCKS handshake establishes readiness,
not PID existence. Connected means bootstrapped with a functional local proxy;
it cannot guarantee a particular destination or an uninterrupted Internet path.
Applications must handle failed Tor requests without bypassing their requirement.

## Lifecycle and resource use

The default **On demand** mode sleeps until an app requests Tor or the user
selects Start. Active and pending Tor applications hold service leases. After
the last app exits, a 30-second grace period avoids repeated bootstrap for quick
reopens. **Always on** starts with Shell. **Disabled** stops the service and
rejects required launches. Manual Start keeps Tor running until Stop or a mode
change; Stop suppresses automatic restart until a new request or explicit Start.

A dedicated Rust worker owns the small Python guardian, which owns Arti. The
worker uses a bounded command/status channel. UI rendering does no process,
network or disk work. With no service and no pending retry, the worker blocks
without a timer; no guardian or Arti process exists. Active supervision checks
process exit once per second. Status changes request redraws; unchanged Tor state
does not drive animation or continuous frames.

The guardian holds an exclusive OS file lock inherited by Arti. Shell lifetime
is a pipe lease: EOF stops and reaps Arti. Linux parent-death signaling also
terminates Arti if the guardian itself dies. Controlled shutdown allows two
seconds before killing Arti. No PID files are trusted or used to kill processes.
An occupied SOCKS port is reported as an error; unrelated processes are neither
adopted nor killed. A subsequent start recreates stale owned Unix sockets under
the lock. Crash recovery retries at 2, 4 and 8 seconds, then requires explicit
Start. Bootstrap has a 180-second deadline; app launch has a 190-second bound.

Arti output is consumed in bounded chunks, not written to an accumulating log.
Only bootstrap progress, readiness and guardian-generated diagnostics leave the
guardian. Destinations, SOCKS credentials and raw circuit logs are not retained.
Status events carry a generation so an old process cannot overwrite a restart's
state. A crash does not change configuration or delete the Arti cache/state.

## Settings

Open **Settings → Wireless Network → Tor**. The 480×272 summary shows support,
connection/bootstrap, service, endpoint, application count and startup mode.
Start, Stop, Restart, Enable/Disable, Startup and Details share keyboard/touch
activation. Arrows move the visible selection; Enter activates; Escape returns.
Details includes the most recent diagnostic. Existing power/install confirmation
defaults and other Settings controls are unchanged.

## Paths and installation

- `${XDG_CONFIG_HOME:-$HOME/.config}/vitrallis/tor.json`: startup selection,
  atomic writes, mode 0600. Example: `{"startup":"on-demand"}`.
- `${XDG_DATA_HOME:-$HOME/.local/share}/vitrallis/tor/`: private 0700 service root.
- `arti.toml`: generated fixed-loopback Arti configuration, atomic 0600 replacement.
- `cache/` and `state/`: Arti-only data, separate from application/wallet data.
- `service.lock`, `proxy.sock`, `status.sock`: owned supervision and local IPC.

Malformed startup files fail closed and are not silently reset. Paths reject
symlink traversal, unsafe permissions and invalid components. The guardian uses
umask 077. No shell interpolates app arguments or Arti paths.

The normal v2 release bundle contains Shell, Terminal, Notepad, Files and Arti,
with the existing per-executable size, ABI and SHA-256 verification. The release
workflow builds Arti independently using the publisher's locked dependency graph:

```sh
sh scripts/build-arti.sh
# Cross build with the existing ARM linker/toolchain environment:
sh scripts/build-arti.sh armv7-unknown-linux-gnueabihf target/armv7-unknown-linux-gnueabihf/release
```

The compact build uses Tokio, Rustls/ring, static SQLite, compression, onion
service client support and process hardening. It avoids a target OpenSSL/SQLite
runtime dependency. No Arti crate is added to the Shell workspace. PocketCHIP
bootstrap installs `bubblewrap` through the existing guarded APT mechanism. The
installer verifies Arti's exact version and creates private default Tor paths
and on-demand configuration transactionally, preserving an existing valid mode.
Custom XDG paths are initialized by the service on first use. App Center never
provisions per-app Arti copies. Uninstall preserves Tor state with other user
application data. No swap or new desktop daemon is required.

See [beta4 upgrade paths](beta4-upgrade.md) for existing installations.

## Troubleshooting and validation

- **Arti missing:** repair the complete five-executable Vitrallis bundle.
- **Tor disabled:** Enable, then choose On demand or Always on.
- **Another Shell owns Tor / occupied endpoint:** close the other service owner;
  do not delete its lock file or kill an arbitrary PID.
- **Bootstrap timeout:** check connectivity, device clock and censorship. This
  panel does not configure bridges or expose arbitrary Arti options.
- **Required app cannot launch:** confirm Bubblewrap and unprivileged user/network
  namespaces work. Unsupported hosts fail closed; there is no clearnet fallback.
- **Malformed configuration:** inspect `tor.json`; valid startup values are
  `on-demand`, `always-on`, `disabled`. Details reports configuration failures.

Normal tests use fake processes and a local fake SOCKS server, with no Tor network.
Optional real-network test:

```sh
VITRALLIS_TEST_ARTI="$PWD/target/release/arti" python3 -m unittest discover -s tests -p test_tor.py
```

See [Tor validation](tor-validation.md) for executed checks and device evidence.
Upstream references: [Arti configuration](https://arti.torproject.org/contributing/for-developers/config-options/)
and [Arti proxy implementation](https://arti.torproject.org/coverage/unit/crates/arti/src/subcommands/proxy.rs.html).
