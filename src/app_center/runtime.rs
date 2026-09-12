//! Detect existing runtimes without installing dependencies or importing app code.
use super::{metadata::Files, storage};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone)]
pub struct Runtime {
    pub program: PathBuf,
    pub env: BTreeMap<OsString, OsString>,
}
pub fn detect(home: &Path, root: &Path, files: &Files) -> Result<Runtime, String> {
    let mut candidates = Vec::new();
    for program in [
        root.join(".venv/bin/python3"),
        PathBuf::from("/usr/bin/python3"),
        PathBuf::from("/usr/local/bin/python3"),
        PathBuf::from("/opt/homebrew/bin/python3"),
    ] {
        if program.is_file() {
            candidates.push(program);
        }
    }
    let mut env = BTreeMap::new();
    for base in [
        root.join("runtime/usr"),
        home.join(".local/share/pocket-update-apps/runtime/usr"),
        home.join(".local/share/pocket-bitcoin/runtime/usr"),
    ] {
        if base.is_dir() {
            storage::safe(&base)?;
            for (key, path) in [
                ("LD_LIBRARY_PATH", base.join("lib/arm-linux-gnueabihf")),
                ("TCL_LIBRARY", base.join("share/tcltk/tcl8.6")),
                ("TK_LIBRARY", base.join("share/tcltk/tk8.6")),
            ] {
                env.insert(key.into(), path.into_os_string());
            }
            let paths = [
                base.join("lib/python3.13"),
                base.join("lib/python3.13/lib-dynload"),
            ];
            env.insert(
                "PYTHONPATH".into(),
                std::env::join_paths(paths).map_err(|e| e.to_string())?,
            );
            break;
        }
    }
    let requirements = files
        .get("requirements.txt")
        .map_or(Ok(""), |b| std::str::from_utf8(b))
        .map_err(|e| e.to_string())?;
    // Probe installed distribution metadata; never invoke pip, setup hooks or app imports.
    let deps: Vec<_> = requirements
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect();
    if deps.len() > 256 || requirements.len() > 65536 {
        return Err("Dependency declaration exceeds bounds".into());
    }
    let needs_tk = files
        .iter()
        .filter(|(name, _)| Path::new(name).extension() == Some(std::ffi::OsStr::new("py")))
        .any(|(_, b)| {
            std::str::from_utf8(b)
                .is_ok_and(|s| s.contains("import tkinter") || s.contains("from tkinter"))
        });
    let script = r"import sys
if sys.argv[1] == 'tk': import tkinter
if len(sys.argv) > 2:
 import re, importlib.metadata as m
 for line in sys.argv[2:]:
  if re.fullmatch(r'[A-Za-z0-9_.-]+(?:==[A-Za-z0-9_.+-]+)?', line):
   name, _, pin = line.partition('==')
   found = m.version(name)
   if pin and found != pin: raise RuntimeError('dependency version mismatch: '+name)
  else:
   from packaging.requirements import Requirement
   requirement = Requirement(line)
   if requirement.url: raise RuntimeError('URL dependencies require manual review')
   if requirement.marker is None or requirement.marker.evaluate():
    if requirement.extras: raise RuntimeError('extras require manual dependency review')
    if m.version(requirement.name) not in requirement.specifier: raise RuntimeError('dependency version mismatch')
";
    for program in candidates {
        let result = probe(&program, &env, script, needs_tk, &deps);
        if result.is_ok() {
            return Ok(Runtime { program, env });
        }
    }
    Err(format!(
        "Missing compatible Python 3{} or dependencies [{}]. Use an app-local .venv. Complex requirements need installed packaging; URLs/extras need manual review. No global installs performed",
        if needs_tk { "/Tk" } else { "" },
        deps.join(", ")
    ))
}
fn probe(
    program: &Path,
    env: &BTreeMap<OsString, OsString>,
    script: &str,
    tk: bool,
    deps: &[String],
) -> Result<(), String> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut command = Command::new(program);
    command
        .args(["-s", "-c", script, if tk { "tk" } else { "plain" }])
        .args(deps)
        .envs(env)
        .current_dir("/")
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    if !env.contains_key(&OsString::from("PYTHONPATH")) {
        command.env_remove("PYTHONPATH");
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("Runtime/dependency probe failed".into())
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Runtime probe timed out".into());
            }
        }
    }
}
pub fn launcher(runtime: &Runtime, entry: &Path) -> Result<Vec<u8>, String> {
    fn quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
    use std::fmt::Write;
    let mut s = String::from("#!/bin/sh\n");
    for (key, value) in &runtime.env {
        let _ = writeln!(
            s,
            "export {}={}",
            key.to_string_lossy(),
            quote(&value.to_string_lossy())
        );
    }
    let program = runtime
        .program
        .to_str()
        .ok_or("Runtime path must be UTF-8")?;
    let entry = entry.to_str().ok_or("Entry path must be UTF-8")?;
    let _ = writeln!(s, "exec {} {}", quote(program), quote(entry));
    Ok(s.into_bytes())
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
    inspect(runtime, input, script).map(|_| ())
}
pub fn legacy_version(
    runtime: &Runtime,
    bytes: &[u8],
) -> Result<Option<super::metadata::Version>, String> {
    let script = r"import ast,sys,re
try:
 tree=ast.parse(sys.stdin.buffer.read())
 values=[]
 for node in tree.body:
  if isinstance(node,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='VERSION' for t in node.targets): values.append(node.value)
  if isinstance(node,ast.AnnAssign) and isinstance(node.target,ast.Name) and node.target.id=='VERSION': values.append(node.value)
 value=ast.literal_eval(values[0]) if len(values)==1 else None
 if isinstance(value,str) and len(value)<=32 and re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)',value): print(value)
except (SyntaxError,ValueError,TypeError,MemoryError,RecursionError): pass
";
    let output = inspect(runtime, bytes.to_vec(), script)?;
    let version = std::str::from_utf8(&output)
        .map_err(|e| e.to_string())?
        .trim();
    if version.is_empty() {
        Ok(None)
    } else {
        super::metadata::version(version).map(Some)
    }
}
fn inspect(runtime: &Runtime, input: Vec<u8>, script: &str) -> Result<Vec<u8>, String> {
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut command = Command::new(&runtime.program);
    command
        .args(["-s", "-c", script])
        .current_dir("/")
        .envs(&runtime.env)
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    if !runtime.env.contains_key(&OsString::from("PYTHONPATH")) {
        command.env_remove("PYTHONPATH");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Syntax preflight input unavailable".into());
    };
    let writer = std::thread::Builder::new()
        .name("app-syntax-input".into())
        .spawn(move || stdin.write_all(&input));
    let writer = match writer {
        Ok(w) => w,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.to_string());
        }
    };
    let deadline = Instant::now() + Duration::from_secs(5);
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
                let _ = child.kill();
                let _ = child.wait();
                break Err("Python syntax preflight timed out".into());
            }
        }
    };
    let written = writer
        .join()
        .map_err(|_| "Syntax input worker failed".to_owned())?
        .map_err(|e| e.to_string());
    result?;
    written?;
    let mut output = Vec::new();
    child
        .stdout
        .take()
        .ok_or("Missing inspection output")?
        .take(4097)
        .read_to_end(&mut output)
        .map_err(|e| e.to_string())?;
    if output.len() > 4096 {
        return Err("Runtime inspection output exceeds 4 KiB".into());
    }
    Ok(output)
}
