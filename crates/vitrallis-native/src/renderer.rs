//! Native presentation boundary: compose complete frames in SDL's backbuffer,
//! then present once with `VSync`.
//!
//! Shell widgets and native apps must use this
//! factory and `PresentationClock`; never draw into a visible window surface.
//! Unsynchronized presentation is a diagnosed fallback only after every
//! accelerated backend has failed to supply synchronization.
//! On X11, a successful swap interval does not prove tear-free scanout for
//! windowed surfaces. The `PocketCHIP` session supplies a Present compositor.
use sdl2::{render::Canvas, video::Window};
use sdl2::{render::RendererInfo as SdlInfo, sys::SDL_RendererFlags};
use std::fmt;
#[cfg(target_os = "linux")]
mod egl;
pub mod graphics;

/// SDL renderer policy, independent of the selected device/system backend.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    #[default]
    Auto,
    Hardware,
    Software,
}

impl RendererMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Hardware => "hardware",
            Self::Software => "software",
        }
    }
}

impl std::str::FromStr for RendererMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "hardware" => Ok(Self::Hardware),
            "software" => Ok(Self::Software),
            _ => Err(format!(
                "invalid renderer {value:?}; use auto, hardware or software"
            )),
        }
    }
}

const ACCELERATED: u32 = SDL_RendererFlags::SDL_RENDERER_ACCELERATED as u32;
const SOFTWARE: u32 = SDL_RendererFlags::SDL_RENDERER_SOFTWARE as u32;
const VSYNC: u32 = SDL_RendererFlags::SDL_RENDERER_PRESENTVSYNC as u32;

/// Startup snapshot for system/debug consumers. SDL flags describe
/// the renderer, not the physical GPU or whether Mesa uses CPU rasterization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererInfo {
    pub requested: RendererMode,
    pub actual: RendererMode,
    pub sdl: SdlInfo,
    pub video_driver: String,
    pub window_size: (u32, u32),
    pub output_size: (u32, u32),
    /// Current SDL display mode; unavailable queries remain unknown.
    pub display_size: Option<(i32, i32)>,
    pub hardware_error: Option<String>,
    pub gl: Option<graphics::GlInfo>,
}

impl RendererInfo {
    #[must_use]
    pub const fn accelerated(&self) -> bool {
        self.sdl.flags & ACCELERATED != 0
    }

    #[must_use]
    pub const fn software(&self) -> bool {
        self.sdl.flags & SOFTWARE != 0
    }

    #[must_use]
    pub const fn vsync(&self) -> bool {
        self.sdl.flags & VSYNC != 0
    }
}

impl fmt::Display for RendererInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "level=info event=renderer_initialized requested={} mode={} driver={:?} accelerated={} software={} vsync={} max_texture_width={} max_texture_height={} video_driver={:?} window_width={} window_height={} output_width={} output_height={}",
            self.requested.as_str(),
            self.actual.as_str(),
            self.sdl.name,
            self.accelerated(),
            self.software(),
            self.vsync(),
            self.sdl.max_texture_width,
            self.sdl.max_texture_height,
            self.video_driver,
            self.window_size.0,
            self.window_size.1,
            self.output_size.0,
            self.output_size.1,
        )?;
        if let Some((width, height)) = self.display_size {
            write!(f, " display_width={width} display_height={height}")?;
        } else {
            write!(f, " display_width=unknown display_height=unknown")?;
        }
        if let Some(gl) = &self.gl {
            write!(
                f,
                " gl_vendor={:?} gl_renderer={:?} gl_version={:?} mesa_software={}",
                gl.vendor,
                gl.renderer,
                gl.version,
                gl.software()
            )?;
        }
        write!(
            f,
            " buffering=sdl-backbuffer synchronization={}",
            if self.vsync() && !self.software() {
                "sdl-vsync-requested"
            } else {
                "unverified-fallback"
            }
        )?;
        if let Some(error) = &self.hardware_error {
            write!(f, " fallback=true hardware_error={error:?}")
        } else {
            write!(f, " fallback=false")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Attempt {
    index: u32,
    mode: RendererMode,
    vsync: bool,
}

fn verify(mode: RendererMode, info: &SdlInfo) -> Result<(), String> {
    let valid = match mode {
        RendererMode::Hardware => info.flags & ACCELERATED != 0 && info.flags & SOFTWARE == 0,
        RendererMode::Software => info.flags & SOFTWARE != 0 && info.flags & ACCELERATED == 0,
        RendererMode::Auto => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "SDL renderer {:?} flags={:#x} do not satisfy {} mode",
            info.name,
            info.flags,
            mode.as_str()
        ))
    }
}

