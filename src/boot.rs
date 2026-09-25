//! Bounded, 12 FPS startup presentation through the normal synchronized canvas.
//! Five pre-rendered keyframes, at most two copies per frame, no effects shaders.
use crate::{
    layout::{Layout, Rect},
    renderer::{self, Screen},
};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    render::{BlendMode, ScaleMode, Texture, TextureCreator},
    video::WindowContext,
};
use std::{
    sync::mpsc::{self, TryRecvError},
    time::{Duration, Instant},
};
use vitrallis_native::theme;

const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 12);
const DURATION: Duration = Duration::from_secs(3);
const ASSETS: [&[u8]; 5] = [
    include_bytes!("../assets/boot/clean.png"),
    include_bytes!("../assets/boot/subtle.png"),
    include_bytes!("../assets/boot/cyan.png"),
    include_bytes!("../assets/boot/full.png"),
    include_bytes!("../assets/boot/scene.png"),
];

struct Artwork<'a> {
    textures: Vec<Texture<'a>>,
}
impl<'a> Artwork<'a> {
    fn load(creator: &'a TextureCreator<WindowContext>, assets: &[&[u8]]) -> Result<Self, String> {
        let mut textures = Vec::with_capacity(assets.len());
        for bytes in assets {
            let surface = renderer::decode_icon(bytes)?;
            if (surface.width(), surface.height()) != (480, 272) {
                return Err("boot keyframe must be 480x272".into());
            }
            let mut texture = creator
                .create_texture_from_surface(surface)
                .map_err(|e| e.to_string())?;
            texture.set_scale_mode(ScaleMode::Nearest);
            texture.set_blend_mode(BlendMode::Blend);
            textures.push(texture);
        }
        Ok(Self { textures })
    }

    fn draw(&mut self, canvas: &mut Screen, size: (u32, u32), frame: u32) -> Result<(), String> {
        canvas.set_draw_color(theme::BACKGROUND);
        canvas.clear();
        let destination = destination(size)?;
        let (base, next, alpha, light) = sample(frame);
        // Keep the identical crystal texture fixed while the cave appears behind
        // it. Cross-fading complete pictures would morph or move the logo.
        let layers = if next == Some(4) {
            [(next, alpha), (base, 255)]
        } else {
            [(base, 255), (next, alpha)]
        };
        for (index, opacity) in layers {
            if let Some(texture) = index.and_then(|i| self.textures.get_mut(i)) {
                texture.set_alpha_mod(opacity);
                texture.set_color_mod(light, light, light);
                canvas.copy(texture, None, destination)?;
            }
        }
        Ok(())
    }
}

/// Integer enlargement; smaller displays crop the centered native scene instead
/// of fractionally scaling pixel art. The logo/name stay inside the 320x200 safe area.
fn destination((width, height): (u32, u32)) -> Result<sdl2::rect::Rect, String> {
    let scale = (width / 480).min(height / 272).max(1);
    let w = i32::try_from(480 * scale).map_err(|_| "boot width")?;
    let h = i32::try_from(272 * scale).map_err(|_| "boot height")?;
    Ok(sdl2::rect::Rect::new(
        (i32::try_from(width).map_err(|_| "display width")? - w) / 2,
        (i32::try_from(height).map_err(|_| "display height")? - h) / 2,
        w.unsigned_abs(),
        h.unsigned_abs(),
    ))
}

// Fixed frame indices make fades deliberately stepped and deterministic.
fn sample(frame: u32) -> (Option<usize>, Option<usize>, u8, u8) {
    let blend = |start| u8::try_from((frame - start) * 255 / 4).unwrap_or(255);
    match frame {
        0..=2 => (None, None, 0, 255),
        3..=6 => (Some(0), None, 0, 20 + blend(3) / 4),
        7..=10 => (Some(0), None, 0, 83 + (blend(7) / 3) * 2),
        11..=14 => (Some(0), Some(1), blend(11), 255),
        15..=18 => (Some(1), Some(2), blend(15), 255),
        19..=22 => (Some(2), Some(3), blend(19), 255),
        23..=26 => (Some(3), None, 0, 255),
        27..=30 => (Some(3), Some(4), blend(27), 255),
        _ => (Some(3), Some(4), 255, 255),
    }
}

/// Discovery runs concurrently; keep the final state until it finishes. No
/// decoration failure may prevent the launcher from starting.
pub fn load<T: Send>(
    sdl: &sdl2::Sdl,
    canvas: &mut Screen,
    layout: &Layout,
    discover: impl FnOnce() -> Result<T, String> + Send,
) -> Result<Option<T>, String> {
    std::thread::scope(|scope| {
        let (send, receive) = mpsc::sync_channel(1);
        let worker = scope.spawn(move || {
            let _ = send.send(discover());
        });
        let result = play(sdl, canvas, layout, &receive);
        worker
            .join()
            .map_err(|_| "startup discovery worker failed")?;
        result
    })
}

