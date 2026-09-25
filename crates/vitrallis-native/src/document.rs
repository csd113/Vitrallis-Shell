//! Bounded UTF-8 document editing with exact LF/CRLF preservation and atomic saves.
use crate::files::{self, Temporary};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    fs,
    io::{self, Read, Write},
    ops::Range,
    path::{Path, PathBuf},
};
pub const MAX_BYTES: usize = 1024 * 1024;
#[derive(Debug)]
pub struct Document {
    text: String,
    starts: Vec<usize>,
    pub cursor: usize,
    pub anchor: Option<usize>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    original: Option<fs::Metadata>,
    original_digest: Option<[u8; 32]>,
    newline: &'static str,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            text: String::new(),
            starts: vec![0],
            cursor: 0,
            anchor: None,
            path: None,
            dirty: false,
            original: None,
            original_digest: None,
            newline: "\n",
        }
    }
}
impl Document {
    /// # Errors
    /// Rejects non-regular files, invalid UTF-8, changing files, and documents over 1 MiB.
    pub fn open(path: &Path) -> io::Result<Self> {
        let path = files::checked_path(path)?;
        let file = files::open_regular(&path)?;
        let original = file.metadata()?;
        if original.len() > MAX_BYTES as u64 {
            return Err(io::Error::other("Notepad opens files up to 1 MiB"));
        }
        let mut text = String::new();
        file.take(MAX_BYTES as u64 + 1).read_to_string(&mut text)?;
        if text.len() > MAX_BYTES {
            return Err(io::Error::other("File grew beyond 1 MiB"));
        }
        if !files::same_snapshot(&original, &fs::symlink_metadata(&path)?) {
            return Err(io::Error::other("File changed while opening"));
        }
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let original_digest = Some(Sha256::digest(text.as_bytes()).into());
        Ok(Self {
            text,
            starts,
            cursor: 0,
            anchor: None,
            path: Some(path),
            dirty: false,
            original: Some(original),
            original_digest,
            newline,
        })
    }
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
    #[must_use]
    pub const fn lines(&self) -> usize {
        self.starts.len()
    }
    /// Clamp a publicly assignable offset to a UTF-8 boundary inside the text.
    /// `cursor` and `anchor` are part of the component API, so a malformed
    /// external write degrades to the nearest boundary instead of panicking.
    fn boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        debug_assert!(self.text.is_char_boundary(offset));
        offset
    }
    #[must_use]
    pub fn row(&self) -> usize {
        let cursor = self.boundary(self.cursor);
        self.starts
            .partition_point(|&start| start <= cursor)
            .saturating_sub(1)
    }
    #[must_use]
    pub fn column(&self) -> usize {
        let cursor = self.boundary(self.cursor);
        let row = self
            .starts
            .partition_point(|&start| start <= cursor)
            .saturating_sub(1);
        self.text[self.starts[row]..cursor].chars().count()
    }
    #[must_use]
    pub fn line(&self, row: usize) -> &str {
        self.starts.get(row).map_or("", |&start| {
            self.text[start..self.starts.get(row + 1).copied().unwrap_or(self.text.len())]
                .trim_end_matches(['\r', '\n'])
        })
    }
    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        let anchor = self.boundary(self.anchor.unwrap_or(self.cursor));
        let cursor = self.boundary(self.cursor);
        anchor.min(cursor)..anchor.max(cursor)
    }
    #[must_use]
    pub fn line_start(&self, row: usize) -> Option<usize> {
        self.starts.get(row).copied()
    }
    /// # Errors
    /// Rejects invalid byte ranges and edits exceeding the size limit before mutation.
    pub fn replace(&mut self, range: Range<usize>, value: &str) -> io::Result<()> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return Err(io::Error::other("Invalid document edit boundary"));
        }
        if self.text.len() - (range.end - range.start) + value.len() > MAX_BYTES {
            return Err(io::Error::other("Document limit is 1 MiB"));
        }
        if range.is_empty() && value.is_empty() {
            return Ok(());
        }
        let first = self.starts.partition_point(|&i| i <= range.start);
        let last = self.starts.partition_point(|&i| i <= range.end);
        let added: Vec<_> = value
            .match_indices('\n')
            .map(|(i, _)| range.start + i + 1)
            .collect();
        for offset in &mut self.starts[last..] {
            *offset = *offset - (range.end - range.start) + value.len();
        }
        self.starts.splice(first..last, added);
        self.text.replace_range(range.clone(), value);
        self.cursor = range.start + value.len();
        self.anchor = None;
        self.dirty = true;
        Ok(())
    }
    /// # Errors
    /// Rejects edits exceeding the document limit.
    pub fn insert(&mut self, text: &str) -> io::Result<()> {
        self.replace(self.selection(), text)
    }
    /// # Errors
    /// Rejects edits exceeding the document limit.
    pub fn newline(&mut self) -> io::Result<()> {
        self.insert(self.newline)
    }
    /// # Errors
    /// Reports invalid editing boundaries.
    pub fn backspace(&mut self) -> io::Result<()> {
        let range = self.selection();
        if !range.is_empty() {
            return self.replace(range, "");
        }
        let previous = self.previous();
        self.replace(previous..self.cursor, "")
    }
    /// # Errors
    /// Reports invalid editing boundaries.
    pub fn delete(&mut self) -> io::Result<()> {
        let range = self.selection();
        if !range.is_empty() {
            return self.replace(range, "");
        }
        self.replace(self.cursor..self.next(), "")
    }
    fn previous(&self) -> usize {
        let cursor = self.boundary(self.cursor);
        if self.text[..cursor].ends_with("\r\n") {
            cursor - 2
        } else {
            self.text[..cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i)
        }
    }
    fn next(&self) -> usize {
        let cursor = self.boundary(self.cursor);
        if self.text[cursor..].starts_with("\r\n") {
            cursor + 2
        } else {
            cursor + self.text[cursor..].chars().next().map_or(0, char::len_utf8)
        }
    }
    pub fn horizontal(&mut self, right: bool, select: bool) {
        self.prepare_selection(select);
        self.cursor = if right { self.next() } else { self.previous() };
    }
    pub fn vertical(&mut self, delta: isize, select: bool) {
        let column = self.column();
        self.prepare_selection(select);
        let row = self
            .row()
            .saturating_add_signed(delta)
            .min(self.lines() - 1);
        self.place(row, column);
    }
    pub fn edge(&mut self, end: bool, select: bool) {
        self.prepare_selection(select);
        let row = self.row();
        self.cursor = self.starts[row] + if end { self.line(row).len() } else { 0 };
    }
    fn prepare_selection(&mut self, select: bool) {
        if select {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
    }
    pub fn place(&mut self, row: usize, column: usize) {
        let row = row.min(self.lines() - 1);
        let line = self.line(row);
        let byte = line
            .char_indices()
            .nth(column)
            .map_or(line.len(), |(i, _)| i);
        self.cursor = self.starts[row] + byte;
    }
    pub const fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.text.len();
    }
    /// Save/Save As stage the entire document next to the destination and sync before rename.
    /// # Errors
    /// Rejects collisions without explicit replacement permission, symlinks, external edits and I/O errors.
    pub fn save(&mut self, path: &Path, replace_existing: bool) -> io::Result<()> {
        let path = files::checked_path(path)?;
        let existing = match fs::symlink_metadata(&path) {
            Ok(m) => Some(m),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        let existing_digest = existing.as_ref().map(|_| digest_file(&path)).transpose()?;
        if let Some(metadata) = &existing {
            if !metadata.is_file() {
                return Err(io::Error::other(
                    "Destination must be a regular file, not a symlink",
                ));
            }
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(io::Error::other(
                    "Refusing to replace a hard-linked document",
                ));
            }
            if self.path.as_ref() == Some(&path) {
                if self
                    .original
                    .as_ref()
                    .is_none_or(|old| !files::same_snapshot(old, metadata))
                    || existing_digest != self.original_digest
                {
                    return Err(io::Error::other(
                        "Document changed on disk; use Save As to preserve your edits",
                    ));
                }
            } else if !replace_existing {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Destination already exists",
                ));
            }
        } else if self.path.as_ref() == Some(&path) && self.original.is_some() {
            return Err(io::Error::other(
                "Document was removed on disk; use Save As",
            ));
        }
        let (temporary, mut file) = Temporary::file(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing destination directory"))?,
        )?;
        file.write_all(self.text.as_bytes())?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(
            existing.as_ref().map_or(0o600, |m| m.mode() & 0o777),
        ))?;
        file.sync_all()?;
        drop(file);
        if let Some(before) = existing {
            if !files::same_snapshot(&before, &fs::symlink_metadata(&path)?)
                || Some(digest_file(&path)?) != existing_digest
            {
                return Err(io::Error::other("Destination changed while saving"));
            }
            fs::rename(&temporary.path, &path)?;
        } else {
            files::rename_new(&temporary.path, &path)?;
        }
        self.original = Some(fs::metadata(&path)?);
        self.original_digest = Some(Sha256::digest(self.text.as_bytes()).into());
        self.path = Some(path.clone());
        self.dirty = false;
        // Rename already succeeded if durability fails; report this explicitly and retain the saved state.
        fs::File::open(
            path.parent()
                .ok_or_else(|| io::Error::other("Missing parent"))?,
        )?
        .sync_all()
        .map_err(|e| io::Error::other(format!("Saved, but directory sync failed: {e}")))
    }
}
// Compare bytes as well as timestamps: coarse or remote filesystems can report
// identical metadata for different same-length contents written in one tick.
fn digest_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = files::open_regular(path)?;
    if file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(io::Error::other(
            "Save replacement exceeds the 1 MiB document limit",
        ));
    }
    let mut buffer = [0; 16 * 1024];
    let mut total = 0;
    let mut digest = Sha256::new();
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count;
        if total > MAX_BYTES {
            return Err(io::Error::other("Save target grew during verification"));
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_offsets_off_the_utf8_boundary_degrade_instead_of_panicking()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut document = Document::default();
        document.insert("aé\nb")?;
        // "a" at 0, "é" at 1..3, "\n" at 3, "b" at 4. Both fields are public.
        document.cursor = 2;
        document.anchor = Some(3);
        assert_eq!(document.column(), 1);
        assert_eq!(document.row(), 0);
        assert_eq!(document.selection(), 1..3);
        document.horizontal(true, true);
        document.horizontal(false, true);
        document.vertical(1, false);
        document.backspace()?;
        document.delete()?;
        document.edge(true, false);
        assert!(document.cursor <= document.text().len());
        assert!(document.text().is_char_boundary(document.cursor));
        // Offsets past the end clamp to the final boundary as well.
        document.cursor = usize::MAX;
        document.anchor = Some(usize::MAX);
        assert_eq!(document.row(), document.lines() - 1);
        assert_eq!(
            document.column(),
            document.line(document.lines() - 1).chars().count()
        );
        assert_eq!(document.selection(), 4..4);
        document.insert("!")?;
        assert_eq!(document.text(), "aé\n!");
        assert!(document.text().is_char_boundary(document.cursor));
        Ok(())
    }
}
