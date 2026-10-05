//! Detect runtimes and provision app-local Python dependencies without importing app code.
use super::metadata::Files;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone)]
pub struct Runtime {
    pub program: PathBuf,
}
pub fn detect(root: &Path, files: &Files) -> Result<Runtime, String> {
    find(root, files, true)
}
fn find(root: &Path, files: &Files, check_dependencies: bool) -> Result<Runtime, String> {
    let candidates = candidates(root, files);
    let requirements = files
        .get("requirements.txt")
        .filter(|_| check_dependencies)
        .map_or(Ok(""), |b| std::str::from_utf8(b))
        .map_err(|e| e.to_string())?;
    // Probe installed distribution metadata without importing app code.
    let deps: Vec<_> = requirements
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect();
    if deps.len() > 256 || requirements.len() > 65536 {
        return Err("Dependency declaration exceeds bounds".into());
    }
    for program in candidates {
        let result = probe(&program, REQUIREMENTS, needs_tk(files), &deps);
        if result.is_ok() {
            return Ok(Runtime { program });
        }
    }
    Err(format!(
        "Missing compatible Python 3{} or dependencies [{}]",
        if needs_tk(files) { "/Tk" } else { "" },
        deps.join(", ")
    ))
}
const REQUIREMENTS: &str = include_str!("python_requirements.py");

fn needs_tk(files: &Files) -> bool {
    files
        .iter()
        .filter(|(name, _)| Path::new(name).extension() == Some(std::ffi::OsStr::new("py")))
        .any(|(_, b)| {
            std::str::from_utf8(b)
                .is_ok_and(|s| s.contains("import tkinter") || s.contains("from tkinter"))
        })
}

fn managed(root: &Path, files: &Files) -> PathBuf {
    root.join("runtime").join(super::storage::sha(
        files.get("requirements.txt").map_or(&[], Vec::as_slice),
    ))
}
pub(super) fn candidates(root: &Path, files: &Files) -> Vec<PathBuf> {
    [
        managed(root, files).join("bin/python3"),
        root.join(".venv/bin/python3"),
        PathBuf::from("/usr/bin/python3"),
        PathBuf::from("/usr/local/bin/python3"),
        PathBuf::from("/opt/homebrew/bin/python3"),
    ]
    .into_iter()
    .filter(|program| program.is_file())
    .collect()
}

const PROVISION: &str = r"import itertools, os, pathlib, stat, subprocess, sys, tempfile, venv
root = pathlib.Path(sys.argv[2])
requirements = sys.argv[3]
validate = sys.argv[4]
# pip --isolated still reads global configuration. Disable all configuration
# files as well, and do not let inherited pip settings redirect writes.
environment = {k: v for k, v in os.environ.items() if not k.startswith(('PIP_', 'PYTHON'))}
environment.update(PIP_CONFIG_FILE=os.devnull, PYTHONNOUSERSITE='1')
os.environ.clear()
os.environ.update(environment)
os.umask(0o077)
# Reject pip options, paths and URLs before creating an environment.
import re
lines = [line.strip() for line in requirements.splitlines() if line.strip() and not line.lstrip().startswith('#')]
if len(lines) > 256 or len(requirements.encode()) > 65536: raise RuntimeError('Dependency declaration exceeds bounds')
for line in lines:
 if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*(?:\s*[<>=!~].*)?(?:\s*;.*)?', line) or any(c in line for c in '@/\\'):
  raise RuntimeError('Unsupported dependency declaration: '+line)
