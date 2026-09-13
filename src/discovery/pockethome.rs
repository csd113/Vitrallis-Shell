//! Read-only integration with the stock `PocketHome` menu: Apps pages, JUCE
//! commands, stable IDs, and display preferences. Normalize into shared models.
use super::{Catalog, executable::resolve};
use crate::{
    app::{AppEntry, AppManifest},
    config::Paths,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn parse_catalog(text: &str, paths: &Paths) -> Result<Catalog, String> {
    let root: Value = serde_json::from_str(&without_trailing_commas(text))
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let pages = root["pages"].as_array().ok_or("missing pages array")?;
    let mut catalog = Catalog::default();
    let mut ids = BTreeMap::<String, usize>::new();
    for (page_index, page) in pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p["name"] == "Apps")
    {
        let Some(items) = page["items"].as_array() else {
            catalog
                .diagnostics
                .push(format!("page {page_index}: missing items array"));
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            match parse_entry(item, paths) {
                Ok(mut app) => {
                    if stock_utility(item, &app) {
                        continue;
                    }
                    // PocketHome has no IDs and permits duplicate commands. Keep all
                    // tiles, with deterministic content IDs and occurrence suffixes.
                    let occurrence = ids.entry(app.id.clone()).or_default();
                    *occurrence += 1;
                    if *occurrence > 1 {
                        app.id = format!("{}-{}", app.id, occurrence);
                    }
                    if let Some(reason) = &app.unavailable {
                        catalog.diagnostics.push(format!("{}: {reason}", app.name));
                    }
                    catalog.apps.push(app);
                }
                Err(error) => catalog
                    .diagnostics
                    .push(format!("page {page_index} item {index}: {error}")),
            }
        }
    }
    if let Some(command) = root.get("wifiCommand") {
        let item =
            serde_json::json!({"name": "System Settings", "shell": command, "icon": "wifiOff.png"});
        match parse_entry(&item, paths) {
            Ok(mut app) => {
                app.id = "vitrallis-wifi-settings".into();
                app.icon = None;
                catalog.apps.push(app);
            }
            Err(error) => catalog.diagnostics.push(format!("Wi-Fi settings: {error}")),
        }
    }
    Ok(catalog)
}

// This function is used only by the PocketHome importer. Verified upstream
// commands are documented in docs/devices/pocketchip/stock-source.md. Match
// argv exactly and check resolved executable provenance, never a visible label.
fn stock_utility(item: &Value, app: &AppEntry) -> bool {
    let Some(shell) = item["shell"].as_str() else {
        return false;
    };
    let Ok((program, args)) = command_tokens(shell) else {
        return false;
    };
    let signature = match program.as_str() {
        "vala-terminal" | "/usr/bin/vala-terminal" => args == ["-fs", "8", "-g", "20", "20"],
        "leafpad"
        | "/usr/bin/leafpad"
        | "l3afpad"
        | "/usr/bin/l3afpad"
        | "pcmanfm"
        | "/usr/bin/pcmanfm"
        | "lxterminal"
        | "/usr/bin/lxterminal" => args.is_empty(),
        _ => false,
    };
    if !signature {
        return false;
    }
    // A custom PATH executable shadowing the stock command remains discoverable.
    app.unavailable.is_some()
        || app.manifest.entry.parent() == Some(std::path::Path::new("/usr/bin"))
}

fn parse_entry(item: &Value, paths: &Paths) -> Result<AppEntry, String> {
    let name = item["name"].as_str().ok_or("name must be a string")?;
    let shell = item["shell"].as_str().ok_or("shell must be a string")?;
    let icon = item["icon"].as_str().ok_or("icon must be a string")?;
    if item.as_object().is_none_or(|fields| {
        fields
            .keys()
            .any(|key| !matches!(key.as_str(), "name" | "shell" | "icon"))
    }) {
        return Err("Device menu entries require name, shell and icon only".into());
    }
    let (program, args) = command_tokens(shell)?;
    let mut manifest = AppManifest {
        entry: paths.cwd.join(&program),
        args,
        ..AppManifest::default()
    };
    // Validate untrusted fields before searching for executables, or launching.
    let mut app = AppEntry {
        source: crate::app::AppSource::PocketHome,
        id: stable_id(name, shell),
        name: name.into(),
        icon: Some(paths.asset(if icon.is_empty() {
            "appIcons/default.png"
        } else {
            icon
        })),
        manifest: manifest.clone(),
        unavailable: None,
    };
    app.validate()?;
    let resolved = resolve(&program, &paths.cwd, &paths.search_path);
    if let Some(path) = resolved {
        manifest.entry = path;
    } else {
        app.unavailable = Some(format!("command not found or not executable: {program}"));
    }
    if app
        .icon
        .as_ref()
        .is_some_and(|p| !p.is_file() || p.metadata().is_ok_and(|m| m.len() == 0))
    {
        app.icon = Some(paths.asset("appIcons/default.png"));
    }
    app.manifest = manifest;
    Ok(app)
}

