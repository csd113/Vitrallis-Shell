# Security reporting

Vitrallis is beta software running with the desktop user's permissions.
Applications are not sandboxed. Read the [trust model](docs/security.md) before
installing packages or testing untrusted input.

## Contact route

The repository's [GitHub issue tracker](https://github.com/csd113/Vitrallis-Shell/issues)
is enabled. GitHub private vulnerability reporting is **disabled**, verified
through the repository API on 2026-09-12. No private reporting address is published
here.

For a sensitive vulnerability, open a minimal issue titled **Security contact
request**, asking the maintainer to arrange a private channel. **Do not include
vulnerability details, exploit code, credentials, personal data, or sensitive logs
in that public issue.** Wait for an agreed private channel before sharing details.
Ordinary non-sensitive bugs can use the bug form.

Once a private channel is established, include the affected version/commit,
platform, impact, a minimal reproduction, and any proposed fix. No response-time
or support-window guarantee is offered. Maintainers should enable GitHub private
vulnerability reporting before broader distribution and update this policy when
a private route is available.

## Scope

Review the current source and complete-bundle release contract. Obsolete
pre-release formats are not maintained. Never test against someone else's device,
account, data, or catalog without authorization. Installation checksums establish
integrity against the selected HTTPS release; they are not independent signatures.
