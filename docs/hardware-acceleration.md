# Hardware acceleration

Vitrallis uses one shared renderer selection path for Shell, Terminal, Notepad and
Files: **Vitrallis → SDL2 → Mesa → the kernel DRM driver → the GPU** on Linux.
The software SDL renderer remains available. Rendering uses SDL texture upload,
copy, blend, fill and readback operations with a **GLES2 feature floor**. The font
atlas is 128×176 RGBA (88 KiB); no GLES3, Vulkan, proprietary Mali/Broadcom API or
DispmanX dependency is required. Newer native desktop SDL backends may also work.

SDL's advertised `opengles2` backend is preferred; other accelerated drivers keep
SDL's order. Each failed attempt drops its renderer/window before retrying, first
with VSync on every accelerated driver, then without synchronization only if all
synchronized attempts failed. GL contexts additionally require swap interval 1. `auto` falls back to SDL software after
all hardware attempts fail. `hardware` fails instead of silently using software.
`software` bypasses accelerated selection. All four binaries accept `--renderer auto|hardware|software`; the shell passes its policy to native children.

## Diagnose the running environment

From the existing graphical desktop session, as its normal user:

```sh
vitrallis --graphics-info
vitrallis --graphics-test --renderer auto
vitrallis --graphics-test --renderer hardware
vitrallis --graphics-test --renderer software
```

For a bundle installation, use `~/.local/share/vitrallis/current/vitrallis`.
These commands create a small, temporary SDL window, use the normal renderer
selection logic and exit. They do not discover/launch apps, read app configuration,
change settings or write user data. Only `--renderer` may be combined with a
graphics command. A working display/session is required; an SSH terminal alone
does not supply one. On the tested PocketCHIP, that session uses `DISPLAY=:0` and
`XAUTHORITY=/home/chip/.Xauthority`. Do not copy those values onto other systems.

`--graphics-info` reports SDL video/backend, renderer flags, software/accelerated
flags, the SDL vsync flag, window/output/display sizes and available renderer
drivers. Linux diagnostics additionally report DRM nodes, kernel driver symlinks,
current-user access checks and loaded graphics libraries. Optional-source failures
never make ordinary startup depend on `/dev/dri`, sysfs or procfs.

For an active GL/GLES renderer, Vitrallis asks **that renderer's current context**
for GL vendor, renderer and version. It does not create a second diagnostic GL
context that could accidentally describe a different GPU. When SDL actually uses
EGL, the optional `EGL_MESA_query_driver` query reports the **EGL display driver**.
An unavailable query remains unknown. Library filenames indicate what is loaded,
not which driver performed rendering. A native Metal renderer, for example, has
no GL identity to report.

Keep these identities distinct:

| Observation | What it identifies |
| --- | --- |
| `Mali400` from GL_RENDERER | Rendering device reported by the current GL context |
| `lima` from the render node's sysfs driver | Kernel driver serving that DRM GPU device |
| Mesa version in GL_VERSION | Current GL userspace implementation/version |
| `sun4i-drm` from EGL_MESA_query_driver on this PocketCHIP | EGL display driver; **not** the GPU's Lima driver |
| `opengles2` from SDL | SDL drawing backend; **not** a GPU model |

SDL can advertise acceleration while Mesa rasterizes on the CPU. The shared
initialization path rejects recognized `llvmpipe`, `softpipe`, `swrast`, `SWR` and
`Software Rasterizer` identities. The original identity remains in the failure
report. Auto retries/falls back; Hardware fails; Software remains usable. This is
software-indicator detection, not a whitelist of approved GPU strings. Unknown
identity and SDL flags alone are not proof of physical GPU execution.

On this PocketCHIP SDL normally creates its GLES2 context through GLX; EGL
identity is therefore omitted on that path. The additional process-local
`SDL_VIDEO_X11_FORCE_EGL=1` check exercises EGL explicitly without making it a
generic renderer requirement.

The self-test uploads a 2×2 color texture, copies it, fills a rectangle, renders
`A` through the production font atlas and compares all 512 pixels of a 32×16
region to their expected RGB values. Readback happens **before** presentation,
since presentation may invalidate the backbuffer. It presents and drops all
resources normally. An SDL operation failure or color/orientation/atlas mismatch
returns a nonzero exit status. This tests API operations, not monitor electronics
or manual touch/keyboard operation.

## Linux packages and recovery

The authoritative physical target for this phase runs Debian 13 with upstream
Mesa Lima. Its installed packages were verified with `dpkg-query`:

| Component | Debian package on the tested target |
| --- | --- |
| SDL2 runtime | `libsdl2-2.0-0` |
| Mesa DRI/Gallium drivers, including Lima | `libgl1-mesa-dri`, with its `mesa-libgallium` dependency |
| EGL dispatcher and Mesa EGL implementation | `libegl1`, `libegl-mesa0` |
| GLES2 dispatcher | `libgles2` |
| DRM userspace library | `libdrm2` |

