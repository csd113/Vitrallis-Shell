//! Refresh only changed sources; retain failed loads until their source changes.
use super::{Launcher, Texture, TextureCreator, WindowContext, decode_icon};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
};

const ICON_BUDGET: u32 = 16 * 1024 * 1024;
// Filtering belongs to the source identity; retained textures need no state changes.
type Key = ([u8; 32], bool);

pub struct Artwork<'a> {
    textures: Vec<Option<Texture<'a>>>,
    keys: Vec<Option<Key>>,
}
impl<'a> std::ops::Deref for Artwork<'a> {
    type Target = [Option<Texture<'a>>];
    fn deref(&self) -> &Self::Target {
        &self.textures
    }
}

fn read_image(path: &std::path::Path) -> Result<Vec<u8>, String> {
    if !std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("expected a regular image file".into());
    }
    let mut options = std::fs::OpenOptions::new();
    let _read_options = options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // User artwork may use symlinks, but a pathname race must not open a
        // blocking FIFO on the render thread. Validate the opened descriptor.
        let _nonblocking_open = options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("expected a regular image file".into());
    }
    let mut bytes = Vec::new();
    let _bytes_read = file
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("image exceeds 1 MiB".into());
    }
    Ok(bytes)
}

fn source(state: &Launcher, index: usize) -> Result<Option<Vec<u8>>, String> {
    if let Some(app) = state.apps.get(index) {
        if let Some(bytes) = crate::shortcuts::icon(app) {
            return Ok(Some(bytes));
        }
        let builtin = crate::native::icon(app).or_else(|| {
            if app.source == crate::app::AppSource::Folder {
                Some(include_bytes!("../../assets/native/files.png").as_slice())
            } else if app.id == crate::app_center::TILE_ID {
                Some(include_bytes!("../../assets/system/apps.png").as_slice())
            } else if app.is_system_settings() {
                Some(include_bytes!("../../assets/system/gear.png").as_slice())
            } else {
                None
            }
        });
        if let Some(bytes) = builtin {
            return Ok(Some(bytes.to_vec()));
        }
        return app.icon.as_deref().map(read_image).transpose();
    }
    if index == state.apps.len() {
        return state
            .preferences
            .wallpaper
            .as_deref()
            .map(read_image)
            .transpose();
    }
    Ok(super::system::ASSETS
        .get(
            index
                .checked_sub(state.apps.len())
                .and_then(|offset| offset.checked_sub(1))
                .ok_or("Invalid artwork index")?,
        )
        .map(|bytes| bytes.to_vec()))
}

pub fn artwork<'a>(creator: &'a TextureCreator<WindowContext>, state: &Launcher) -> Artwork<'a> {
    let mut artwork = Artwork {
        textures: Vec::new(),
        keys: Vec::new(),
    };
    artwork.refresh(creator, state);
    artwork
}

impl<'a> Artwork<'a> {
    pub fn reset(&mut self, creator: &'a TextureCreator<WindowContext>, state: &Launcher) {
        self.textures.clear();
        self.keys.clear();
        self.refresh(creator, state);
    }
    pub fn refresh(&mut self, creator: &'a TextureCreator<WindowContext>, state: &Launcher) {
        // Hash one bounded source at a time, only on catalogue/preference refresh.
        // This detects in-place file edits without retaining decoded CPU pixels.
        let Some(source_count) = state
            .apps
            .len()
            .checked_add(super::system::ASSETS.len())
            .and_then(|count| count.checked_add(1))
        else {
            eprintln!("level=error event=artwork_count_overflow");
            return;
        };
        let mut keys: Vec<_> = (0..source_count)
            .map(|index| match source(state, index) {
                Ok(source_bytes) => source_bytes.map(|bytes| {
                    (
                        <[u8; 32]>::from(Sha256::digest(&bytes)),
                        index != state.apps.len(),
                    )
                }),
                Err(error) => {
                    eprintln!("level=warn event=icon_fallback index={index} message={error:?}");
                    None
                }
            })
            .collect();
        let wanted: HashSet<_> = keys.iter().flatten().copied().collect();
        let mut retained: HashMap<Key, Vec<Option<Texture<'a>>>> = HashMap::new();
        for (key, texture) in self.keys.drain(..).zip(self.textures.drain(..)) {
            if let Some(retained_key) = key.filter(|identity| wanted.contains(identity)) {
                retained.entry(retained_key).or_default().push(texture);
            }
        }
        // Drop obsolete textures before uploading replacements. Existing and new
        // icon allocations together stay within the original retained budget.
        let mut textures: Vec<_> = keys
            .iter()
            .map(|key| key.and_then(|identity| retained.get_mut(&identity).and_then(Vec::pop)))
            .collect();
        drop(retained);
        let mut remaining = remaining_budget(textures.iter_mut().take(state.apps.len()).flatten());
        for (index, (key, slot)) in keys.iter_mut().zip(&mut textures).enumerate() {
            if key.is_none() || slot.is_some() {
                continue;
            }
            let budget = if index < state.apps.len() {
                remaining
            } else {
                512 * 512 * 4
            };
            if budget == 0 {
                *key = None;
                continue;
            }
            let result = source(state, index).and_then(|source_bytes| {
                let bytes = source_bytes.ok_or("image source disappeared")?;
                if Some((
                    <[u8; 32]>::from(Sha256::digest(&bytes)),
                    index != state.apps.len(),
                )) != *key
                {
                    *key = None;
                    return Err("image changed during refresh".into());
                }
                let surface = decode_icon(&bytes)?;
                let size = surface
                    .width()
                    .checked_mul(surface.height())
                    .and_then(|area| area.checked_mul(4))
                    .ok_or("Artwork size overflow")?;
                if size > budget {
                    *key = None;
                    return Err("artwork budget exhausted".into());
                }
                let mut texture = creator
                    .create_texture_from_surface(&surface)
                    .map_err(|e| e.to_string())?;
                texture.set_scale_mode(if index == state.apps.len() {
                    sdl2::render::ScaleMode::Nearest
                } else {
                    sdl2::render::ScaleMode::Linear
                });
                #[cfg(test)]
                super::performance::count(|c| c.uploads = c.uploads.saturating_add(1));
                if index < state.apps.len() {
                    remaining = remaining
                        .checked_sub(size)
                        .ok_or("Artwork budget exhausted")?;
                }
                Ok(texture)
            });
            *slot = Some(match result {
                Ok(texture) => Some(texture),
                Err(error) => {
                    eprintln!("level=warn event=icon_fallback index={index} message={error:?}");
                    None
                }
            });
        }
        self.textures = textures.into_iter().map(Option::flatten).collect();
        self.keys = keys;
    }
}

// Retained app textures count against the same budget as newly uploaded ones.
fn remaining_budget<'slot, 'texture: 'slot>(
    textures: impl Iterator<Item = &'slot mut Option<Texture<'texture>>>,
) -> u32 {
    let mut remaining = ICON_BUDGET;
    for slot in textures {
        if let Some(texture) = slot {
            let info = texture.query();
            let bytes = info
                .width
                .checked_mul(info.height)
                .and_then(|area| area.checked_mul(4));
            if let Some(unused) = bytes.and_then(|size| remaining.checked_sub(size)) {
                remaining = unused;
            } else {
                *slot = None;
            }
        }
    }
    remaining
}
