# Project instructions

- Never bump or otherwise change version numbers without the user's explicit
  permission. Authorization for a named release applies only to that release;
  it does not authorize future version bumps.
- Keep all native system-service screens usable with keyboard and touch. Every
  visible navigation/action button must have a reachable, visible keyboard
  selection and use the same activation behavior as touch. Preserve safe
  confirmation defaults for power and installation actions.

- Vitrallis is pre-release. Backward compatibility with obsolete pre-release
  builds is not a project requirement unless explicitly requested. When an
  internal API, package format, path, configuration format, architecture, or
  behavior is replaced, update current consumers and remove the superseded
  implementation. Do not introduce compatibility layers, legacy fallbacks,
  aliases, dual code paths, migration shims, deprecated formats, or version-gated
  support unless the task specifically requires them.
