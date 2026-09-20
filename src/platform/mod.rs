pub mod generic;
pub mod linux_handheld;

pub mod command;
pub mod system;
pub mod update;

/// Session policy and an independently refreshed system backend.
pub trait Platform: system::System + Copy {
    fn fullscreen(&self) -> bool;
    /// Preferred display dimensions; `--size` can override this default without
    /// selecting a different system backend or changing input support.
    fn resolution(&self) -> (u16, u16);
    fn prepare_app(&self, _app: &mut crate::app::AppEntry) {}
    fn raise_after_exit(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AppWindow {
    Calibration,
}

impl AppWindow {
    pub fn for_entry(entry: &std::path::Path) -> Option<Self> {
        if entry == std::path::Path::new(crate::platform::linux_handheld::CALIBRATION) {
            return Some(Self::Calibration);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusResult {
    Focused,
    Missing,
}

/// Repair X input focus after a Tk window closes. Awesome can already consider
/// the shell focused even when X has reverted to no input window.
pub fn restore_shell_focus() {
    if std::env::var_os("VITRALLIS_SESSION").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let pid = std::process::id();
    let result = std::thread::Builder::new()
        .name("shell-focus".into())
        .spawn(move || {
            // Do not steal focus if the user has already switched to another app.
            let code = format!(
                "for _,c in ipairs(client.get()) do if c.pid=={pid} then \
                 if not client.focus or client.focus==c then \
                 client.focus=nil; client.focus=c; c:raise() end; return end end"
            );
            if let Err(error) = command::run("/usr/bin/awesome-client", &[&code]) {
                eprintln!("level=warning event=shell_focus_failed message={error:?}");
            }
        });
    if let Err(error) = result {
        eprintln!("level=warning event=shell_focus_worker_failed message={error:?}");
    }
}

pub fn focus_application(pid: u32, hint: Option<AppWindow>) -> Result<FocusResult, String> {
    // The inspected calibrator uses an override-redirect surface and grabs its
    // own input. It has no Awesome client to raise or wait for.
    if matches!(hint, Some(AppWindow::Calibration)) {
        return Ok(FocusResult::Focused);
    }
    if std::env::var_os("VITRALLIS_SESSION").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return Err("use the window manager to return to the running app".into());
    }
    application_window(pid, true)
}

pub fn application_window(pid: u32, focus: bool) -> Result<FocusResult, String> {
    let shell_pid = std::process::id();
    let activation = if focus {
        "if client.focus and client.focus~=c and client.focus.pid~=shell_pid then return 'no matching window' end; client.focus=c; c:raise();"
    } else {
        ""
    };
    // The only interpolated value is a process ID obtained from Child::id.
    // A child window may belong to a descendant; match our private process group.
    let code = format!(
        "local shell_pid={shell_pid}; for _,c in ipairs(client.get()) do \
         local p=tonumber(c.pid); if p and p>0 and p%1==0 then \
         local f=io.open('/proc/'..string.format('%.0f',p)..'/stat','r'); \
         if f then local s=f:read('*l'); f:close(); \
         local group=s and s:match('.*%)%s+%S+%s+%d+%s+(%d+)'); \
         if tonumber(group)=={pid} then {activation} return 'focused' end \
         end end end; \
         return 'no matching window'"
    );
    let result = command::run("/usr/bin/awesome-client", &[&code])?;
    if result.contains("\"focused\"") {
        Ok(FocusResult::Focused)
    } else {
        Ok(FocusResult::Missing)
    }
}