// JUCE fromTokens(command, true) groups using double quotes, retains quotes in
// arguments, treats single quotes/backslashes literally, and skips empty tokens.
// Only the executable is unquoted by JUCE's ActiveProcess. Never shell-expand.
fn command_tokens(command: &str) -> Result<(String, Vec<std::ffi::OsString>), String> {
    if command.contains('\0') {
        return Err("NUL in command".into());
    }
    let mut quoted = false;
    let mut current = String::new();
    let mut words = Vec::new();
    for c in command.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if matches!(c, ' ' | '\n' | '\r' | '\t') && !quoted {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if quoted {
        return Err("unclosed double quote in command".into());
    }
    if !current.is_empty() {
        words.push(current);
    }
    let mut words = words.into_iter();
    let program = words.next().ok_or("empty command")?;
    let program = program.trim_matches(['"', '\'']).to_owned();
    if program.is_empty() {
        return Err("empty executable".into());
    }
    Ok((program, words.map(Into::into).collect()))
}

fn stable_id(name: &str, shell: &str) -> String {
    // Fixed FNV-1a (not randomized DefaultHasher), independent of position/icon.
    let hash = name
        .bytes()
        .chain([0])
        .chain(shell.bytes())
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("pockethome-{hash:016x}")
}

// The shipped JUCE config accepts trailing commas. Replace just those commas
// outside strings, preserving offsets for serde_json diagnostics.
fn without_trailing_commas(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in text.bytes().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b','
            && text.as_bytes()[..index]
                .iter()
                .rev()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| !matches!(b, b'[' | b'{' | b',' | b':'))
            && text.as_bytes()[index + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| matches!(b, b']' | b'}'))
        {
            bytes[index] = b' ';
        }
    }
    // Only ASCII commas were replaced in a valid UTF-8 string.
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paths() -> Paths {
        Paths {
            explicit_catalog: None,
            asset_roots: vec!["/missing-assets".into()],
            cwd: "/".into(),
            search_path: vec!["/bin".into(), "/usr/bin".into()],
        }
    }
    #[test]
    fn custom_path_shadow_and_non_stock_arguments_are_preserved()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let mut paths = paths();
        paths.search_path.insert(0, scratch.0.clone());
        #[cfg(unix)]
        std::os::unix::fs::symlink("/bin/sh", scratch.0.join("leafpad"))?;
        let root = serde_json::json!({"pages":[{"name":"Apps","items":[
            {"name":"Custom","shell":"leafpad","icon":""},
            {"name":"Terminal","shell":"vala-terminal -fs 12","icon":""},
            {"name":"Files","shell":"pcmanfm /work","icon":""},
            {"name":"Notepad","shell":"/opt/editor/leafpad","icon":""}
        ]}]});
        let catalog = parse_catalog(&root.to_string(), &paths)?;
        assert_eq!(catalog.apps.len(), 4);
        assert_eq!(catalog.apps[0].manifest.entry, scratch.0.join("leafpad"));
        Ok(())
    }

    #[test]
    fn ordered_apps_pages_only_skip_invalid_keep_missing_and_duplicates() -> Result<(), String> {
        let text = r#"{"pages":[{"name":"Settings","items":[{"name":"No","shell":"sh","icon":""}]},{"name":"Apps","items":[
        {"name":"Zulu","shell":"sh","icon":""},
        {"name":"Invalid","shell":3,"icon":""},
        {"name":"Alpha","shell":"/no/such/vitrallis-command","icon":"lost.png"},
        {"name":"Zulu","shell":"sh","icon":""},
        ]},{"name":"Apps","items":[{"name":"Last","shell":"sh","icon":""}]}]}"#;
        let result = parse_catalog(text, &paths())?;
        assert_eq!(
            result
                .apps
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            ["Zulu", "Alpha", "Zulu", "Last"]
        );
        assert_eq!(result.diagnostics.len(), 2);
        assert!(result.apps[0].unavailable.is_none());
        assert!(result.apps[1].unavailable.is_some());
        assert_ne!(result.apps[0].id, result.apps[2].id);
        assert_eq!(result.apps[1].icon, Some("/appIcons/default.png".into()));
        let again = parse_catalog(text, &paths())?;
        assert_eq!(again.apps[0].id, result.apps[0].id);
        assert_eq!(stable_id("Zulu", "sh"), result.apps[0].id);
        Ok(())
    }
    #[test]
    fn juce_commands_preserve_quotes_and_do_not_expand_shell_syntax() -> Result<(), String> {
        let (program, args) =
            command_tokens(r#""/bin/echo" "two words" 'single words' $HOME a\ b ;"#)?;
        assert_eq!(program, "/bin/echo");
        let args: Vec<_> = args.iter().map(|a| a.to_string_lossy()).collect();
        assert_eq!(
            args,
            [
                "\"two words\"",
                "'single",
                "words'",
                "$HOME",
                "a\\",
                "b",
                ";"
            ]
        );
        assert!(command_tokens("").is_err());
        assert!(command_tokens("sh \"unfinished").is_err());
        assert!(command_tokens("sh\0bad").is_err());
        Ok(())
    }
    #[test]
    fn comma_normalization_preserves_escaped_strings_and_rejects_other_invalid_json() {
        let value = r#"{"text":"a,] b\",} c", "items":[1,],}"#;
        let normalized = without_trailing_commas(value);
        let result: Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(result["text"], "a,] b\",} c");
        assert_eq!(result["items"][0], 1);
        for text in ["{}", "{bad}", "{\"pages\":false}", "{\"pages\":[,]}"] {
            assert!(parse_catalog(text, &paths()).is_err());
        }
    }
    #[test]
    fn device_menu_rejects_non_contract_fields() {
        for (key, value) in [
            ("env", serde_json::json!({"KEY":"value"})),
            ("cwd", serde_json::json!("/tmp")),
            ("args", serde_json::json!(["arg"])),
            ("name", serde_json::json!("\n")),
        ] {
            let mut item = serde_json::json!({"name":"Tool", "shell":"sh", "icon":""});
            item[key] = value;
            assert!(parse_entry(&item, &paths()).is_err());
        }
    }

    #[test]
    fn path_order_relative_commands_and_asset_fallbacks() -> Result<(), Box<dyn std::error::Error>>
    {
        let scratch = crate::test_support::Scratch::new()?;
        let root = &scratch.0;
        std::fs::create_dir(root.join("first"))?;
        std::fs::create_dir(root.join("second"))?;
        std::fs::create_dir(root.join("second/appIcons"))?;
        std::fs::write(root.join("second/appIcons/icon.png"), "asset")?;
        let mut paths = paths();
        paths.asset_roots = vec![root.join("first"), root.join("second")];
        paths.cwd = root.clone();
        assert_eq!(
            paths.asset("appIcons/icon.png"),
            root.join("second/appIcons/icon.png")
        );
        assert_eq!(
            paths.asset("/absolute/icon.png"),
            std::path::PathBuf::from("/absolute/icon.png")
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/bin/sh", root.join("first/tool"))?;
            std::os::unix::fs::symlink("/bin/sh", root.join("second/tool"))?;
            assert_eq!(
                resolve("tool", root, &paths.asset_roots),
                Some(root.join("first/tool"))
            );
            assert_eq!(
                resolve("./second/tool", root, &[]),
                Some(root.join("./second/tool"))
            );
        }
        std::fs::write(root.join("first/not-executable"), "test")?;
        assert!(resolve("not-executable", root, &paths.asset_roots).is_none());
        Ok(())
    }
}
