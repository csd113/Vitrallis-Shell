//! User documents are separate from XDG settings and disposable caches.
use std::{
    io,
    path::{Component, Path, PathBuf},
};

/// Build a canonical documents location without creating it.
/// # Errors
/// Rejects traversal, relative home paths and malformed app identities.
pub fn documents(home: &Path, id: &str) -> io::Result<PathBuf> {
    if !home.is_absolute()
        || home
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        || id.is_empty()
        || id.len() > 256
        || matches!(id, "." | "..")
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(io::Error::other("Invalid app documents location"));
    }
    Ok(home.join("documents").join(id))
}

/// Create a document directory only after checking every existing ancestor.
/// # Errors
/// Rejects symlinks and non-directories; reports filesystem errors.
pub fn create_documents(id: &str) -> io::Result<PathBuf> {
    let path = documents(&crate::home(), id)?;
    for parent in path.ancestors() {
        match parent.symlink_metadata() {
            Ok(info) if !info.is_dir() || info.file_type().is_symlink() => {
                return Err(io::Error::other("Unsafe documents directory"));
            }
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_use_home_and_stable_id_without_traversal() -> io::Result<()> {
        assert_eq!(
            documents(Path::new("/home/alex"), "io.vitrallis.notepad")?,
            Path::new("/home/alex/documents/io.vitrallis.notepad")
        );
        for id in [
            "",
            ".",
            "..",
            "../escape",
            "/absolute",
            "name/child",
            "bad\nname",
        ] {
            assert!(documents(Path::new("/home/alex"), id).is_err());
        }
        assert!(documents(Path::new("relative"), "app").is_err());
        assert!(documents(Path::new("/home/../tmp"), "app").is_err());
        Ok(())
    }
}
