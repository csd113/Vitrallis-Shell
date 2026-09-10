use super::{Catalog, Discovery};
use crate::{
    app::{AppEntry, AppManifest},
    config::Paths,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

pub struct Marshmallow<'a> {
    pub paths: &'a Paths,
    pub explicit_config: bool,
}
impl Discovery for Marshmallow<'_> {
    fn discover(&self) -> Result<Catalog, String> {
        let path = match &self.paths.user_config {
            Some(path)
                if self.explicit_config
                    || path
                        .try_exists()
                        .map_err(|e| format!("config {}: {e}", path.display()))? =>
            {
                path.clone()
            }
            _ => self.paths.asset("config.json"),
        };
        eprintln!(
            "level=info event=discovery_source path={:?}",
            path.to_string_lossy()
        );
        if !std::fs::metadata(&path)
            .map_err(|e| format!("config {}: {e}", path.display()))?
            .is_file()
        {
            return Err("app config must be a regular file".into());
        }
        let file =
            std::fs::File::open(&path).map_err(|e| format!("config {}: {e}", path.display()))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("app config must be a regular file".into());
        }
        let mut text = String::new();
        file.take(1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|e| format!("config {}: {e}", path.display()))?;
        if text.len() > 1024 * 1024 {
            return Err("app config exceeds 1 MiB".into());
        }
        parse(&text, self.paths).map_err(|e| format!("config {}: {e}", path.display()))
    }
}

