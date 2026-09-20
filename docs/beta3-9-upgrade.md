# beta3.9 update bridge

This four-executable release can be installed by existing managed Shell builds.
It prepares the updater for beta4's five-executable v2 bundle, including shared
Arti 2.6.0 with its independent version. It does not include the Tor service.

Use Settings → Device → Updates to install and relaunch beta3.9. Once beta4 is
published, use Updates again to install its complete five-executable generation.
Applications and user configuration are preserved by the existing atomic update
transaction. No manual reinstallation is needed.

Older clients always select the highest published version, so beta4 also publishes
a four-executable entry point for users who skip this bridge. Such users relaunch
beta4 and repeat Check/Install/Relaunch once to fetch the complete v2 bundle.
The updater permits this same-version completion only when the managed generation
is missing Arti; unsafe files and downgrades are rejected. New installations use
the full beta4 installer and v2 bundle directly.
