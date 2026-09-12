//! Bundled application registration and requests through the normal process owner.
use crate::{
    app::{AppEntry, AppManifest, AppSource},
    discovery::Catalog,
    launcher::Launcher,
};
use std::path::Path;
use vitrallis_native::{APPLICATIONS, ipc::Broker};

pub fn integrate(catalog: &mut Catalog) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = executable
        .parent()
        .ok_or("Cannot locate bundled applications")?;
    let apps = entries(parent);
    // Native IDs are reserved. A separately installed package cannot impersonate one.
    catalog
        .apps
        .retain(|entry| !APPLICATIONS.iter().any(|app| entry.id == app.id));
    catalog.apps.splice(0..0, apps);
    Ok(())
}
fn entries(directory: &Path) -> Vec<AppEntry> {
    APPLICATIONS
        .iter()
        .map(|app| {
            let entry = directory.join(app.executable);
            let unavailable = std::fs::metadata(&entry)
                .map_err(|e| e.to_string())
                .and_then(|metadata| {
                    if !metadata.is_file() {
                        return Err("not a regular executable".into());
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if metadata.permissions().mode() & 0o111 == 0 {
                            return Err("not executable".into());
                        }
                    }
                    Ok(())
                })
                .err()
                .map(|error| {
                    format!(
                        "Bundled {} is unavailable: {error}. Repair the Vitrallis installation.",
                        app.name
                    )
                });
            AppEntry {
                id: app.id.into(),
                source: AppSource::Native,
                name: app.name.into(),
                icon: None,
                unavailable,
                manifest: AppManifest {
                    entry,
                    cwd: Some(vitrallis_native::home()),
                    ..AppManifest::default()
                },
            }
        })
        .collect()
}
pub fn icon(app: &AppEntry) -> Option<&'static [u8]> {
    if app.source != AppSource::Native {
        return None;
    }
    match app.id.as_str() {
        "io.vitrallis.terminal" => Some(include_bytes!("../assets/native/terminal.png")),
        "io.vitrallis.notepad" => Some(include_bytes!("../assets/native/notepad.png")),
        "io.vitrallis.files" => Some(include_bytes!("../assets/native/files.png")),
        _ => None,
    }
}
pub fn configure(apps: &mut [AppEntry], broker: &Broker) {
    for app in apps {
        if app.source == AppSource::Native {
            app.manifest.env.insert(
                vitrallis_native::ipc::ENV.into(),
                broker.path.clone().into_os_string(),
            );
        }
    }
}
pub fn inherit_runtime(apps: &mut [AppEntry], previous: &[AppEntry]) {
    for app in apps
        .iter_mut()
        .filter(|app| app.source == AppSource::Native)
    {
        if let Some(environment) = previous
            .iter()
            .find(|old| old.id == app.id)
            .map(|old| &old.manifest.env)
        {
            app.manifest.env.clone_from(environment);
        }
    }
}
pub fn broker(state: &mut Launcher) -> Result<Broker, String> {
    let broker = Broker::new().map_err(|e| format!("Native application requests: {e}"))?;
    configure(&mut state.apps, &broker);
    Ok(broker)
}
pub fn requested(broker: &Broker, state: &mut Launcher) -> Result<Option<usize>, String> {
    // Leave requests in the bounded OS socket queue until the current action or
    // confirmation finishes. In particular, activation must not dismiss an error.
    if state.phase == crate::launcher::Phase::Launching
        || state.error.is_some()
        || state.settings.open
        || state.app_center.open
        || state.app_center.busy
    {
        return Ok(None);
    }
    let Some(path) = broker.receive().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    if vitrallis_native::files::handler(&path).map_err(|e| e.to_string())?
        != vitrallis_native::files::Handler::Text
    {
        return Err("Notepad can open only readable text files".into());
    }
    let index = state
        .apps
        .iter()
        .position(|app| app.source == AppSource::Native && app.id == "io.vitrallis.notepad")
        .ok_or("Bundled Notepad is unavailable")?;
    state.apps[index].manifest.args = vec!["--".into(), path.into_os_string()];
    state.returned_home();
    Ok(state.input(crate::input::Action::SelectAndActivate(index)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs, io,
        os::unix::{ffi::OsStrExt, fs::PermissionsExt, net::UnixDatagram},
    };
    #[test]
    fn native_registry_ids_icons_resolution_and_missing_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let mut apps = entries(&scratch.0);
        assert_eq!(
            apps.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            [
                "io.vitrallis.terminal",
                "io.vitrallis.notepad",
                "io.vitrallis.files"
            ]
        );
        for app in &apps {
            assert_eq!(app.source, AppSource::Native);
            assert!(
                app.unavailable
                    .as_deref()
                    .is_some_and(|message| message.contains("Repair"))
            );
            let bytes = icon(app).ok_or("Missing original icon")?;
            let decoder = png::Decoder::new(io::Cursor::new(bytes));
            let reader = decoder.read_info()?;
            assert_eq!((reader.info().width, reader.info().height), (128, 128));
            fs::write(&app.manifest.entry, "#!/bin/sh\nexit 0\n")?;
            fs::set_permissions(&app.manifest.entry, fs::Permissions::from_mode(0o755))?;
        }
        apps = entries(&scratch.0);
        assert!(
            apps.iter()
                .all(|app| app.unavailable.is_none() && app.manifest.runtime.is_none())
        );
        for app in apps {
            assert!(crate::process::command(&app)?.status()?.success());
        }
        Ok(())
    }
    #[test]
    fn files_requests_use_normal_opening_and_wait_for_safe_launcher_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let note = scratch.0.join("note with spaces.txt");
        fs::write(&note, "text\n")?;
        let note = note.canonicalize()?;
        let mut state = Launcher::new(entries(&scratch.0), 3, 6)?;
        let broker = broker(&mut state)?;
        let sender = UnixDatagram::unbound()?;
        sender.send_to(note.as_os_str().as_bytes(), &broker.path)?;
        state.failed("Existing error must be acknowledged".into());
        assert_eq!(requested(&broker, &mut state)?, None);
        state.finished("Ready".into());
        assert_eq!(requested(&broker, &mut state)?, Some(1));
        assert_eq!(state.phase, crate::launcher::Phase::Launching);
        assert_eq!(state.opening.as_deref(), Some("Notepad"));
        assert_eq!(
            state.apps[1].manifest.args,
            [std::ffi::OsString::from("--"), note.into_os_string()]
        );
        Ok(())
    }
}
