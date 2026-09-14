//! Optional observations of the current SDL renderer; never creates a second GL context.
use super::RendererInfo;
use sdl2::{pixels::PixelFormatEnum, rect::Rect, render::Canvas, video::Window};
use std::fmt::Write;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Strings returned by the active GL context. A backend name is not a GPU model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlInfo {
    pub vendor: String,
    pub renderer: String,
    pub version: String,
    pub egl_version: Option<String>,
    pub egl_display_driver: Option<String>,
}

impl GlInfo {
    #[must_use]
    pub fn software(&self) -> bool {
        software_renderer(&self.renderer)
    }
}

fn software_renderer(renderer: &str) -> bool {
    let renderer = renderer.to_ascii_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "swrast",
        "software rasterizer",
        "swr",
    ]
    .iter()
    .any(|name| {
        renderer
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == *name)
            || (*name == "software rasterizer" && renderer.contains(name))
    })
}

/// Read only the GL context used by this canvas. Readback activates SDL's backend
/// and drains its queue before the read-only GL query. No GL state is changed.
pub(super) fn current_gl(canvas: &mut Canvas<Window>) -> Option<GlInfo> {
    if !matches!(canvas.info().name, "opengl" | "opengles" | "opengles2") {
        return None;
    }
    canvas
        .read_pixels(Rect::new(0, 0, 1, 1), PixelFormatEnum::RGBA32)
        .ok()?;
    // SAFETY: SDL video and this canvas live on the calling thread. These borrowed
    // handles are only compared, never adopted or freed. A foreign context is rejected.
    unsafe {
        if sdl2::sys::SDL_GL_GetCurrentContext().is_null()
            || sdl2::sys::SDL_GL_GetCurrentWindow() != canvas.window().raw()
        {
            return None;
        }
    }
    let address = canvas
        .window()
        .subsystem()
        .gl_get_proc_address("glGetString");
    if address.is_null() {
        return None;
    }
    // SAFETY: SDL resolved the standard GL/GLES glGetString entry point for its
    // current context. The system ABI matches APIENTRY; only core string enums
    // are queried. GL owns valid NUL-terminated strings until context destruction;
    // copy them now and never retain pointers. Null results remain unknown.
    let query = unsafe {
        std::mem::transmute::<*const (), unsafe extern "system" fn(u32) -> *const std::ffi::c_char>(
            address,
        )
    };
    let string = |name| {
        // SAFETY: the current context and entry point were checked above.
        let pointer = unsafe { query(name) };
        if pointer.is_null() {
            None
        } else {
            // SAFETY: glGetString's non-null result is a GL-owned C string.
            Some(
                unsafe { std::ffi::CStr::from_ptr(pointer) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    };
    #[cfg(target_os = "linux")]
    let (egl_version, egl_display_driver) = super::egl::identity();
    #[cfg(not(target_os = "linux"))]
    let (egl_version, egl_display_driver) = (None, None);
    Some(GlInfo {
        vendor: string(0x1f00)?,
        renderer: string(0x1f01)?,
        version: string(0x1f02)?,
        egl_version,
        egl_display_driver,
    })
}

/// Copy/paste support report. Missing Linux sources are observations, not errors.
#[must_use]
pub fn report(info: &RendererInfo) -> String {
    let mut out = format!(
        "Requested mode: {}\nSDL renderer mode: {}\nSDL video: {}\nSDL renderer: {}\nSDL flags: {:#x}\nSDL accelerated: {}\nSDL software: {}\nVSync (SDL flag): {}\nWindow: {}x{}\nOutput: {}x{}\n",
        info.requested.as_str(),
        info.actual.as_str(),
        info.video_driver,
        info.sdl.name,
        info.sdl.flags,
        info.accelerated(),
        info.software(),
        info.vsync(),
        info.window_size.0,
        info.window_size.1,
        info.output_size.0,
        info.output_size.1,
    );
    if let Some((w, h)) = info.display_size {
        let _ = writeln!(out, "Display: {w}x{h}");
    }
    if let Some(gl) = &info.gl {
        let _ = writeln!(
            out,
            "GL vendor: {:?}\nMesa/OpenGL renderer: {:?}\nGL version: {:?}\nKnown software rasterizer: {}",
            gl.vendor,
            gl.renderer,
            gl.version,
            gl.software()
        );
        if let Some(version) = &gl.egl_version {
            let _ = writeln!(out, "EGL version (active display): {version:?}");
        }
        if let Some(driver) = &gl.egl_display_driver {
            let _ = writeln!(
                out,
                "EGL display driver (EGL_MESA_query_driver; not necessarily GPU driver): {driver:?}"
            );
        }
        out.push_str("GPU execution: requires corroborating device/driver evidence; GL identity alone is not proof\n");
    } else {
        out.push_str("GL identity: unavailable for this renderer\n");
    }
    if let Some(error) = &info.hardware_error {
        let _ = writeln!(
            out,
            "Hardware failure: {error:?}\nContinuing with SDL software rendering."
        );
    }
    out
}

/// Enumerate available SDL drivers and optional Linux sources even if renderer
/// creation fails. Call after SDL video initialization.
#[must_use]
pub fn capabilities() -> String {
    let mut out = String::from("Available SDL renderers:\n");
    for driver in sdl2::render::drivers() {
        let _ = writeln!(out, "  {} flags={:#x}", driver.name, driver.flags);
    }
    #[cfg(target_os = "linux")]
    linux_report(&mut out);
    out
}

#[cfg(target_os = "linux")]
fn linux_report(out: &mut String) {
    match std::fs::read_dir("/dev/dri") {
        Ok(entries) => {
            out.push_str("DRM devices (/dev/dri): present (not proof of acceleration)\n");
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !drm_node(&name) {
                    continue;
                }
                let driver = std::fs::read_link(
                    Path::new("/sys/class/drm")
                        .join(name.as_ref())
                        .join("device/driver"),
                )
                .ok()
                .and_then(|path| path.file_name().map(|s| s.to_string_lossy().into_owned()));
                // access(2) uses this process's IDs, not mode-bit guesses. Merely
                // checks permissions; does not open a device or acquire DRM master.
                let access = std::ffi::CString::new(entry.path().as_os_str().as_encoded_bytes())
                    .ok()
                    .map(|path| {
                        // SAFETY: path is a live NUL-terminated CString; access retains no pointer.
                        unsafe { libc::access(path.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
                    });
                let _ = writeln!(
                    out,
                    "  {} kernel DRM driver={} read/write access={}",
                    entry.path().display(),
                    driver.as_deref().unwrap_or("unknown"),
                    access.map_or("unknown", |allowed| if allowed { "yes" } else { "no" }),
                );
            }
        }
        Err(error) => {
            let _ = writeln!(
                out,
                "DRM devices: {error} (software rendering remains available)"
            );
        }
    }
    // Report loaded libraries as evidence only, never equate a loaded DRI module
    // with the current Mesa driver. Modern Mesa may use a shared gallium library.
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        let paths: std::collections::BTreeSet<_> = maps
            .lines()
            .filter_map(|line| line.split_whitespace().nth(5))
            .filter(|path| {
                ["libEGL", "libGLES", "_dri.so", "libgallium"]
                    .iter()
                    .any(|name| path.contains(name))
            })
            .collect();
        out.push_str("Loaded graphics libraries (not active-driver identification):\n");
        for path in paths {
            let _ = writeln!(out, "  {path}");
        }
    }
    out.push_str("If hardware fails: check Mesa DRI/EGL/GLES packages, display access and DRM node permissions. Do not run as root. Preserve the SDL error above.\n");
}

#[cfg(any(target_os = "linux", test))]
fn drm_node(name: &str) -> bool {
    name.strip_prefix("card")
        .or_else(|| name.strip_prefix("renderD"))
        .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
}

/// Exercise the same font atlas and GLES2-level SDL operations used by the apps.
/// Readback precedes present because presentation may invalidate the backbuffer.
/// # Errors
/// Returns the original SDL error or a pixel mismatch. Never writes user data.
pub fn self_test(canvas: &mut Canvas<Window>) -> Result<(), String> {
    use sdl2::{pixels::Color, render::BlendMode};
    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::RGBA32, 2, 2)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(BlendMode::None);
    texture
        .update(
            None,
            &[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
            8,
        )
        .map_err(|e| e.to_string())?;
    let mut atlas = crate::font::Atlas::new(&creator)?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.set_draw_color(Color::RGB(17, 29, 40));
    canvas.fill_rect(Rect::new(0, 0, 32, 16))?;
    canvas.copy(&texture, None, Rect::new(0, 0, 2, 2))?;
    atlas.draw(
        canvas,
        'A',
        Rect::new(8, 0, 8, 8),
        Color::RGB(255, 255, 255),
    )?;
    let pixels = canvas.read_pixels(Rect::new(0, 0, 32, 16), PixelFormatEnum::RGB24)?;
    let expected_glyph =
        font8x8::UnicodeFonts::get(&font8x8::BASIC_FONTS, 'A').ok_or("missing test glyph")?;
    for y in 0..16 {
        for x in 0..32 {
            let expected = if x < 2 && y < 2 {
                [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]][y * 2 + x]
            } else if (8..16).contains(&x) && y < 8 && expected_glyph[y] & (1 << (x - 8)) != 0 {
                [255, 255, 255]
            } else {
                [17, 29, 40]
            };
            let offset = (y * 32 + x) * 3;
            if pixels.get(offset..offset + 3) != Some(expected.as_slice()) {
                return Err(format!(
                    "graphics readback mismatch at ({x}, {y}); texture/fill/atlas colors or orientation failed"
                ));
            }
        }
    }
    canvas.present();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn software_indicators_do_not_whitelist_hardware_models() {
        for name in [
            "llvmpipe (LLVM 19)",
            "SOFTPIPE",
            "Mesa swrast",
            "Software Rasterizer",
            "SWR (LLVM)",
        ] {
            assert!(software_renderer(name), "{name}");
        }
        for name in [
            "Mali400",
            "VC4 V3D 2.1",
            "V3D 4.2",
            "Apple M1",
            "Unknown GPU",
            "",
        ] {
            assert!(!software_renderer(name), "{name}");
        }
    }
    #[test]
    fn drm_device_names_are_bounded_to_nodes() {
        for name in ["card0", "card12", "renderD128"] {
            assert!(drm_node(name));
        }
        for name in [
            "card",
            "card0-HDMI-A-1",
            "by-path",
            "renderD",
            "../../card0",
        ] {
            assert!(!drm_node(name));
        }
    }
}