fn play<T>(
    sdl: &sdl2::Sdl,
    canvas: &mut Screen,
    layout: &Layout,
    receive: &mpsc::Receiver<Result<T, String>>,
) -> Result<Option<T>, String> {
    canvas.set_draw_color(theme::BACKGROUND);
    canvas.clear();
    canvas.present();
    let creator = canvas.texture_creator();
    let mut art = load_optional(&creator);
    let mut events = sdl.event_pump()?;
    let start = Instant::now();
    let mut result = None;
    let mut drawn = None;
    let mut skip = false;
    loop {
        if result.is_none() {
            match receive.try_recv() {
                Ok(value) => result = Some(value?),
                Err(TryRecvError::Empty) => (),
                Err(TryRecvError::Disconnected) => return Err("startup discovery stopped".into()),
            }
        }
        let elapsed = start.elapsed();
        if result.is_some() && (skip || elapsed >= DURATION) {
            eprintln!(
                "level=info event=boot_complete elapsed_ms={}",
                elapsed.as_millis()
            );
            draw(canvas, layout, art.as_mut(), 35)?;
            return Ok(result);
        }
        let frame = u32::try_from(elapsed.as_nanos() / FRAME.as_nanos())
            .unwrap_or(36)
            .min(36);
        if drawn != Some(frame) {
            draw(canvas, layout, art.as_mut(), frame)?;
            canvas.present();
            drawn = Some(frame);
        }
        // Once holding the last frame, do not redraw or animate. A short wait
        // still allows close, reset and discovery completion without spinning.
        let wait = if frame < 36 {
            u32::try_from(
                (FRAME * (frame + 1))
                    .saturating_sub(start.elapsed())
                    .as_millis(),
            )
            .unwrap_or(83)
            .clamp(1, 83)
        } else {
            100
        };
        if let Some(event) = events.wait_event_timeout(wait) {
            match event {
                Event::Quit { .. }
                | Event::Window {
                    win_event: WindowEvent::Close,
                    ..
                } => return Ok(None),
                Event::KeyDown {
                    keycode: Some(Keycode::Escape | Keycode::Home),
                    repeat: false,
                    ..
                } => skip = true,
                Event::RenderDeviceReset { .. } => {
                    reset_artwork(canvas, &creator, &mut art)?;
                    drawn = None;
                }
                Event::Window {
                    win_event:
                        WindowEvent::Exposed | WindowEvent::Shown | WindowEvent::SizeChanged(..),
                    ..
                } => drawn = None,
                _ => (),
            }
        }
    }
}

fn reset_artwork<'a>(
    canvas: &mut Screen,
    creator: &'a TextureCreator<WindowContext>,
    art: &mut Option<Artwork<'a>>,
) -> Result<(), String> {
    canvas.reset()?;
    drop(art.take());
    *art = load_optional(creator);
    Ok(())
}

fn load_optional(creator: &TextureCreator<WindowContext>) -> Option<Artwork<'_>> {
    match Artwork::load(creator, &ASSETS) {
        Ok(art) => Some(art),
        Err(error) => {
            eprintln!("level=warn event=boot_artwork_fallback message={error:?}");
            None
        }
    }
}

fn draw(
    canvas: &mut Screen,
    layout: &Layout,
    art: Option<&mut Artwork<'_>>,
    frame: u32,
) -> Result<(), String> {
    let (width, height) = canvas.output_size()?;
    let width = i32::try_from(width).map_err(|_| "boot display width")?;
    let height = i32::try_from(height).map_err(|_| "boot display height")?;
    if let Some(art) = art {
        art.draw(canvas, canvas.output_size()?, frame)?;
    } else {
        canvas.set_draw_color(theme::BACKGROUND);
        canvas.clear();
        let scale = layout.text_scale;
        renderer::text(
            canvas,
            "VITRALLIS",
            Rect {
                x: 0,
                y: height / 2 - 16 * scale,
                w: width,
                h: 32 * scale,
            },
            scale,
            theme::TEXT,
        )?;
    }
    if frame == 36 {
        let scale = layout.text_scale;
        renderer::text(
            canvas,
            "Starting...",
            Rect {
                x: 0,
                y: height - 24 * scale,
                w: width,
                h: 16 * scale,
            },
            scale,
            theme::MUTED,
        )?;
    }
    Ok(())
}

