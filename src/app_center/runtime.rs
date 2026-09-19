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
        let result = probe(&program, script, needs_tk, &deps);
        if result.is_ok() {
            return Ok(Runtime { program });
        }
    }
    Err(format!(
        "Missing compatible Python 3{} or dependencies [{}]",
        if needs_tk { "/Tk" } else { "" },
        deps.join(", ")
    ))
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

const PROVISION: &str = r"import os, pathlib, subprocess, sys, tempfile, venv
root = pathlib.Path(sys.argv[2])
requirements = sys.argv[3]
# Reject pip options, paths and URLs before creating an environment.
import re
lines = [line.strip() for line in requirements.splitlines() if line.strip() and not line.lstrip().startswith('#')]
if len(lines) > 256 or len(requirements.encode()) > 65536: raise RuntimeError('Dependency declaration exceeds bounds')
for line in lines:
 if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*(?:\s*[<>=!~].*)?(?:\s*;.*)?', line) or any(c in line for c in '@/\\'):
  raise RuntimeError('Unsupported dependency declaration: '+line)
with tempfile.TemporaryDirectory(prefix='.pending-', dir=str(root.parent)) as staging:
 venv.EnvBuilder(with_pip=True, symlinks=False).create(staging)
 python = str(pathlib.Path(staging) / 'bin/python3')
 validate = 'from pip._vendor.packaging.requirements import Requirement; import sys; [Requirement(line) for line in sys.argv[1:]]'
 subprocess.run([python, '-I', '-c', validate] + lines, check=True, timeout=10)
 subprocess.run([python, '-I', '-m', 'pip', '--isolated', 'install', '--disable-pip-version-check', '--no-input', 'packaging'] + lines, check=True, timeout=600)
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
    super::storage::safe(&target)?;
    if target.exists() {
        return Err("App dependency environment is damaged; remove it before retrying".into());
    }
    super::storage::directory(target.parent().ok_or("Missing runtime parent")?)?;
    let requirements = files
        .get("requirements.txt")
        .map_or(Ok(""), |b| std::str::from_utf8(b))
        .map_err(|e| e.to_string())?;
    run_probe(
        &base.program,
        PROVISION,
        false,
        &[
            target.to_str().ok_or("Runtime path must be UTF-8")?.into(),
            requirements.into(),
        ],
        std::time::Duration::from_secs(660),
    )
    .map_err(|e| {
        format!(
            "Could not install app dependencies (Python venv/pip and network access required): {e}"
        )
    })?;
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
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .args(["-s", "-c", script, if tk { "tk" } else { "plain" }])
        .args(deps)
        .current_dir("/")
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    command.env_remove("PYTHONPATH");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let stdout = child.stdout.take().ok_or("Missing runtime stdout")?;
    let stderr = child.stderr.take().ok_or("Missing runtime stderr")?;
    let outputs = [drain(stdout), drain(stderr)];
    let outcome = (|| {
        let status = loop {
            match child
                .try_wait()
                .map_err(|e| format!("Runtime wait failed: {e}"))?
            {
                Some(status) => break status,
                None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                None => return Err("Runtime/dependency process timed out".into()),
            }
        };
        // EOF covers inherited output pipes as well as the direct child. The
        // provisioning helper itself waits for venv and every pip subprocess.
        let mut diagnostics = String::new();
        for output in outputs {
            let bytes = output?
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "Runtime/dependency output did not finish before deadline")??;
            diagnostics.push_str(&String::from_utf8_lossy(&bytes));
        }
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "Runtime/dependency process exited {status}: {}",
                diagnostics.trim()
            ))
        }
    })();
    if outcome.is_err() {
        crate::process::cleanup_group(child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
    outcome
}

fn drain(
    output: impl std::io::Read + Send + 'static,
) -> Result<std::sync::mpsc::Receiver<Result<Vec<u8>, String>>, String> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("app-runtime-output".into())
        .spawn(move || {
            let mut output = output;
            let mut tail = Vec::new();
            let mut buffer = [0; 4096];
            let result = (|| {
                loop {
                    let count = output.read(&mut buffer).map_err(|e| e.to_string())?;
                    if count == 0 {
                        return Ok(tail);
                    }
                    tail.extend_from_slice(&buffer[..count]);
                    if tail.len() > 8192 {
                        tail.drain(..tail.len() - 8192);
                    }
                }
            })();
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(receive)
}

pub fn launcher(runtime: &Runtime, entry: &Path, commit: &str) -> Result<Vec<u8>, String> {
    fn quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
    use std::fmt::Write;
    super::metadata::hex(commit, 40)?;
    let cache = entry
        .parent()
        .ok_or("Missing entry parent")?
        .join(".vitrallis-bytecode")
        .join(commit);
    let mut s = String::from("#!/bin/sh\n");
    s.push_str("unset PYTHONHOME PYTHONPATH PYTHONSTARTUP\nexport PYTHONNOUSERSITE=1\nexport PYTHONDONTWRITEBYTECODE=1\n");
    let _ = writeln!(
        s,
        "export PYTHONPYCACHEPREFIX={}",
        quote(cache.to_str().ok_or("Cache path must be UTF-8")?)
    );
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
    compile_sources(runtime, input, script)
}
fn compile_sources(runtime: &Runtime, input: Vec<u8>, script: &str) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut command = Command::new(&runtime.program);
    command
        .args(["-s", "-c", script])
        .current_dir("/")
        .env_remove("PYTHONSTARTUP")
        .env_remove("PYTHONHOME");
    command.env_remove("PYTHONPATH");
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
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
    let deadline = Instant::now() + Duration::from_secs(30);
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
    written
}

#[cfg(test)]
mod tests {
    #[test]
    fn dependency_provisioning_is_local_and_cleans_up_failures() -> Result<(), String> {
        let script = r"import pathlib, subprocess, sys, tempfile
from unittest.mock import patch
provision = sys.argv[2]
with tempfile.TemporaryDirectory() as directory:
 root = pathlib.Path(directory) / 'environment'
 def create(path):
  (pathlib.Path(path) / 'ready').touch()
 for requirement, fail in [('Pillow>=10.4,<13', False), ('Pillow>=10.4,<13', True), ('--target=/tmp/unsafe', False), ('Pillow @ https://example.com/a.whl', False)]:
  with patch('venv.EnvBuilder') as builder, patch('subprocess.run') as run:
   builder.return_value.create.side_effect = create
   if fail: run.side_effect = subprocess.CalledProcessError(1, 'pip')
   sys.argv = ['installer', 'plain', str(root), requirement]
   try:
    exec(provision, {})
   except (RuntimeError, subprocess.CalledProcessError):
    assert not root.exists()
    assert not list(pathlib.Path(directory).iterdir())
    if not fail: builder.assert_not_called()
   else:
    assert not fail and requirement == 'Pillow>=10.4,<13'
    assert (root / 'ready').is_file()
    command = run.call_args_list[-1].args[0]
    assert command[0].startswith(directory + '/')
    assert command[1:] == ['-I', '-m', 'pip', '--isolated', 'install', '--disable-pip-version-check', '--no-input', 'packaging', requirement]
    (root / 'ready').unlink()
    root.rmdir()
";
        super::probe(
            std::path::Path::new("/usr/bin/python3"),
            script,
            false,
            &[super::PROVISION.into()],
        )
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn dependency_chain_waits_for_real_completion_and_preserves_failure_output()
    -> Result<(), String> {
        let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
        let marker = scratch.0.join("dependency-ready");
        let script = "import subprocess,sys; subprocess.run([sys.executable, '-c', 'import pathlib,sys,time; time.sleep(0.15); pathlib.Path(sys.argv[1]).write_text(\"installed\")', sys.argv[2]], check=True)";
        for _ in 0..3 {
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
        let error = run_probe(Path::new("/usr/bin/python3"), "import sys; print('x'*100000); print('pip dependency failed: wheel unavailable',file=sys.stderr); sys.exit(7)", false, &[], Duration::from_secs(10)).expect_err("failed installer cannot succeed");
        assert!(error.contains("wheel unavailable") && error.contains('7'));
        assert!(error.len() < 17000);
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
}