with tempfile.TemporaryDirectory(prefix='.pending-', dir=str(root.parent)) as staging:
 # System distributions are already trusted by runtime detection. Local site
 # packages take precedence; -I excludes user site and environment overrides.
 venv.EnvBuilder(with_pip=True, symlinks=False, system_site_packages=True).create(staging)
 python = str(pathlib.Path(staging) / 'bin/python3')
 subprocess.run([python, '-I', '-c', validate, 'validate'] + lines, check=True, timeout=30, env=environment)
 subprocess.run([python, '-I', '-m', 'pip', '--isolated', 'install', '--disable-pip-version-check', '--no-input', 'packaging'] + lines, check=True, timeout=600, env=environment)
 # venv adds this fixed alias on 64-bit Linux even with symlinks=False.
 # The interpreter uses lib/ directly; managed storage forbids symlinks.
 lib64 = pathlib.Path(staging) / 'lib64'
 if lib64.is_symlink() and os.readlink(lib64) == 'lib': lib64.unlink()
 # copy2 preserves modes from the base interpreter's activation templates;
 # umask alone cannot prevent those templates from retaining group write.
 for path in itertools.chain([pathlib.Path(staging)], pathlib.Path(staging).rglob('*')):
  info = path.lstat()
  if not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)) or (stat.S_ISREG(info.st_mode) and info.st_nlink != 1):
   raise RuntimeError('Unsafe generated runtime entry: '+str(path))
  path.chmod(stat.S_IMODE(info.st_mode) & ~0o022)
 # Never publish an environment that pip claims succeeded but cannot satisfy
 # the same metadata/Tk checks used when selecting it for launch.
 subprocess.run([python, '-I', '-c', validate, sys.argv[1]] + lines, check=True, timeout=30, env=environment)
 os.rename(staging, root)
 pathlib.Path(staging).mkdir()
";

pub fn ensure(root: &Path, files: &Files) -> Result<Runtime, String> {
    if let Ok(runtime) = detect(root, files) {
        return Ok(runtime);
    }
    let base = find(root, files, false)?;
    validate(&base, files)?;
    let target = managed(root, files);
    // Uninstall journals the generated files and leaves their directories intact
    // for recovery. Only an entirely empty, safe hierarchy may be discarded.
    if target.symlink_metadata().is_ok()
        && !super::runtime_cleanup::remove_empty(&target)
            .map_err(|e| format!("App dependency environment is damaged: {e}"))?
    {
        return Err("App dependency environment is damaged; uninstall it before retrying".into());
    }
    super::storage::safe(&target)?;
    super::storage::directory(target.parent().ok_or("Missing runtime parent")?)?;
    let requirements = files
        .get("requirements.txt")
        .map_or(Ok(""), |b| std::str::from_utf8(b))
        .map_err(|e| e.to_string())?;
    run_probe(
        &base.program,
        PROVISION,
        needs_tk(files),
        &[
            target.to_str().ok_or("Runtime path must be UTF-8")?.into(),
            requirements.into(),
            REQUIREMENTS.into(),
        ],
        std::time::Duration::from_mins(12),
    )
    .map_err(|e| format!("{e}\nApp dependency installation failed."))?;
    detect(root, files)
}

fn probe(program: &Path, script: &str, tk: bool, deps: &[String]) -> Result<(), String> {
    run_probe(
        program,
        script,
        tk,
        deps,
        std::time::Duration::from_secs(30),
    )
}
fn run_probe(
    program: &Path,
    script: &str,
    tk: bool,
    deps: &[String],
    timeout: std::time::Duration,
) -> Result<(), String> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or("Runtime/dependency deadline overflow")?;
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let _group_options = command.process_group(0);
    }
    let _probe_options = command
        .args(["-I", "-c", script, if tk { "tk" } else { "plain" }])
        .args(deps)
        .current_dir("/")
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    let _python_environment = command.env_remove("PYTHONPATH");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let outcome = (|| {
        let stdout = child.stdout.take().ok_or("Missing runtime stdout")?;
        let stderr = child.stderr.take().ok_or("Missing runtime stderr")?;
        let outputs = [drain(stdout), drain(stderr)];
        loop {
            #[cfg(unix)]
            let exited = vitrallis_native::process::exited_unreaped(&mut child)
                .map_err(|error| format!("Runtime exit probe failed: {error}"))?;
            #[cfg(not(unix))]
            let exited = child
                .try_wait()
                .map_err(|error| format!("Runtime wait failed: {error}"))?
                .is_some();
            if exited {
                break;
            }
            if Instant::now() >= deadline {
                return Err("Runtime/dependency process timed out".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // EOF covers inherited output pipes as well as the direct child. The
        // provisioning helper itself waits for venv and every pip subprocess.
        let mut diagnostics = String::new();
        for output in outputs {
            let bytes = output?
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|error| {
                    format!("Runtime/dependency output did not finish before deadline: {error}")
                })??;
            diagnostics.push_str(&String::from_utf8_lossy(&bytes));
        }
        Ok(diagnostics)
    })();
    // Keep the leader waitable until output completion and group termination.
    // A reaped child's numeric PID must never authorize descendant cleanup.
    crate::process::cleanup_group(&mut child);
    let diagnostics = match outcome {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            crate::process::cleanup_child(&mut child);
            return Err(error);
        }
    };
    let status = child
        .wait()
        .map_err(|error| format!("Runtime wait failed: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}\nRuntime/dependency process exited {status}: {}",
            failure_reason(&diagnostics),
            diagnostics.trim()
        ))
    }
}

