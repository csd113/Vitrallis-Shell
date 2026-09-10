pub mod generic;
pub mod pocketchip;

mod command;
pub mod system;

/// Session policy and an independently refreshed system backend.
pub trait Platform: system::System + Copy {
    fn fullscreen(&self) -> bool;
    fn resolution(&self) -> (u16, u16);
    fn prepare_app(&self, _app: &mut crate::app::AppEntry) {}
    fn raise_after_exit(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AppWindow {
    Bitcoin,
    Store,
    Calibration,
}

impl AppWindow {
    pub fn for_entry(entry: &std::path::Path) -> Option<Self> {
        if entry == std::path::Path::new("/usr/local/bin/pocketchip-calibration") {
            return Some(Self::Calibration);
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
        if entry == home.join(".local/share/pocket-bitcoin/launch") {
            Some(Self::Bitcoin)
        } else if entry == home.join(".local/share/pocket-update-apps/launch") {
            Some(Self::Store)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusResult {
    Focused,
    Missing,
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
    // The only interpolated value is a process ID obtained from Child::id.
    // A child window may belong to a descendant; match our private process group.
    // Tk does not publish _NET_WM_PID on this image. Only the two inspected
    // upstream wrappers use this closed class/title fallback; it changes focus,
    // never process ownership or termination policy.
    let fallback = match hint {
        Some(AppWindow::Bitcoin) => {
            "c.class=='Tk' and c.name:match('^Bitcoin CAD v%d+%.%d+%.%d+$')"
        }
        Some(AppWindow::Store) => "c.class=='Tk' and c.name=='Update Apps'",
        None | Some(AppWindow::Calibration) => "false",
    };
    let code = format!(
        "for _,c in ipairs(client.get()) do \
         local p=tonumber(c.pid); if p and p>0 and p%1==0 then \
         local f=io.open('/proc/'..string.format('%.0f',p)..'/stat','r'); \
         if f then local s=f:read('*l'); f:close(); \
         local group=s and s:match('.*%)%s+%S+%s+%d+%s+(%d+)'); \
         if tonumber(group)=={pid} then client.focus=c; c:raise(); return 'focused' end \
         end end end; \
         for _,c in ipairs(client.get()) do if {fallback} then \
         client.focus=c; c:raise(); return 'focused' end end; return 'no matching window'"
    );
    let result = command::run("/usr/bin/awesome-client", &[&code])?;
    if result.contains("\"focused\"") {
        Ok(FocusResult::Focused)
    } else {
        Ok(FocusResult::Missing)
    }
}
