use super::{
    browser::Browser,
    document::{Document, MAX_BYTES},
    files,
};
use std::{
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "vitrallis-native-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path.canonicalize()?))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn insertion_deletion_and_unicode_boundaries() -> io::Result<()> {
    let mut doc = Document::default();
    doc.insert("aé中🙂")?;
    assert_eq!(doc.column(), 4);
    doc.horizontal(false, false);
    doc.backspace()?;
    assert_eq!(doc.text(), "aé🙂");
    doc.edge(false, false);
    doc.delete()?;
    assert_eq!(doc.text(), "é🙂");
    assert!(doc.replace(1..2, "bad").is_err());
    assert_eq!(doc.text(), "é🙂");
    doc.select_all();
    doc.insert("replacement")?;
    assert_eq!(doc.text(), "replacement");
    assert!(doc.dirty);
    Ok(())
}
#[test]
fn line_index_tracks_edits_without_rebuilding_text() -> io::Result<()> {
    let mut doc = Document::default();
    doc.insert("one\ntwo\nthree\n")?;
    doc.place(1, 2);
    doc.newline()?;
    assert_eq!(doc.text(), "one\ntw\no\nthree\n");
    assert_eq!(doc.lines(), 5);
    doc.replace(2..8, "X\nY")?;
    assert_eq!(doc.text(), "onX\nY\nthree\n");
    assert_eq!(doc.lines(), 4);
    for (row, text) in ["onX", "Y", "three", ""].iter().enumerate() {
        assert_eq!(doc.line(row), *text);
    }
    doc.place(2, 99);
    assert_eq!(doc.column(), 5);
    doc.vertical(-1, false);
    assert_eq!(doc.column(), 1);
    Ok(())
}
#[test]
fn crlf_empty_and_no_final_newline_roundtrip() -> io::Result<()> {
    let dir = Scratch::new()?;
    for (i, content) in ["", "one", "one\ntwo\n", "one\r\ntwo", "é🙂\r\n\r\n"]
        .iter()
        .enumerate()
    {
        let path = dir.0.join(format!("{i}.txt"));
        fs::write(&path, content)?;
        let mut doc = Document::open(&path)?;
        assert!(!doc.dirty);
        doc.save(&path, false)?;
        assert_eq!(fs::read_to_string(path)?, *content);
    }
    let path = dir.0.join("crlf");
    fs::write(&path, "a\r\nb")?;
    let mut doc = Document::open(&path)?;
    doc.place(1, 0);
    doc.backspace()?;
    assert_eq!(doc.text(), "ab");
    doc.newline()?;
    assert_eq!(doc.text(), "a\r\nb");
    Ok(())
}
#[test]
fn atomic_save_replaces_inode_and_preserves_permissions() -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = Scratch::new()?;
    let path = dir.0.join("note with spaces");
    fs::write(&path, "before")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
    let before = fs::metadata(&path)?;
    let mut doc = Document::open(&path)?;
    doc.select_all();
    doc.insert("after")?;
    doc.save(&path, false)?;
    assert_eq!(fs::read_to_string(&path)?, "after");
    assert_ne!(before.ino(), fs::metadata(&path)?.ino());
    assert_eq!(fs::metadata(&path)?.mode() & 0o777, 0o640);
    assert!(!doc.dirty);
    assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
    Ok(())
}
#[test]
fn failed_save_and_external_changes_keep_existing_bytes() -> io::Result<()> {
    let dir = Scratch::new()?;
    let path = dir.0.join("note");
    fs::write(&path, "original")?;
    let mut doc = Document::open(&path)?;
    doc.insert("edit")?;
    fs::write(&path, "external")?;
    assert!(doc.save(&path, false).is_err());
    assert_eq!(fs::read_to_string(&path)?, "external");
    assert!(doc.dirty);
    assert!(doc.save(&dir.0.join("missing/note"), false).is_err());
    assert!(doc.dirty);
    let new = dir.0.join("copy");
    doc.save(&new, false)?;
    assert_eq!(fs::read_to_string(new)?, "editoriginal");
    assert!(!doc.dirty);
    Ok(())
}
#[test]
fn save_as_requires_replacement_and_rejects_links() -> io::Result<()> {
    let dir = Scratch::new()?;
    let path = dir.0.join("existing");
    fs::write(&path, "keep")?;
    let mut doc = Document::default();
    doc.insert("new")?;
    assert!(doc.save(&path, false).is_err());
    assert_eq!(fs::read(&path)?, b"keep");
    let link = dir.0.join("link");
    std::os::unix::fs::symlink(&path, &link)?;
    assert!(doc.save(&link, true).is_err());
    assert!(Document::open(&link).is_err());
    let hard = dir.0.join("hard");
    fs::hard_link(&path, &hard)?;
    assert!(doc.save(&path, true).is_err());
    fs::remove_file(hard)?;
    doc.save(&path, true)?;
    assert_eq!(fs::read(&path)?, b"new");
    Ok(())
}
#[test]
fn document_size_guard_and_invalid_utf8() -> io::Result<()> {
    let dir = Scratch::new()?;
    let path = dir.0.join("large");
    fs::File::create(&path)?.set_len(MAX_BYTES as u64 + 1)?;
    assert!(Document::open(&path).is_err());
    fs::write(&path, [0xff, 0x00])?;
    assert!(Document::open(&path).is_err());
    let mut doc = Document::default();
    doc.insert(&"x".repeat(MAX_BYTES))?;
    assert!(doc.insert("x").is_err());
    assert_eq!(doc.text().len(), MAX_BYTES);
    doc.edge(false, false);
    doc.delete()?;
    doc.insert("y")?;
    Ok(())
}
#[test]
fn directories_sort_filter_and_parent_navigation() -> io::Result<()> {
    let dir = Scratch::new()?;
    fs::create_dir(dir.0.join("z-directory"))?;
    fs::write(dir.0.join("Beta"), "")?;
    fs::write(dir.0.join("alpha"), "")?;
    fs::write(dir.0.join(".hidden"), "")?;
    let mut b = Browser::new(&dir.0)?;
    assert_eq!(
        b.entries
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>(),
        ["z-directory", "alpha", "Beta"]
    );
    b.hidden = true;
    b.refresh()?;
    assert_eq!(b.entries.len(), 4);
    b.selected = 1;
    assert!(!b.activate()?);
    assert!(b.path.ends_with("z-directory"));
    b.parent()?;
    assert_eq!(b.path, dir.0);
    Ok(())
}
#[test]
fn failed_directory_navigation_keeps_old_listing() -> io::Result<()> {
    let dir = Scratch::new()?;
    fs::write(dir.0.join("file"), "")?;
    let mut b = Browser::new(&dir.0)?;
    assert!(b.enter(&dir.0.join("absent")).is_err());
    assert_eq!(b.path, dir.0);
    assert_eq!(b.entries.len(), 1);
    Ok(())
}
#[test]
fn folder_rename_move_and_collision_guards() -> io::Result<()> {
    let dir = Scratch::new()?;
    files::create_folder(&dir.0, "folder")?;
    assert!(files::create_folder(&dir.0, "folder").is_err());
    for name in [".", "..", "a/b", "a\\b", ""] {
        assert!(files::create_folder(&dir.0, name).is_err());
    }
    let source = dir.0.join("folder");
    let target = dir.0.join("renamed");
    files::move_entry(&source, &target)?;
    assert!(files::move_entry(&target, &target.join("inside")).is_err());
    fs::write(&source, "occupied")?;
    assert!(files::move_entry(&target, &source).is_err());
    assert_eq!(fs::read(&source)?, b"occupied");
    std::os::unix::fs::symlink(dir.0.join("missing"), dir.0.join("dangling"))?;
    assert!(files::move_entry(&source, &dir.0.join("dangling")).is_err());
    Ok(())
}
#[test]
fn stream_copy_large_file_and_never_overwrite() -> io::Result<()> {
    use std::io::Write;
    let dir = Scratch::new()?;
    let source = dir.0.join("source");
    let target = dir.0.join("target");
    let mut file = fs::File::create(&source)?;
    for _ in 0..512 {
        file.write_all(&[42; 8192])?;
    }
    drop(file);
    let mut calls = 0;
    let mut bytes = 0;
    files::copy_entry(&source, &target, &mut |n| {
        assert!(n >= bytes);
        assert!(n - bytes <= files::COPY_BUFFER as u64);
        bytes = n;
        calls += 1;
    })?;
    assert!(calls > 1);
    assert_eq!(bytes, 4 * 1024 * 1024);
    assert_eq!(fs::read(&source)?, fs::read(&target)?);
    assert!(files::copy_entry(&source, &target, &mut |_| {}).is_err());
    assert!(files::copy_entry(&source, &source, &mut |_| {}).is_err());
    Ok(())
}
#[test]
fn directory_copy_failure_rolls_back_private_tree() -> io::Result<()> {
    let dir = Scratch::new()?;
    let source = dir.0.join("source");
    fs::create_dir(&source)?;
    fs::write(source.join("note"), "text")?;
    std::os::unix::fs::symlink(dir.0.join("elsewhere"), source.join("link"))?;
    let target = dir.0.join("target");
    assert!(files::copy_entry(&source, &target, &mut |_| {}).is_err());
    assert!(!target.exists());
    assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
    fs::remove_file(source.join("link"))?;
    files::copy_entry(&source, &target, &mut |_| {})?;
    assert_eq!(fs::read(target.join("note"))?, b"text");
    Ok(())
}
#[test]
fn delete_requires_confirmation_and_does_not_follow_symlinks() -> io::Result<()> {
    let dir = Scratch::new()?;
    let file = dir.0.join("keep");
    fs::write(&file, "keep")?;
    assert!(files::delete(&file, false).is_err());
    assert!(file.exists());
    assert!(files::delete(&dir.0.join(".."), true).is_err());
    assert!(files::delete(&dir.0.join("."), true).is_err());
    let link = dir.0.join("link");
    std::os::unix::fs::symlink(&file, &link)?;
    files::delete(&link, true)?;
    assert!(file.exists());
    assert!(files::delete(&dir.0, true).is_err());
    files::delete(&file, true)?;
    assert!(!file.exists());
    Ok(())
}
#[test]
fn content_dispatch_is_bounded_and_does_not_execute() -> io::Result<()> {
    let dir = Scratch::new()?;
    let text = dir.0.join("no-extension");
    fs::write(&text, "Hello é🙂\n")?;
    assert_eq!(files::handler(&text)?, files::Handler::Text);
    let binary = dir.0.join("pretend.txt");
    fs::write(&binary, b"hello\0world")?;
    assert_eq!(files::handler(&binary)?, files::Handler::Properties);
    std::os::unix::fs::symlink(&text, dir.0.join("link"))?;
    assert_eq!(
        files::handler(&dir.0.join("link"))?,
        files::Handler::Properties
    );
    Ok(())
}
#[test]
fn listing_thousands_is_a_single_bounded_sorted_model() -> io::Result<()> {
    let dir = Scratch::new()?;
    for i in (0..3000).rev() {
        fs::write(dir.0.join(format!("entry-{i:04}")), "")?;
    }
    let mut browser = Browser::new(&dir.0)?;
    assert_eq!(browser.entries.len(), 3000);
    assert_eq!(browser.entries[0].label, "entry-0000");
    browser.move_by(2999, 10);
    assert_eq!(browser.offset, 2990);
    browser.move_by(-2999, 10);
    assert_eq!(browser.offset, 0);
    Ok(())
}