fn failure_reason(diagnostics: &str) -> &str {
    // Pip often prints a generic build summary after the useful compiler error.
    diagnostics
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("error: ") && !line.contains("subprocess-exited-with-error"))
        .or_else(|| {
            diagnostics.lines().rev().map(str::trim).find(|line| {
                line.starts_with("ERROR: ")
                    || line.starts_with("RuntimeError: ")
                    || line.contains("No module named")
            })
        })
        .unwrap_or("Python venv/pip and network access are required; see details below")
}

fn drain(
    mut output: impl std::io::Read + Send + 'static,
) -> Result<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>, String> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // The receiver observes completion or a worker panic as disconnection.
    // On cancellation the pipe closes when the owned helper group is stopped.
    let _worker = std::thread::Builder::new()
        .name("app-runtime-output".into())
        .spawn(move || {
            let mut tail = Vec::new();
            let mut buffer = [0; 4096];
            let result = (|| {
                loop {
                    let count = match output.read(&mut buffer) {
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        result => result.map_err(|error| error.to_string())?,
                    };
                    if count == 0 {
                        return Ok(tail);
                    }
                    tail.extend_from_slice(
                        buffer.get(..count).ok_or("Invalid runtime output length")?,
                    );
                    if tail.len() > 8192 {
                        drop(tail.drain(..tail.len().saturating_sub(8192)));
                    }
                }
            })();
            if send.send(result).is_err() {
                eprintln!("level=debug event=runtime_output_cancelled");
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(receive)
}

pub fn launcher(runtime: &Runtime, entry: &Path) -> Result<Vec<u8>, String> {
    fn quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
    use std::fmt::Write;
    let mut s = String::from("#!/bin/sh\n");
    // Reuse installed library bytecode without creating caches in app payloads.
    // Managed module caches are invalidated before replacement by install.rs;
    // always checking hashes also rejects stale unchecked-hash library caches.
    s.push_str("unset PYTHONHOME PYTHONPATH PYTHONSTARTUP PYTHONPYCACHEPREFIX\nexport PYTHONNOUSERSITE=1\nexport PYTHONDONTWRITEBYTECODE=1\n");
    let program = runtime
        .program
        .to_str()
        .ok_or("Runtime path must be UTF-8")?;
    let entry_name = entry.to_str().ok_or("Entry path must be UTF-8")?;
    writeln!(
        s,
        "exec {} --check-hash-based-pycs always {}",
        quote(program),
        quote(entry_name)
    )
    .map_err(|error| error.to_string())?;
    Ok(s.into_bytes())
}

/// Give desktop shortcuts and Shell launches the same persistent storage contract.
pub fn environment(bytes: &[u8], home: &Path, id: &str) -> Result<Vec<u8>, String> {
    use std::fmt::Write;
    super::metadata::identity(id)?;
    let payload = vitrallis_native::paths::app_dir(home, id).map_err(|e| e.to_string())?;
    let data = vitrallis_native::paths::app_data(home, id).map_err(|e| e.to_string())?;
    let documents = vitrallis_native::paths::documents(home, id).map_err(|e| e.to_string())?;
    let script = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let body = script
        .strip_prefix("#!/bin/sh\n")
        .ok_or("Invalid app launcher")?;
    let mut output = String::from("#!/bin/sh\numask 077\n");
    for (key, value) in [
        ("VITRALLIS_APP_ID", Path::new(id)),
        ("VITRALLIS_APP_DIR", payload.as_path()),
        ("VITRALLIS_APP_DATA_DIR", data.as_path()),
        ("VITRALLIS_DOCUMENTS_DIR", documents.as_path()),
    ] {
        let path_text = value.to_str().ok_or("App path must be UTF-8")?;
        writeln!(
            output,
            "export {key}='{}'",
            path_text.replace('\'', "'\\''")
        )
        .map_err(|error| error.to_string())?;
    }
    output.push_str("if [ -e \"$VITRALLIS_APP_DIR/.installation-pending\" ] || [ -L \"$VITRALLIS_APP_DIR/.installation-pending\" ]; then printf '%s\\n' 'Installation incomplete; repair in App Center.' >&2; exit 1; fi\n");
    output.push_str("cd \"$VITRALLIS_APP_DATA_DIR\" || { printf '%s\\n' 'App data unavailable; repair in App Center.' >&2; exit 1; }\n");
    output.push_str(body);
    Ok(output.into_bytes())
}

/// Compile Python syntax in the selected app runtime without importing or executing it.
pub fn validate(runtime: &Runtime, files: &Files) -> Result<(), String> {
    let sources = files
        .iter()
        .filter(|(name, _)| Path::new(name).extension() == Some(std::ffi::OsStr::new("py")))
        .map(|(name, bytes)| {
            let source = std::str::from_utf8(bytes)
                .map_err(|e| format!("Python UTF-8 source {name}: {e}"))?;
            Ok((name, source))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let input = serde_json::to_vec(&sources).map_err(|e| e.to_string())?;
    let script = "import json,sys\nfor name,source in json.load(sys.stdin).items():\n compile(source,name,'exec',dont_inherit=True)\n";
    compile_sources(runtime, input, script)
}
fn compile_sources(runtime: &Runtime, input: Vec<u8>, script: &str) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(30))
        .ok_or("Python syntax deadline overflow")?;
    let mut command = Command::new(&runtime.program);
    let _syntax_options = command
        .args(["-s", "-c", script])
        .current_dir("/")
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    let _syntax_environment = command.env_remove("PYTHONPATH");
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let Some(mut stdin) = child.stdin.take() else {
        crate::process::cleanup_child(&mut child);
        return Err("Syntax preflight input unavailable".into());
    };
    let spawned_writer = std::thread::Builder::new()
        .name("app-syntax-input".into())
        .spawn(move || stdin.write_all(&input));
    let writer = match spawned_writer {
        Ok(w) => w,
        Err(e) => {
            crate::process::cleanup_child(&mut child);
            return Err(e.to_string());
        }
    };
    let result: Result<(), String> = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("Package Python syntax is incompatible with the selected runtime".into())
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                crate::process::cleanup_child(&mut child);
                break Err("Python syntax preflight timed out".into());
            }
        }
    };
    let written = writer
        .join()
        .map_err(|payload| format!("Syntax input worker failed: {payload:?}"))?
        .map_err(|e| e.to_string());
    result?;
    written
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use std::time::{Duration, Instant};

    struct CachedLibrary {
        _scratch: crate::test_support::Scratch,
        entry: PathBuf,
        module: PathBuf,
        bytecode: PathBuf,
        default_prefix: Option<String>,
        external_cache: Option<PathBuf>,
    }

    impl Drop for CachedLibrary {
        fn drop(&mut self) {
            // Apple Python's default cache lives outside the source tree. Remove
            // only the subtree containing this unique scratch-directory name.
            if let Some(cache) = &self.external_cache
                && let Err(error) = std::fs::remove_dir_all(cache)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!(
                    "Cannot remove Python test cache {}: {error}",
                    cache.display()
                );
            }
        }
    }

    fn cached_library(checked: bool, prefixed: bool) -> Result<CachedLibrary, String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let library = root.join("library");
        let app = root.join("app");
        std::fs::create_dir(&library).map_err(|e| e.to_string())?;
        std::fs::create_dir(&app).map_err(|e| e.to_string())?;
        let module = library.join("trusted_lib.py");
        std::fs::write(&module, "VALUE = 'old'\n").map_err(|e| e.to_string())?;
        let compiled = std::process::Command::new("/usr/bin/python3")
            .args([
                "-s",
                "-c",
                "import json,py_compile,sys\ndefault=sys.pycache_prefix\nif sys.argv[2]: sys.pycache_prefix=sys.argv[2]\nbytecode=py_compile.compile(sys.argv[1],doraise=True,invalidation_mode=py_compile.PycInvalidationMode[sys.argv[3]])\nprint(json.dumps({'bytecode':bytecode,'default_prefix':default}))",
            ])
            .arg(&module)
            .arg(if prefixed { root.join("poison") } else { PathBuf::new() })
            .arg(if checked { "CHECKED_HASH" } else { "UNCHECKED_HASH" })
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONSTARTUP")
            .env_remove("PYTHONPYCACHEPREFIX")
            .env("PYTHONNOUSERSITE", "1")
            .output()
            .map_err(|e| e.to_string())?;
        if !compiled.status.success() {
            return Err(format!("Compile library fixture: {compiled:?}"));
        }
        let compiled_metadata: serde_json::Value =
            serde_json::from_slice(&compiled.stdout).map_err(|e| e.to_string())?;
        let bytecode = PathBuf::from(
            (*compiled_metadata
                .get("bytecode")
                .ok_or("Missing fixture element")?)
            .as_str()
            .ok_or("Missing fixture bytecode path")?,
        );
        let default_prefix = match compiled_metadata
            .get("default_prefix")
            .ok_or("Missing fixture element")?
        {
            serde_json::Value::Null => None,
            serde_json::Value::String(prefix) => Some(prefix.clone()),
            serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_) => return Err("Invalid interpreter cache prefix".into()),
        };
        let external_cache = if bytecode.starts_with(&root) {
            None
        } else {
            Some(
                bytecode
                    .ancestors()
                    .find(|path| path.file_name() == root.file_name())
                    .ok_or("Library cache lacks its scratch namespace")?
                    .to_path_buf(),
            )
        };
        let entry = app.join("main.py");
        std::fs::write(
            &entry,
            "import sys\nsys.dont_write_bytecode=True\nfrom pathlib import Path\nlibrary=Path(__file__).parent.parent/'library'\nsource=library/'trusted_lib.py'\ncompiles=[]\ndef audit(event,args):\n if event=='compile' and args[1]==str(source): compiles.append(True)\nsys.addaudithook(audit)\nsys.path.insert(0,str(library))\nimport trusted_lib,json\nprint(json.dumps({'value':trusted_lib.VALUE,'compiles':len(compiles),'prefix':sys.pycache_prefix,'writes_disabled':sys.dont_write_bytecode}))\n",
        )
        .map_err(|e| e.to_string())?;
        Ok(CachedLibrary {
            _scratch: scratch,
            entry,
            module,
            bytecode,
            default_prefix,
            external_cache,
        })
    }

    fn launch_cached_library(fixture: &CachedLibrary) -> Result<serde_json::Value, String> {
        let script = fixture.entry.with_file_name("launch");
        std::fs::write(
            &script,
            launcher(
                &Runtime {
                    program: "/usr/bin/python3".into(),
                },
                &fixture.entry,
            )?,
        )
        .map_err(|e| e.to_string())?;
        let output = std::process::Command::new("/bin/sh")
            .arg(script)
            .env(
                "PYTHONPYCACHEPREFIX",
                fixture
                    .module
                    .parent()
                    .ok_or("Library parent")?
                    .with_file_name("poison"),
            )
            .current_dir("/")
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!("Launch library fixture: {output:?}"));
        }
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
    }

    #[test]
    fn launcher_reuses_valid_library_bytecode_without_writes() -> Result<(), String> {
        let fixture = cached_library(true, false)?;
        let before = std::fs::read(&fixture.bytecode).map_err(|e| e.to_string())?;
        for _ in 0_i32..2_i32 {
            let result = launch_cached_library(&fixture)?;
            assert_eq!(
                (*result.get("value").ok_or("Missing fixture element")?),
                "old"
            );
            assert_eq!(
                (*result.get("compiles").ok_or("Missing fixture element")?),
                0_i32
            );
            assert_eq!(
                (*result.get("prefix").ok_or("Missing fixture element")?),
                serde_json::json!(fixture.default_prefix)
            );
            assert_eq!(
                (*result
                    .get("writes_disabled")
                    .ok_or("Missing fixture element")?),
                true
            );
            assert_eq!(
                std::fs::read(&fixture.bytecode).map_err(|e| e.to_string())?,
                before
            );
        }
        Ok(())
    }

    #[test]
    fn launcher_rejects_stale_unchecked_hashes_and_inherited_cache_prefix() -> Result<(), String> {
        for prefixed in [false, true] {
            let fixture = cached_library(false, prefixed)?;
            let before = std::fs::read(&fixture.bytecode).map_err(|e| e.to_string())?;
            std::fs::write(&fixture.module, "VALUE = 'new'\n").map_err(|e| e.to_string())?;
            let result = launch_cached_library(&fixture)?;
            assert_eq!(
                (*result.get("value").ok_or("Missing fixture element")?),
                "new"
            );
            assert_eq!(
                (*result.get("compiles").ok_or("Missing fixture element")?),
                1_i32
            );
            assert_eq!(
                (*result.get("prefix").ok_or("Missing fixture element")?),
                serde_json::json!(fixture.default_prefix)
            );
            assert_eq!(
                (*result
                    .get("writes_disabled")
                    .ok_or("Missing fixture element")?),
                true
            );
            assert_eq!(
                std::fs::read(&fixture.bytecode).map_err(|e| e.to_string())?,
                before
            );
        }
        Ok(())
    }

    #[test]
    fn launcher_environment_quotes_paths_and_does_not_depend_on_callers_directory()
    -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let home = scratch
            .0
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join("owner's $files (space)");
        let id = "io.test.environment";
        let data = vitrallis_native::paths::app_data(&home, id).map_err(|e| e.to_string())?;
        super::super::storage::private_directory(&data, &home)?;
        let bytes = environment(b"#!/bin/sh\nprintf '%s\\n' \"$VITRALLIS_APP_ID\" \"$VITRALLIS_APP_DATA_DIR\" \"$PWD\"\n", &home, id)?;
        let script = home.join("launch");
        std::fs::write(&script, bytes).map_err(|e| e.to_string())?;
        let output = std::process::Command::new("/bin/sh")
            .arg(script)
            .current_dir("/")
            .output()
            .map_err(|e| e.to_string())?;
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).map_err(|e| e.to_string())?,
            format!("{id}\n{}\n{}\n", data.display(), data.display())
        );
        Ok(())
    }
    #[test]
    fn dependency_chain_waits_for_real_completion_and_preserves_failure_output()
    -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let marker = scratch.0.join("dependency-ready");
        let script = "import subprocess,sys; subprocess.run([sys.executable, '-c', 'import pathlib,sys,time; time.sleep(0.15); pathlib.Path(sys.argv[1]).write_text(\"installed\")', sys.argv[2]], check=True)";
        for _ in 0_i32..3_i32 {
            run_probe(
                Path::new("/usr/bin/python3"),
                script,
                false,
                &[marker.to_string_lossy().into_owned()],
                Duration::from_secs(10),
            )?;
            assert_eq!(
                std::fs::read_to_string(&marker).map_err(|e| e.to_string())?,
                "installed"
            );
            std::fs::remove_file(&marker).map_err(|e| e.to_string())?;
        }
        let probe = run_probe(
            Path::new("/usr/bin/python3"),
            "import sys; print('x'*100000); print('pip dependency failed: wheel unavailable',file=sys.stderr); sys.exit(7)",
            false,
            &[],
            Duration::from_secs(10),
        );
        let error = probe
            .err()
            .ok_or("the failing installer probe must not succeed")?;
        assert!(error.contains("wheel unavailable") && error.contains('7'));
        assert!(error.len() < 17500);
        let compiler = "Collecting Pillow\nerror: [Errno 2] No such file or directory: arm-linux-gnueabihf-gcc\nERROR: Failed building wheel for Pillow";
        assert!(failure_reason(compiler).contains("arm-linux-gnueabihf-gcc"));
        Ok(())
    }
    #[test]
    fn descendants_cannot_outlive_the_completion_deadline() {
        let start = Instant::now();
        let result = run_probe(
            Path::new("/usr/bin/python3"),
            "import subprocess,sys; subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(10)'])",
            false,
            &[],
            Duration::from_millis(200),
        );
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[cfg(unix)]
    #[test]
    fn damaged_group_writable_environment_reports_recovery() -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
        let mut files = Files::new();
        drop(files.insert(
            "requirements.txt".into(),
            b"vitrallis-absent-dependency>=1\n".to_vec(),
        ));
        let target = managed(&root, &files);
        std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        // Python's venv creates group-writable directories under umask 002.
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o775))
            .map_err(|e| e.to_string())?;
        let error = ensure(&root, &files)
            .err()
            .ok_or("damaged environment was accepted")?;
        assert!(error.contains("damaged"), "{error}");
        Ok(())
    }
}
