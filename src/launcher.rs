use crate::{app::AppEntry, input::Action, navigation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Launching,
    Running,
}

#[derive(Debug)]
pub struct Launcher {
    pub apps: Vec<AppEntry>,
    pub selected: usize,
    pub phase: Phase,
    pub status: String,
    pub error: Option<String>,
    columns: usize,
    capacity: usize,
}
impl Launcher {
    pub fn new(apps: Vec<AppEntry>, columns: usize, capacity: usize) -> Result<Self, String> {
        if columns == 0 || capacity == 0 || capacity % columns != 0 {
            return Err("invalid grid capacity".into());
        }
        for (i, app) in apps.iter().enumerate() {
            app.validate()?;
            if apps[..i].iter().any(|a| a.id == app.id) {
                return Err(format!("duplicate app id {}", app.id));
            }
        }
        Ok(Self {
            selected: 0,
            phase: Phase::Ready,
            error: None,
            status: if apps.is_empty() {
                "NO APPS - CHECK CONFIG / LOG"
            } else {
                "ARROWS: SELECT   ENTER / TAP: OPEN"
            }
            .into(),
            apps,
            columns,
            capacity,
        })
    }
    /// Returns an app index only once per launch transition.
    pub fn input(&mut self, action: Action) -> Option<usize> {
        if self.phase != Phase::Ready {
            return None;
        }
        if self.error.is_some() {
            if matches!(
                action,
                Action::Back | Action::Activate | Action::SelectAndActivate(_)
            ) {
                self.error = None;
                self.status = "ARROWS: SELECT   ENTER / TAP: OPEN".into();
            }
            return None;
        }
        match action {
            Action::Move(direction) => {
                self.selected =
                    navigation::moved(self.selected, direction, self.columns, self.apps.len());
            }
            Action::Page(forward) => {
                let page = self.page_start() / self.capacity;
                let target = if forward {
                    page.saturating_add(1).min(self.page_count() - 1)
                } else {
                    page.saturating_sub(1)
                };
                self.selected = (target * self.capacity + self.selected % self.capacity)
                    .min(self.apps.len().saturating_sub(1));
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
    pub const fn page_start(&self) -> usize {
        self.selected / self.capacity * self.capacity
    }
    pub fn page_count(&self) -> usize {
        self.apps.len().div_ceil(self.capacity).max(1)
    }
    pub fn visible_count(&self) -> usize {
        self.apps
            .len()
            .saturating_sub(self.page_start())
            .min(self.capacity)
    }
    pub fn started(&mut self) {
        self.phase = Phase::Running;
        self.status = "APP RUNNING - WAITING FOR EXIT".into();
    }
    pub fn failed(&mut self, message: String) {
        self.phase = Phase::Ready;
        self.status = "LAUNCH FAILED - ENTER / TAP: DISMISS".into();
        self.error = Some(message);
    }
    pub fn finished(&mut self, message: String) {
        self.error = None;
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
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/mock/vitrallis"));
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

#[cfg(test)]
mod pagination_tests {
    use super::*;
    use crate::{input::PointerInput, layout::Layout, navigation::Direction};
    fn apps(count: usize) -> Vec<AppEntry> {
        let seed =
            crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis")).remove(0);
        (0..count)
            .map(|i| {
                let mut app = seed.clone();
                app.id = format!("app-{i}");
                app
            })
            .collect()
    }
    #[test]
    fn every_page_keyboard_touch_hitbox_and_focus_persistence() -> Result<(), String> {
        use sdl2::{event::Event, mouse::MouseButton};
        for count in [0, 1, 5, 6, 7, 11, 12, 13, 61] {
            for (width, height) in [(480, 272), (800, 480), (1280, 720)] {
                let layout = Layout::home(width, height)?;
                let mut state = Launcher::new(apps(count), 3, 6)?;
                let mut pointer = PointerInput::default();
                assert_eq!(state.page_count(), count.div_ceil(6).max(1));
                for page in 0..state.page_count() {
                    assert_eq!(state.page_start(), page * 6);
                    for local in 0..state.visible_count() {
                        let tile = layout.tiles[local];
                        let down = Event::MouseButtonDown {
                            timestamp: 0,
                            window_id: 1,
                            which: 0,
                            mouse_btn: MouseButton::Left,
                            clicks: 1,
                            x: tile.x + tile.w / 2,
                            y: tile.y + tile.h / 2,
                        };
                        let up = Event::MouseButtonUp {
                            timestamp: 0,
                            window_id: 1,
                            which: 0,
                            mouse_btn: MouseButton::Left,
                            clicks: 1,
                            x: tile.x + tile.w / 2,
                            y: tile.y + tile.h / 2,
                        };
                        assert!(
                            pointer
                                .action(&down, &layout, state.visible_count())
                                .is_none()
                        );
                        let Some(Action::SelectAndActivate(hit)) =
                            pointer.action(&up, &layout, state.visible_count())
                        else {
                            return Err("missing hit".into());
                        };
                        assert_eq!(hit, local);
                        let index = state.page_start() + hit;
                        assert_eq!(state.input(Action::SelectAndActivate(index)), Some(index));
                        assert!(state.input(Action::Activate).is_none());
                        state.started();
                        state.finished("closed".into());
                        assert_eq!(state.selected, index);
                        assert_eq!(state.page_start(), page * 6);
                    }
                    if state.visible_count() < 6 {
                        let tile = layout.tiles[state.visible_count()];
                        assert_eq!(
                            layout.hit(
                                f64::from(tile.x + 1),
                                f64::from(tile.y + 1),
                                state.visible_count()
                            ),
                            None
                        );
                    }
                    state.input(Action::Page(true));
                }
                state.selected = 0;
                for expected in 1..count {
                    state.input(Action::Move(Direction::Right));
                    assert_eq!(state.selected, expected);
                }
                state.input(Action::Move(Direction::Right));
                assert_eq!(state.selected, count.saturating_sub(1));
                for expected in (0..count.saturating_sub(1)).rev() {
                    state.input(Action::Move(Direction::Left));
                    assert_eq!(state.selected, expected);
                }
            }
        }
        Ok(())
    }
    #[test]
    fn page_boundaries_and_partial_final_row_are_safe() -> Result<(), String> {
        let mut state = Launcher::new(apps(8), 3, 6)?;
        state.selected = 4;
        state.input(Action::Move(Direction::Down));
        assert_eq!(state.selected, 7);
        state.input(Action::Move(Direction::Down));
        assert_eq!(state.selected, 7);
        state.input(Action::Move(Direction::Up));
        assert_eq!(state.selected, 4);
        state.input(Action::Page(true));
        assert_eq!(state.selected, 7);
        state.input(Action::Page(false));
        assert_eq!(state.selected, 1);
        assert!(Launcher::new(apps(1), 3, 0).is_err());
        Ok(())
    }
}