#[test]
fn growing_copy_fails_without_publishing_and_non_utf8_paths_are_preserved() -> io::Result<()> {
    use std::{io::Write, os::unix::ffi::OsStringExt};
    let dir = Scratch::new()?;
    // Linux preserves arbitrary non-NUL filename bytes; APFS requires Unicode.
    let name = if cfg!(target_os = "linux") {
        std::ffi::OsString::from_vec(b"name-\xff".to_vec())
    } else {
        std::ffi::OsString::from("name-é")
    };
    let source = dir.0.join(name);
    fs::write(&source, vec![b'x'; files::COPY_BUFFER * 2])?;
    let target = dir.0.join("copy");
    let mut append = fs::OpenOptions::new().append(true).open(&source)?;
    let mut failure = None;
    let result = files::copy_entry(&source, &target, &mut |_| {
        if let Err(error) = append.write_all(&[b'y'; 1024]) {
            failure = Some(error);
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    assert!(result.is_err());
    assert!(!target.exists());
    assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
    let mut doc = Document::open(&source)?;
    doc.insert("UTF-8 content, OS-byte path\n")?;
    doc.save(&source, false)?;
    assert_eq!(doc.path, Some(source));
    Ok(())
}

#[test]
fn permission_errors_keep_source_and_document_unchanged() -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = Scratch::new()?;
    // Root bypasses discretionary permissions; the host/unprivileged CI run covers this case.
    if fs::metadata(&dir.0)?.uid() == 0 {
        return Ok(());
    }
    let source = dir.0.join("source");
    fs::write(&source, "keep")?;
    let mut doc = Document::open(&source)?;
    doc.insert("edit")?;
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500))?;
    let save = doc.save(&source, false);
    let copy = files::copy_entry(&source, &dir.0.join("copy"), &mut |_| {});
    let create = files::create_folder(&dir.0, "new");
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700))?;
    assert!(save.is_err() && copy.is_err() && create.is_err());
    assert!(doc.dirty);
    assert_eq!(fs::read_to_string(source)?, "keep");
    assert_eq!(fs::read_dir(&dir.0)?.count(), 1);
    Ok(())
}
