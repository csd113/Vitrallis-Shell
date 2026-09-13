# Stock PocketHome source contract

Source inspection on 2026-09-12 used the upstream NextThingCo repository, cloned
outside this worktree. No device was contacted. This verifies the launcher contract,
not that its original OS image meets Vitrallis's current runtime requirements.
Debian 12+, armhf, glibc 2.36+, SDL2 2.26.5+, Awesome 4.x and a working systemd user
manager remain required by the ARM installation helpers.

## Pinned evidence

The original stock revision is
[`28b6e389fd0a51a1c135c614990e08183a86a7bf`](https://github.com/NextThingCo/PocketCHIP-pocket-home/tree/28b6e389fd0a51a1c135c614990e08183a86a7bf).
The inspected current upstream revision is
[`65f7c772cd02096cb2dc3409c90238c546631f2a`](https://github.com/NextThingCo/PocketCHIP-pocket-home/tree/65f7c772cd02096cb2dc3409c90238c546631f2a).
These are separate from the modified launcher in the preserved historical reports.

- Stock [`Main.cpp`](https://github.com/NextThingCo/PocketCHIP-pocket-home/blob/28b6e389fd0a51a1c135c614990e08183a86a7bf/Source/Main.cpp)
  loads `assetFile("config.json")` at startup. It has no desktop-entry scanner or
  per-user config loader.
- Stock [`Utils.cpp`](https://github.com/NextThingCo/PocketCHIP-pocket-home/blob/28b6e389fd0a51a1c135c614990e08183a86a7bf/Source/Utils.cpp)
  selects `/usr/share/pocket-home/`, with a development-assets fallback. Its user
  asset override is an unimplemented TODO. Vitrallis uses that system config by
  default and permits explicit exported fixtures via `--app-config`/`--assets`.
- Stock [`AppsPageComponent.cpp`](https://github.com/NextThingCo/PocketCHIP-pocket-home/blob/28b6e389fd0a51a1c135c614990e08183a86a7bf/Source/AppsPageComponent.cpp)
  launches `shell` with JUCE ChildProcess. It uses `xdotool` and Awesome for refocus;
  this does not mean a JSON command receives shell expansion. Vitrallis keeps
  explicit argv semantics and uses its own process ownership and window handling.
- The stock app list has no stable utility IDs. Its
  [`config.json`](https://github.com/NextThingCo/PocketCHIP-pocket-home/blob/28b6e389fd0a51a1c135c614990e08183a86a7bf/assets/config.json)
  identifies the terminal by `vala-terminal -fs 8 -g 20 20`, editor by `leafpad`,
  and file browser by `pcmanfm`.
- Current upstream's
  [`config.json`](https://github.com/NextThingCo/PocketCHIP-pocket-home/blob/65f7c772cd02096cb2dc3409c90238c546631f2a/assets/config.json)
  uses `lxterminal`, `l3afpad`, and `pcmanfm`; its separate Wi-Fi command is
  `lxterminal -e nmtui`. That argument-bearing command remains visible.

The [stock](../../../tests/fixtures/pockethome/stock.json) and
[current](../../../tests/fixtures/pockethome/current.json) fixtures record the
factual command inventories, normalized to strict JSON with only Apps items and
the current Wi-Fi command. They are not renamed modified-launcher fixtures. No
upstream code, icons, fonts or other artwork is copied into Vitrallis.

## Installation and return

Adding an item to `~/.pocket-home/config.json` cannot register Vitrallis with stock
PocketHome. The current installer therefore leaves both launcher configs untouched.
It installs an opt-in desktop shortcut and the user-local `launch` command. Start
that command from stock Terminal in the existing graphical session. Desktop menus
that consume `.desktop` files can also show the shortcut; stock PocketHome does not.
No root menu edit or replacement launcher is necessary.

Home routing uses Awesome 4's root keys and client APIs, without assuming a global
`focus_home_screen` or `launch_home_screen` function. It saves the previously focused
window, temporarily replaces the unmodified XF86PowerOff binding, and restores only
its own changes. Exiting the shell or stopping its owned unit restores routing and
raises the saved window if it still exists. Systemd ExecStopPost repeats restoration
safely on supervisor failure. Physical key delivery and the target's display-manager
and systemd integration still require fresh hardware validation.

## Discovery identity

Filtering is confined to the PocketHome importer. It matches the complete verified
commands above (bare executable or exact `/usr/bin/` path), including exact arguments.
If PATH resolves a bare command outside `/usr/bin`, it is a custom shadow and is kept.
Missing stock executables are still suppressed by the verified menu signature. Names,
translations, icons and substrings do not identify duplicates. Additional arguments,
custom absolute paths, other stock applications and App Center packages are kept.
Native utility registration runs independently and retains missing-binary repair errors.

## Remaining brand literals

The final audit must retain these intentional exceptions:

- `docs/devices/pocketchip.md` and `docs/devices/pocketchip/`: device setup, source
  citations, original historical reports, screenshots and logs. Historical command
  names and third-party binary hashes are evidence, not current install instructions.
- Links to that device-specific documentation from general docs and source comments.
- `/usr/local/bin/pocketchip-calibration`: an external OS utility, isolated in the
  `CALIBRATION` constant. Renaming it would break the installed utility. It is optional.
- `/usr/share/pocketchip-localdoc/index.html` in the stock inventory fixture: the
  upstream Help command, deliberately preserved and never filtered as a utility.
- External upstream repository URLs and the original Marshmallow historical audit.
  Those names record provenance; no runtime prerequisite depends on them.

The current Vitrallis flag is `--linux-handheld`, the Rust backend is
`platform::linux_handheld`, the cross-build command is `scripts/build-armhf.sh`, and
session helpers live under `integrations/pocketchip/`. No old flag aliases or
pre-release migration branches are supplied. Legal notices and historical evidence
are preserved; generic Linux mode does not require either launcher.