The installer does not run apt. Its PocketCHIP GPU provisioning step uses sudo
for the root-owned platform helper; the desktop remains unprivileged. It retains SDL's required
runtime check and gives nonfatal advice for missing EGL/GLES/DRM libraries or
`/dev/dri`. Package provisioning stays with the device owner/distro image. Missing
DRM nodes must not block a software-capable installation. No proprietary Mali
blob is needed. See [PocketCHIP setup](devices/pocketchip.md).

If Auto falls back, preserve the full original SDL failure and run graphics-info.
Check the existing display/session first, then distro Mesa DRI/EGL/GLES packages
and the listed DRM node access. Permission failure may require the distro's
normal seat ACL or `render`/`video` group setup; do not run the shell as root or
make DRM nodes world-writable. Missing libraries, a missing driver, and a broken
EGL/GL context can produce similar SDL errors; diagnostics do not pretend to know
which caused an initialization failure. `--renderer software` is the immediate
recovery path. Explicit Hardware failure does not make installation unusable.

For a controlled fallback check, `LIBGL_ALWAYS_SOFTWARE=1 vitrallis --renderer
hardware --graphics-test` should fail when Mesa exposes a recognized software
renderer. The equivalent Auto invocation should pass using SDL software. This
sets a process-local environment variable, not a system graphics configuration.

Lima utilization comes from the kernel's `devfreq_monitor` load field when the
GPU has an OPP/devfreq configuration. The normal PocketCHIP installer provisions
the device-validated single 297 MHz OPP and private trace access. Missing OPP data,
unavailable tracing, and an unsupported GPU are separate Debug states; 0% is a
valid sample. GPU runtime suspension can stop samples, in which case old values
expire rather than being held indefinitely. See [GPU setup and verification](devices/pocketchip/gpu-utilization.md).

## Architectural support and physical validation

| Target | GPU | Mesa driver | Physical GPU validation |
| --- | --- | --- | --- |
| PocketCHIP | Mali-400 | Lima | TESTED / PASS during this phase — see the physical record |
| Raspberry Pi 1 | VideoCore IV | vc4 | PHYSICAL GPU VALIDATION PENDING / UNTESTED |
| Raspberry Pi 4 | VideoCore VI | V3D | PHYSICAL GPU VALIDATION PENDING / UNTESTED |

Raspberry Pi 1 / vc4: physical GPU validation pending.

Raspberry Pi 4 / V3D: physical GPU validation pending.

Generic rendering has no Mali/board-name selection, Pi-generation branch,
Broadcom API, DispmanX or Raspberry Pi OS requirement. The intended Pi path is
SDL2 with the distro's Mesa vc4 or V3D stack. Distro package names and permissions
must be checked on the future target. No Raspberry Pi was contacted, emulated as
a GPU, or physically tested in this phase. No Wayland implementation is added;
selection uses the supplied SDL video subsystem and does not force X11, so this
work does not prevent a future SDL Wayland backend.

Software/dummy CI, Linux Mesa software testing, macOS accelerated checks, ARM
compilation and mocks are separate from physical GPU acceptance. None upgrades
the Raspberry Pi entries. The existing CI still runs its complete software gate
and ARM release build; it does not require a physical GPU. The optional
`cargo test --test desktop mesa_software_is_not_hardware -- --ignored` test needs
a graphical Linux Mesa session and checks software detection/fallback only.

The [PocketCHIP phase record](devices/pocketchip/graphics-phase3.md) contains exact
versions, commands, results, performance observations and limitations. Earlier
[renderer optimization measurements](rendering-performance.md) retain their
original host-only scope.

## Future physical procedure (vc4 and V3D still pending)

For each real target, record OS/kernel, GPU identity, node-to-driver mapping,
permissions, SDL/GL/EGL output and Mesa packages. Run graphics-info and graphics-test
in all three renderer modes. Corroborate renderer identity with the actual process's
DRM descriptors and available kernel telemetry. Test cold launch, home/icons/
wallpaper, fast navigation, settings, App Center, native apps, desktop shortcuts,
text-heavy screens, repeated launch/exit, fullscreen return and screenshot colors.
Observe at least 15 minutes idle plus extended navigation, CPU/RSS, resource counts,
GPU activity where measurable, thermal behavior and clean application shutdown.
Record injected input separately from manual switches/touch. Preserve failures
and unavailable measurements. Change a target's matrix status only after its
own physical tests actually complete.

## Interface references

- [SDL renderer flags](https://wiki.libsdl.org/SDL2/SDL_GetRendererInfo) describe SDL capabilities.
- [SDL current GL context](https://wiki.libsdl.org/SDL2/SDL_GL_GetCurrentContext) and [queued rendering](https://wiki.libsdl.org/SDL2/SDL_RenderFlush) explain the context/readback boundary.
- [EGL_MESA_query_driver](https://registry.khronos.org/EGL/extensions/MESA/EGL_MESA_query_driver.txt) defines the optional display-driver query.
- [Mesa Lima](https://docs.mesa3d.org/drivers/lima.html) documents upstream Mali-400 support.
- [Mesa environment variables](https://docs.mesa3d.org/envvars.html) documents the process-local software override.
