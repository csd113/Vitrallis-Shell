use super::*;

fn png(path: &std::path::Path, size: u32, color: [u8; 4]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&color.repeat(usize::try_from(size * size).map_err(|_| "size")?))
        .map_err(|e| e.to_string())
}

pub(super) fn lifecycle(
    creator: &TextureCreator<WindowContext>,
    canvas: &mut Screen<'_>,
) -> Result<(), String> {
    let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
    let path = scratch.0.join("icon.png");
    png(&path, 32, [1, 2, 3, 255])?;
    let mut apps = crate::platform::generic::demo_apps(std::path::Path::new("/fixture"));
    apps.truncate(2);
    apps[0].icon = Some(path.clone());
    let mut state = Launcher::new(apps, 3, 6)?;
    let mut textures = artwork(creator, &state);
    performance::reset();
    state.selected = 1;
    textures.refresh(creator, &state);
    state.apps.reverse();
    textures.refresh(creator, &state);
    assert_eq!(performance::snapshot().uploads, 0);
    assert_eq!(performance::snapshot().decodes, 0);
    textures.reset(creator, &state);
    assert_eq!(
        performance::snapshot().uploads,
        6,
        "device reset rebuilds icon and five system textures"
    );
    performance::reset();
    png(&path, 32, [7, 8, 9, 255])?;
    textures.refresh(creator, &state);
    assert_eq!(performance::snapshot().uploads, 1);
    assert_eq!(performance::snapshot().decodes, 1);
    state.preferences.wallpaper = Some(path.clone());
    performance::reset();
    textures.refresh(creator, &state);
    assert_eq!(performance::snapshot().uploads, 1);
    state.apps.clear();
    performance::reset();
    textures.refresh(creator, &state);
    assert_eq!(performance::snapshot().uploads, 0);
    assert_eq!(performance::snapshot().decodes, 0);
    std::fs::write(&path, b"invalid image").map_err(|e| e.to_string())?;
    textures.refresh(creator, &state);
    performance::reset();
    textures.refresh(creator, &state);
    assert_eq!(
        performance::snapshot().decodes,
        0,
        "failed source is cached"
    );

    // More artwork than the 512 MiB-class device budget permits.
    state.preferences.wallpaper = None;
    for index in 0..17_u8 {
        let path = scratch.0.join(format!("{index}.png"));
        png(&path, 512, [index, 0, 0, 255])?;
        let mut app =
            crate::platform::generic::demo_apps(std::path::Path::new("/fixture")).remove(0);
        app.id = format!("budget-{index}");
        app.icon = Some(path);
        state.apps.push(app);
    }
    textures.refresh(creator, &state);
    let bytes: u32 = textures
        .iter()
        .take(17)
        .flatten()
        .map(|t| {
            let size = t.query();
            size.width * size.height * 4
        })
        .sum();
    assert_eq!(bytes, 16 * 1024 * 1024);
    assert!(textures[16].is_none());
    state.apps.remove(0);
    textures.refresh(creator, &state);
    assert!(
        textures.iter().take(16).all(Option::is_some),
        "budget misses retry after space is freed"
    );

    // Small App Center images use bounded content keys, independent of selection.
    let bounds = Rect {
        x: 0,
        y: 0,
        w: 32,
        h: 32,
    };
    assert!(canvas.center_icon(&[0; 8], bounds).is_err());
    for color in 0..=128_u8 {
        canvas.center_icon(&[color; 4096], bounds)?;
    }
    assert_eq!(canvas.center_icons.len(), 128);
    performance::reset();
    canvas.center_icon(&[128; 4096], bounds)?;
    assert_eq!(performance::snapshot().uploads, 0);
    canvas.reset()?;
    assert!(canvas.center_icons.is_empty());
    canvas.center_icon(&[128; 4096], bounds)?;
    assert_eq!(performance::snapshot().uploads, 1);
    Ok(())
}

#[test]
#[ignore = "requires an SDL accelerated backend and a graphical session"]
fn accelerated_atlas_survives_unchanged_artwork_refresh() -> Result<(), String> {
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let (canvas, _) = backend::initialize(&video, backend::RendererMode::Hardware, || {
        video
            .window("Atlas refresh regression", 480, 272)
            .hidden()
            .build()
            .map_err(|e| e.to_string())
    })?;
    let creator = canvas.texture_creator();
    let mut canvas = Screen::new(canvas, &creator)?;
    let layout = Layout::home(480, 272)?;
    let mut state = Launcher::new(Vec::new(), 3, 6)?;
    state.app_center = crate::app_center::Center::qa_samples()?.remove(0).1;
    let mut textures = artwork(&creator, &state);
    render(&mut canvas, &layout, &state, &textures)?;
    let expected = canvas.read_pixels(None, PixelFormatEnum::RGBA32)?;
    canvas.present();
    for _ in 0..5 {
        textures.refresh(&creator, &state);
        render(&mut canvas, &layout, &state, &textures)?;
        assert_eq!(canvas.read_pixels(None, PixelFormatEnum::RGBA32)?, expected);
        canvas.present();
    }
    Ok(())
}