#[cfg(test)]
pub use tests::{lifecycle, qa};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an accelerated backend and a graphical session"]
    fn hardware_boot_matches_software() -> Result<(), String> {
        use renderer::backend::{self, RendererMode};
        let sdl = sdl2::init()?;
        let video = sdl.video()?;
        let mut reference = Vec::new();
        let mut maximum_difference = 0;
        let qa = std::env::var_os("VITRALLIS_QA_DIR").map(std::path::PathBuf::from);
        for mode in [RendererMode::Software, RendererMode::Hardware] {
            let (canvas, info) = backend::initialize(&video, mode, || {
                video
                    .window("Boot pixel validation", 480, 272)
                    .hidden()
                    .build()
                    .map_err(|e| e.to_string())
            })?;
            eprintln!("{info}");
            let creator = canvas.texture_creator();
            let mut canvas = Screen::new(canvas, &creator)?;
            let mut art = Artwork::load(&creator, &ASSETS)?;
            for frame in 0..=36 {
                art.draw(&mut canvas, (480, 272), frame)?;
                if let Some(directory) = &qa {
                    renderer::screenshot(
                        &canvas,
                        &directory.join(format!("boot-{mode:?}-{frame:02}.bmp")),
                    )?;
                }
                let pixels = canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGB24)?;
                if mode == RendererMode::Software {
                    reference.push(pixels);
                } else {
                    let expected = &reference[usize::try_from(frame).map_err(|_| "boot frame")?];
                    assert_eq!(pixels.len(), expected.len());
                    let difference = pixels
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap_or(0);
                    eprintln!("boot frame {frame}: maximum channel difference {difference}");
                    maximum_difference = maximum_difference.max(difference);
                    let total_error: usize = pixels
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| usize::from(a.abs_diff(*b)))
                        .sum();
                    let (_, next, alpha, _) = sample(frame);
                    let error_budget = if next == Some(4) && alpha > 0 && alpha < 255 {
                        pixels.len() * 5 / 4
                    } else {
                        pixels.len() / 5
                    };
                    assert!(
                        total_error <= error_budget,
                        "boot frame {frame}: total error {total_error}"
                    );
                }
                canvas.present();
            }
        }
        // Software integer color/alpha modulation truncates intermediate values;
        // GLES normalizes and rounds them, including the sprite's own alpha.
        // Mali-400 measurements bound the two-layer difference to 5/255 per
        // channel. Sparse sprite blends average below 1/5 level; blending the
        // cave across the entire screen averages below 5/4 levels.
        assert!(
            maximum_difference <= 5,
            "boot channel difference {maximum_difference}"
        );
        Ok(())
    }

    #[test]
    fn timeline_reaches_the_storyboard_and_holds_without_wrapping() -> Result<(), String> {
        assert_eq!(sample(0).0, None);
        assert_eq!(sample(3).0, Some(0));
        for (frame, stage) in [(11, 1), (15, 2), (19, 3), (27, 4)] {
            assert_eq!(sample(frame).1, Some(stage));
        }
        assert_eq!(sample(36), sample(u32::MAX));
        for frame in 0..=36 {
            let (base, next, _, _) = sample(frame);
            assert!(
                base.into_iter()
                    .chain(next)
                    .all(|index| index < ASSETS.len())
            );
        }
        for (size, scale) in [
            ((320, 200), 1),
            ((480, 272), 1),
            ((800, 480), 1),
            ((960, 544), 2),
            ((1280, 720), 2),
        ] {
            let rect = destination(size)?;
            assert_eq!((rect.width(), rect.height()), (480 * scale, 272 * scale));
        }
        Ok(())
    }

    // Run inside the existing SDL fixture, never a second simultaneous SDL thread.
    pub fn lifecycle(sdl: &sdl2::Sdl, canvas: &mut Screen, layout: &Layout) -> Result<(), String> {
        let events = sdl.event()?;
        let creator = canvas.texture_creator();
        let mut art = load_optional(&creator);
        draw(canvas, layout, art.as_mut(), 35)?;
        let before = canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)?;
        reset_artwork(canvas, &creator, &mut art)?;
        draw(canvas, layout, art.as_mut(), 35)?;
        assert_eq!(
            canvas.read_pixels(None, sdl2::pixels::PixelFormatEnum::RGBA32)?,
            before
        );
        drop(art);
        events.push_event(Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Escape),
            scancode: None,
            keymod: sdl2::keyboard::Mod::NOMOD,
            repeat: false,
        })?;
        assert_eq!(load(sdl, canvas, layout, || Ok(42))?, Some(42));
        assert_eq!(
            load::<()>(sdl, canvas, layout, || Err("discovery failure".into())),
            Err("discovery failure".into())
        );
        events.push_event(Event::Quit { timestamp: 0 })?;
        assert_eq!(load(sdl, canvas, layout, || Ok(42))?, None);
        Ok(())
    }

    pub fn qa(
        canvas: &mut Screen,
        layout: &Layout,
        output: &std::path::Path,
    ) -> Result<(), String> {
        let creator = canvas.texture_creator();
        let mut art = Artwork::load(&creator, &ASSETS)?;
        assert_eq!(art.textures.len(), 5);
        assert!(Artwork::load(&creator, &[b"invalid"]).is_err());
        assert!(
            Artwork::load(
                &creator,
                &[include_bytes!("../assets/branding/crystal-8.png")]
            )
            .is_err()
        );
        for frame in [0, 3, 10, 14, 18, 22, 26, 30, 35, 36] {
            draw(canvas, layout, Some(&mut art), frame)?;
            renderer::screenshot(
                canvas,
                &output.join(format!(
                    "boot-{frame:02}-{}x{}.bmp",
                    layout.width, layout.height
                )),
            )?;
        }
        draw(canvas, layout, None, 36)?;
        renderer::screenshot(
            canvas,
            &output.join(format!(
                "boot-fallback-{}x{}.bmp",
                layout.width, layout.height
            )),
        )
    }
}