/// Keep the retry policy display-free; the factory owns and drops each rejected
/// canvas before another attempt. Driver indices always come from SDL discovery.
fn select<T>(
    requested: RendererMode,
    drivers: &[(u32, SdlInfo)],
    mut create: impl FnMut(Attempt) -> Result<(T, SdlInfo), String>,
) -> Result<(T, SdlInfo, Option<String>), String> {
    let mut failures = Vec::new();
    if requested != RendererMode::Software {
        let mut hardware: Vec<_> = drivers
            .iter()
            .filter(|(_, info)| verify(RendererMode::Hardware, info).is_ok())
            .collect();
        // Prefer the GLES2 backend when SDL actually provides it. All remaining
        // accelerated drivers retain SDL's order (e.g. Metal on a macOS host).
        hardware.sort_by_key(|(_, info)| info.name != "opengles2");
        for vsync in [true, false] {
            for (index, driver) in &hardware {
                let attempt = Attempt {
                    index: *index,
                    mode: RendererMode::Hardware,
                    vsync,
                };
                let result = create(attempt).and_then(|(canvas, info)| {
                    verify(attempt.mode, &info)?;
                    if attempt.vsync && info.flags & VSYNC == 0 {
                        return Err("renderer did not enable requested VSync".into());
                    }
                    Ok((canvas, info))
                });
                match result {
                    Ok((canvas, info)) => return Ok((canvas, info, None)),
                    Err(error) => {
                        failures.push(format!("driver={:?} vsync={vsync}: {error}", driver.name));
                    }
                }
            }
        }
        if failures.is_empty() {
            failures.push("SDL advertises no accelerated render drivers".into());
        }
    }
    let hardware_error = (!failures.is_empty()).then(|| failures.join("; "));
    if requested == RendererMode::Hardware {
        return Err(format!(
            "hardware renderer unavailable: {}; check Mesa DRI/EGL/GLES libraries, DRM node permissions and display access; run --graphics-info, or use --renderer auto or --renderer software",
            hardware_error
                .as_deref()
                .unwrap_or("no accelerated renderer")
        ));
    }
    let result = drivers
        .iter()
        .find(|(_, info)| verify(RendererMode::Software, info).is_ok())
        .ok_or_else(|| "SDL advertises no software render driver".to_owned())
        .and_then(|(index, _)| {
            let mut last_error = String::new();
            for vsync in [true, false] {
                let result = create(Attempt {
                    index: *index,
                    mode: RendererMode::Software,
                    vsync,
                })
                .and_then(|(canvas, info)| {
                    verify(RendererMode::Software, &info)?;
                    Ok((canvas, info))
                });
                match result {
                    Ok(value) => return Ok(value),
                    Err(error) => last_error = error,
                }
            }
            Err(last_error)
        });
    match result {
        Ok((canvas, info)) => Ok((canvas, info, hardware_error)),
        Err(error) => Err(format!(
            "software renderer unavailable: {error}; hardware attempt: {}",
            hardware_error.as_deref().unwrap_or("not requested")
        )),
    }
}