fn parse(text: &str, paths: &Paths) -> Result<Catalog, String> {
    let root: Value = serde_json::from_str(&without_trailing_commas(text))
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let pages = root["pages"].as_array().ok_or("missing pages array")?;
    let mut catalog = Catalog {
        preferences: crate::preferences::Preferences::parse(&root, paths),
        ..Catalog::default()
    };
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
            match entry(item, paths) {
                Ok(mut app) => {
                    // Marshmallow has no IDs and permits duplicate commands. Keep all
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
        match entry(&item, paths) {
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

fn entry(item: &Value, paths: &Paths) -> Result<AppEntry, String> {
    let name = item["name"].as_str().ok_or("name must be a string")?;
    let shell = item["shell"].as_str().ok_or("shell must be a string")?;
    let icon = item["icon"].as_str().ok_or("icon must be a string")?;
    let (program, mut args) = tokens(shell)?;
    let mut env = BTreeMap::new();
    if let Some(value) = item.get("env") {
        for (key, value) in value.as_object().ok_or("env must be an object")? {
            env.insert(
                key.into(),
                value
                    .as_str()
                    .ok_or("environment values must be strings")?
                    .into(),
            );
        }
    }
    if let Some(value) = item.get("args") {
        for arg in value.as_array().ok_or("args must be an array")? {
            args.push(arg.as_str().ok_or("arguments must be strings")?.into());
        }
    }
    let cwd = item
        .get("cwd")
        .map(|v| v.as_str().ok_or("cwd must be a string").map(PathBuf::from))
        .transpose()?;
    let mut manifest = AppManifest {
        entry: paths.cwd.join(&program),
        args,
        cwd,
        env,
        runtime: None,
    };
    // Validate untrusted fields before searching for executables, or launching.
    let mut app = AppEntry {
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
    let cwd = manifest.cwd.as_deref().unwrap_or(&paths.cwd);
    let search: Vec<_> = manifest.env.get(std::ffi::OsStr::new("PATH")).map_or_else(
        || paths.search_path.clone(),
        |p| std::env::split_paths(p).map(|p| cwd.join(p)).collect(),
    );
    let resolved = resolve(&program, cwd, &search);
    if let Some(path) = resolved {
        manifest.entry = path;
    } else {
        app.unavailable = Some(format!("command not found or not executable: {program}"));
    }
    if !cwd.is_dir() {
        app.unavailable = Some(format!("working directory unavailable: {}", cwd.display()));
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

fn resolve(program: &str, cwd: &Path, search: &[PathBuf]) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(cwd.join(program)).filter(|p| executable(p));
    }
    search
        .iter()
        .map(|p| p.join(program))
        .find(|p| executable(p))
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

// JUCE fromTokens(command, true) groups using double quotes, retains quotes in
// arguments, treats single quotes/backslashes literally, and skips empty tokens.
// Only the executable is unquoted by JUCE's ActiveProcess. Never shell-expand.
fn tokens(command: &str) -> Result<(String, Vec<std::ffi::OsString>), String> {
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
            user_config: None,
            asset_roots: vec!["/missing-assets".into()],
            native_apps: None,
            cwd: "/".into(),
            search_path: vec!["/bin".into(), "/usr/bin".into()],
        }
    }
    #[test]
    fn ordered_apps_pages_only_skip_invalid_keep_missing_and_duplicates() -> Result<(), String> {
        let text = r#"{"pages":[{"name":"Settings","items":[{"name":"No","shell":"sh","icon":""}]},{"name":"Apps","items":[
        {"name":"Zulu","shell":"sh","icon":""},
        {"name":"Invalid","shell":3,"icon":""},
        {"name":"Alpha","shell":"/no/such/vitrallis-command","icon":"lost.png"},
        {"name":"Zulu","shell":"sh","icon":""},
        ]},{"name":"Apps","items":[{"name":"Last","shell":"sh","icon":""}]}]}"#;
        let result = parse(text, &paths())?;
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
        let again = parse(text, &paths())?;
        assert_eq!(again.apps[0].id, result.apps[0].id);
        assert_eq!(stable_id("Zulu", "sh"), result.apps[0].id);
        Ok(())
    }
    #[test]
    fn juce_commands_preserve_quotes_and_do_not_expand_shell_syntax() -> Result<(), String> {
        let (program, args) = tokens(r#""/bin/echo" "two words" 'single words' $HOME a\ b ;"#)?;
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
        assert!(tokens("").is_err());
        assert!(tokens("sh \"unfinished").is_err());
        assert!(tokens("sh\0bad").is_err());
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
            assert!(parse(text, &paths()).is_err());
        }
    }
    #[test]
    fn extensions_are_validated_and_preserved() -> Result<(), String> {
        let item = serde_json::json!({"name":"Tool", "shell":"sh", "icon":"", "cwd":"/tmp", "args":["two words", ""], "env":{"VITRALLIS_TEST":"yes"}});
        let app = entry(&item, &paths())?;
        assert_eq!(app.manifest.cwd, Some("/tmp".into()));
        assert_eq!(app.manifest.args, ["two words", ""]);
        assert_eq!(
            app.manifest.env.get(std::ffi::OsStr::new("VITRALLIS_TEST")),
            Some(&"yes".into())
        );
        for (key, value) in [
            ("env", serde_json::json!({"BAD=KEY":"value"})),
            ("cwd", serde_json::json!("relative")),
            ("args", serde_json::json!([42])),
            ("name", serde_json::json!("\n")),
        ] {
            let mut invalid = item.clone();
            invalid[key] = value;
            assert!(entry(&invalid, &paths()).is_err());
        }
        Ok(())
    }

    #[test]
    fn user_catalog_overrides_defaults_without_merging_or_mutation()
    -> Result<(), Box<dyn std::error::Error>> {
        let scratch = crate::test_support::Scratch::new()?;
        let root = &scratch.0;
        let mut paths = paths();
        paths.user_config = Some(root.join("user.json"));
        paths.asset_roots = vec![root.clone()];
        let default =
            br#"{"pages":[{"name":"Apps","items":[{"name":"Default","shell":"sh","icon":""}]}]}"#;
        std::fs::write(root.join("config.json"), default)?;
        let backend = Marshmallow {
            paths: &paths,
            explicit_config: false,
        };
        assert_eq!(backend.discover()?.apps[0].name, "Default");
        assert!(!root.join("user.json").exists());
        let user = br#"{"pages":[{"name":"Apps","items":[]}]}"#;
        std::fs::write(root.join("user.json"), user)?;
        assert!(backend.discover()?.apps.is_empty());
        assert_eq!(std::fs::read(root.join("user.json"))?, user);
        assert_eq!(std::fs::read(root.join("config.json"))?, default);
        std::fs::write(root.join("user.json"), "broken")?;
        assert!(backend.discover().is_err()); // Never silently replace broken user config.
        std::fs::remove_file(root.join("user.json"))?;
        assert!(
            Marshmallow {
                paths: &paths,
                explicit_config: true
            }
            .discover()
            .is_err()
        );
        Ok(())
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
            PathBuf::from("/absolute/icon.png")
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
