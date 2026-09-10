use crate::{app::App, input::Action, navigation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Launching,
    Running,
}

#[derive(Debug)]
pub struct Launcher {
    pub apps: Vec<App>,
    pub selected: usize,
    pub phase: Phase,
    pub status: String,
    columns: usize,
}
impl Launcher {
    pub fn new(apps: Vec<App>, columns: usize, capacity: usize) -> Result<Self, String> {
        if columns == 0 || apps.len() > capacity {
            return Err("invalid grid capacity".into());
        }
        for (i, app) in apps.iter().enumerate() {
            app.validate()?;
            if apps[..i].iter().any(|a| a.id == app.id) {
                return Err(format!("duplicate app id {}", app.id));
            }
        }
        Ok(Self {
            apps,
            selected: 0,
            phase: Phase::Ready,
            status: "ARROWS: SELECT   ENTER / TAP: OPEN".into(),
            columns,
        })
    }
    /// Returns an app index only once per launch transition.
    pub fn input(&mut self, action: Action) -> Option<usize> {
        if self.phase != Phase::Ready {
            return None;
        }
        match action {
            Action::Move(direction) => {
                self.selected =
                    navigation::moved(self.selected, direction, self.columns, self.apps.len());
            }
            Action::Back => self.status = "ARROWS: SELECT   ENTER / TAP: OPEN".into(),
            Action::SelectAndActivate(index) if index < self.apps.len() => {
                self.selected = index;
                return self.input(Action::Activate);
            }
            Action::Activate if !self.apps.is_empty() => {
                self.phase = Phase::Launching;
                self.status = format!("OPENING {}", self.apps[self.selected].name);
                return Some(self.selected);
            }
            Action::Activate | Action::SelectAndActivate(_) => {}
        }
        None
    }
    pub fn started(&mut self) {
        self.phase = Phase::Running;
        self.status = "APP RUNNING - WAITING FOR EXIT".into();
    }
    pub fn finished(&mut self, message: String) {
        self.phase = Phase::Ready;
        self.status = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_is_guarded_and_recovers() -> Result<(), String> {
        use crate::platform::Platform;
        let backend = crate::platform::generic::Mock;
        let apps = backend.apps()?;
        assert_eq!(backend.resolution(), (480, 272));
        assert!(!backend.fullscreen());
        let mut state = Launcher::new(apps, 3, 6)?;
        assert_eq!(state.input(Action::SelectAndActivate(1)), Some(1));
        assert_eq!(state.phase, Phase::Launching);
        assert_eq!(state.input(Action::Activate), None);
        state.started();
        state.input(Action::Back);
        assert_eq!(state.phase, Phase::Running);
        state.finished("done".into());
        assert_eq!(state.selected, 1);
        assert_eq!(state.input(Action::Activate), Some(1));
        state.finished("failed".into());
        assert_eq!(state.phase, Phase::Ready);
        Ok(())
    }
    #[test]
    fn empty_and_duplicate_catalogs_are_safe() -> Result<(), String> {
        let mut empty = Launcher::new(vec![], 3, 6)?;
        assert_eq!(empty.input(Action::Activate), None);
        assert_eq!(empty.input(Action::SelectAndActivate(0)), None);
        let mut apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        apps[1].id = apps[0].id.clone();
        assert!(Launcher::new(apps, 3, 6).is_err());
        Ok(())
    }
}