/// Create and verify a renderer, retrying with a fresh application window.
/// The factory must return a hidden window with the application's window policy.
/// # Errors
/// Reports all failed hardware attempts and software failure, or required hardware failure.
pub fn initialize(
    video: &sdl2::VideoSubsystem,
    requested: RendererMode,
    mut window: impl FnMut() -> Result<Window, String>,
) -> Result<(Canvas<Window>, RendererInfo), String> {
    let drivers: Vec<_> = (0_u32..).zip(sdl2::render::drivers()).collect();
    // Failed attempts may destroy the last window. They must not queue a quit
    // event that closes the eventual successful renderer (notably on macOS).
    sdl2::hint::set("SDL_QUIT_ON_LAST_WINDOW_CLOSE", "0");
    // SDL/environment hints must not silently override the native UI contract.
    sdl2::hint::set_with_priority("SDL_RENDER_VSYNC", "1", &sdl2::hint::Hint::Override);
    video.gl_attr().set_double_buffer(true);
    let ((canvas, output_size, gl), sdl, hardware_error) =
        select(requested, &drivers, |attempt| {
            // A fresh window discards any GL/Metal state from a failed backend.
            // Keep unsuccessful attempts hidden and preserve the same window policy.
            sdl2::hint::set_with_priority(
                "SDL_RENDER_VSYNC",
                if attempt.vsync { "1" } else { "0" },
                &sdl2::hint::Hint::Override,
            );
            let window = window()?;
            let builder = window.into_canvas().index(attempt.index);
            let mut builder = if attempt.mode == RendererMode::Software {
                builder.software()
            } else {
                builder.accelerated()
            };
            if attempt.vsync {
                builder = builder.present_vsync();
            }
            let mut canvas = builder.build().map_err(|error| error.to_string())?;
            let info = canvas.info();
            verify(attempt.mode, &info)?;
            let gl = graphics::current_gl(&canvas);
            if attempt.mode == RendererMode::Hardware {
                reject_software_gl(gl.as_ref())?;
                if attempt.vsync && gl.is_some() {
                    // SDL can simulate VSync using a timer even on GL. Require
                    // a real swap interval before accepting this attempt.
                    video.gl_set_swap_interval(sdl2::video::SwapInterval::VSync)?;
                    if video.gl_get_swap_interval() != sdl2::video::SwapInterval::VSync {
                        return Err("GL did not enable swap interval 1".into());
                    }
                }
            }
            canvas.window_mut().show();
            // Showing a fullscreen-desktop window can change its drawable size.
            // Query after that transition, within the fallible retry boundary.
            let output_size = canvas.output_size()?;
            Ok(((canvas, output_size, gl), info))
        })?;
    let display_size = canvas.window().display_index().ok().and_then(|index| {
        video
            .current_display_mode(index)
            .ok()
            .map(|mode| (mode.w, mode.h))
    });
    let info = RendererInfo {
        requested,
        actual: if sdl.flags & ACCELERATED != 0 {
            RendererMode::Hardware
        } else {
            RendererMode::Software
        },
        sdl,
        video_driver: video.current_video_driver().into(),
        window_size: canvas.window().size(),
        output_size,
        display_size,
        hardware_error,
        gl,
    };
    if !info.vsync() || info.software() {
        eprintln!(
            "level=warn event=presentation_fallback driver={:?} synchronization=unverified pacing=bounded tearing_possible=true",
            info.sdl.name
        );
    }
    if let Some(error) = &info.hardware_error {
        eprintln!(
            "Hardware renderer initialization failed: {error:?}. Vitrallis is continuing with software rendering. Check Mesa DRI/EGL/GLES packages, DRM permissions and display access. Run vitrallis --graphics-info for diagnostics."
        );
    }
    Ok((canvas, info))
}

fn reject_software_gl(gl: Option<&graphics::GlInfo>) -> Result<(), String> {
    gl.filter(|gl| gl.software()).map_or(Ok(()), |gl| Err(format!(
        "SDL accelerated backend uses software Mesa renderer {:?}; genuine GPU acceleration is unavailable",
        gl.renderer
    )))
}

/// Bounds production frame submission even during sustained input or PTY output.
///
/// Hardware `VSync` is the primary synchronizer. The deadline only prevents an
/// unavailable/nonblocking backend from spinning; it is not a vblank substitute.
/// Idle callers continue to block in their event queue and do not repaint.
pub struct PresentationClock {
    next_frame: std::time::Instant,
    interval: std::time::Duration,
}

impl PresentationClock {
    #[must_use]
    pub fn new(canvas: &Canvas<Window>) -> Self {
        let window = canvas.window();
        let refresh = window
            .display_index()
            .ok()
            .and_then(|index| window.subsystem().current_display_mode(index).ok())
            .map_or(60, |mode| mode.refresh_rate);
        Self {
            next_frame: std::time::Instant::now(),
            interval: std::time::Duration::from_secs_f64(1.0 / f64::from(refresh.clamp(30, 240))),
        }
    }

    /// Present one complete backbuffer on the SDL/main thread. Time spent waiting
    /// for the backend's swap consumes the deadline; no extra post-swap sleep.
    pub fn present(&mut self, canvas: &mut Canvas<Window>) {
        let wait = self
            .next_frame
            .saturating_duration_since(std::time::Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        self.next_frame = std::time::Instant::now() + self.interval;
        canvas.present();
    }
}

#[cfg(test)]
mod tests;
