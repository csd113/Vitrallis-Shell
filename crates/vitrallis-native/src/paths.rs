//! Replaceable applications and persistent data share one user-owned root.
use std::{
    io,
    path::{Component, Path, PathBuf},
};

/// Build a canonical persistent application location without creating it.
/// # Errors
/// Rejects traversal, relative home paths and malformed app identities.
pub fn app_data(home: &Path, id: &str) -> io::Result<PathBuf> {
    if !home.is_absolute()
        || home
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .any(|byte| *byte < 32 || *byte == 127)
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
        return Err(io::Error::other("Invalid application storage location"));
    }
    Ok(home.join("Documents/Vitrallis/AppData").join(id))
}

/// Build the replaceable package location without creating it.
/// # Errors
/// Rejects the same invalid home paths and identities as [`app_data`].
pub fn app_dir(home: &Path, id: &str) -> io::Result<PathBuf> {
    let _validated_data_path = app_data(home, id)?;
    Ok(home.join("Documents/Vitrallis/Apps").join(id))
}

/// Build the default document location inside persistent application data.
/// # Errors
/// Rejects the same invalid home paths and identities as [`app_data`].
pub fn documents(home: &Path, id: &str) -> io::Result<PathBuf> {
    Ok(app_data(home, id)?.join("Documents"))
}

/// Create a document directory only after checking every existing ancestor.
/// # Errors
/// Rejects symlinks and non-directories; reports filesystem errors.
pub fn create_documents(id: &str) -> io::Result<PathBuf> {
    create_at(&crate::home(), id)
}

fn create_at(home: &Path, id: &str) -> io::Result<PathBuf> {
    let path = documents(home, id)?;
    let data = app_data(home, id)?;
    #[cfg(unix)]
    let owner = {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(home)?.uid()
    };
    for parent in path.ancestors() {
        match parent.symlink_metadata() {
            Ok(info) => {
                if !info.is_dir() || info.file_type().is_symlink() {
                    return Err(io::Error::other("Unsafe documents directory"));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if (info.mode() & 0o022 != 0 && info.mode() & 0o1000 == 0)
                        || (parent.starts_with(&data)
                            && (info.uid() != owner || info.mode() & 0o077 != 0))
                    {
                        return Err(io::Error::other(
                            "Documents directory must be private and user-owned",
                        ));
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
    }
    let mut builder = std::fs::DirBuilder::new();
    let _recursive_builder = builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let _private_builder = builder.mode(0o700);
    }
    builder.create(&path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_use_home_and_stable_id_without_traversal() -> io::Result<()> {
        assert_eq!(
            documents(Path::new("/home/alex"), "io.vitrallis.notepad")?,
            Path::new("/home/alex/Documents/Vitrallis/AppData/io.vitrallis.notepad/Documents")
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
        assert_eq!(
            app_dir(Path::new("/home/alex"), "app")?,
            Path::new("/home/alex/Documents/Vitrallis/Apps/app")
        );
        assert_eq!(
            app_data(Path::new("/home/alex"), "app")?,
            Path::new("/home/alex/Documents/Vitrallis/AppData/app")
        );
        assert!(documents(Path::new("/home/bad\nname"), "app").is_err());
        assert!(documents(Path::new("relative"), "app").is_err());
        assert!(documents(Path::new("/home/../tmp"), "app").is_err());
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn document_creation_rejects_links_and_insecure_data_before_writing() -> io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let home = std::env::temp_dir()
            .canonicalize()?
            .join(format!("vitrallis-document-paths-{}", std::process::id()));
        std::fs::DirBuilder::new().mode(0o700).create(&home)?;
        let result = (|| {
            let docs = create_at(&home, "io.test.documents")?;
            assert_eq!(
                std::fs::metadata(&docs)?.permissions().mode() & 0o777,
                0o700
            );
            std::fs::remove_dir(&docs)?;
            let data = app_data(&home, "io.test.documents")?;
            std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755))?;
            assert!(create_at(&home, "io.test.documents").is_err());
            assert!(!docs.exists());
            std::fs::remove_dir(&data)?;
            std::os::unix::fs::symlink(&home, &data)?;
            assert!(create_at(&home, "io.test.documents").is_err());
            assert!(!home.join("Documents").join("Documents").exists());
            Ok(())
        })();
        std::fs::remove_dir_all(home)?;
        result
    }
}
