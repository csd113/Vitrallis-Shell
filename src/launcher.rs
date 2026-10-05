use crate::{app::AppEntry, input::Action, navigation, process::AppState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ready,
    /// A start request is in flight; navigation stays live, activation does not.
    Launching,
    /// An application owns the foreground; the Shell backgrounds it on return.
    Running,
}

#[derive(Debug)]
pub struct Launcher {
    /// Startup capabilities, separate from periodically refreshed device status.
    pub renderer_info: Option<crate::renderer::backend::RendererInfo>,
    /// Authoritative lifecycle state per application ID.
    pub app_states: std::collections::BTreeMap<String, AppState>,
    pub preferences: crate::preferences::Preferences,
    pub settings: crate::settings::Settings,
    pub app_center: crate::app_center::Center,
    pub desktop: crate::shortcuts::screen::Desktop,
    pub apps: Vec<AppEntry>,
    pub all_apps: Vec<AppEntry>,
    pub folders: crate::folders::Folders,
    pub folder: Option<String>,
    pub view_changed: bool,
    pub selected: usize,
    pub phase: Phase,
    pub status: String,
    /// Allow `status` to be presented while an app launch is active.
    pub status_notice: bool,
    pub opening: Option<String>,
    pub error: Option<String>,
    columns: usize,
    capacity: usize,
}
impl Launcher {
    pub fn new(apps: Vec<AppEntry>, columns: usize, capacity: usize) -> Result<Self, String> {
        if columns == 0 || capacity == 0 || !capacity.is_multiple_of(columns) {
            return Err("invalid grid capacity".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for app in &apps {
            app.validate()?;
            if !ids.insert(app.id.as_str()) {
                return Err(format!("duplicate app id {}", app.id));
            }
        }
        Ok(Self {
            renderer_info: None,
            app_states: std::collections::BTreeMap::new(),
            preferences: crate::preferences::Preferences::default(),
            settings: crate::settings::Settings::default(),
            app_center: crate::app_center::Center::default(),
            desktop: crate::shortcuts::screen::Desktop::default(),
            selected: 0,
            phase: Phase::Ready,
            error: None,
            opening: None,
            status: if apps.is_empty() {
                "NO APPS - CHECK CONFIG / LOG"
            } else {
                "ARROWS: SELECT   ENTER / TAP: OPEN"
            }
            .into(),
            status_notice: true,
            all_apps: apps.clone(),
            folders: crate::folders::Folders::default(),
            folder: None,
            view_changed: false,
            apps,
            columns,
            capacity,
        })
    }
    /// Authoritative lifecycle state for one application.
    #[must_use]
    pub fn app_state(&self, id: &str) -> AppState {
        self.app_states
            .get(id)
            .copied()
            .unwrap_or(AppState::Stopped)
    }
    /// Mirror the process owner's lifecycle state for every known application.
    /// Called only on real transitions, never once per rendered frame.
    pub fn sync_states(&mut self, processes: &impl crate::process::Processes) {
        self.app_states.clear();
        self.app_center.running.clear();
        for app in &self.all_apps {
            let state = processes.state(&app.id);
            if state.is_running() {
                let _new_running_app = self.app_center.running.insert(app.id.clone());
            }
            if state != AppState::Stopped {
                let _previous_state = self.app_states.insert(app.id.clone(), state);
            }
        }
    }
    /// Record lifecycle feedback without changing the current phase.
    pub fn notify(&mut self, text: String) {
        self.status = text;
        self.status_notice = true;
    }
    /// The user asked to launch `name`: acknowledge before any process work so
    /// the main menu never looks frozen while the worker prepares the child.
    pub fn launching(&mut self, name: &str) {
        self.opening = Some(name.into());
        self.phase = Phase::Launching;
        self.notify(format!("{name} is launching..."));
        self.error = None;
    }
    /// An application owns the foreground now.
    pub fn launched(&mut self, name: &str) {
        self.opening = None;
        self.phase = Phase::Running;
        self.notify(format!("{name} is running - Enter / tap to switch back"));
    }
    /// Returns an app index only once per launch transition.
    pub fn input(&mut self, action: Action) -> Option<usize> {
        if self.error.is_some() {
            if matches!(
                action,
                Action::Back | Action::Activate | Action::SelectAndActivate(_)
            ) {
                self.error = None;
                self.status = "ARROWS: SELECT   ENTER / TAP: OPEN".into();
                self.status_notice = false;
            }
            return None;
        }
        let returned_from_app = self.phase == Phase::Running;
        if self.phase == Phase::Running {
            // The main menu never terminates or hides a running app: returning
            // home moves it into the background and leaves it running.
            self.returned_home();
        } else if self.phase == Phase::Launching {
            // Navigation stays responsive while an app starts, and the launching
            // message stays visible; a second launch waits for the one in flight.
            match action {
                Action::Move(direction) => {
                    self.selected =
                        navigation::moved(self.selected, direction, self.columns, self.apps.len());
                }
                Action::Page(forward) => self.page(forward),
                Action::Back if self.folder.is_some() => self.leave_folder(),
                Action::System | Action::Activate | Action::SelectAndActivate(_) | Action::Back => {
                }
            }
            return None;
        }
        self.desktop.toolbar = None;
        match action {
            Action::Move(direction) => {
                self.selected =
                    navigation::moved(self.selected, direction, self.columns, self.apps.len());
                if !returned_from_app {
                    self.status_notice = false;
                }
            }
            Action::Page(forward) => {
                self.page(forward);
                if !returned_from_app {
                    self.status_notice = false;
                }
            }
            Action::Back => {
                if self.folder.is_some() {
                    self.leave_folder();
                }
                if !returned_from_app {
                    self.status = "ARROWS: SELECT   ENTER / TAP: OPEN".into();
                    self.status_notice = false;
                }
            }
            Action::SelectAndActivate(index) if index < self.apps.len() => {
                self.selected = index;
                return self.input(Action::Activate);
            }
            Action::Activate if !self.apps.is_empty() => {
                self.selected = self.selected.min(self.apps.len().saturating_sub(1));
                let app = self.apps.get(self.selected)?;
                if app.source == crate::app::AppSource::Folder {
                    self.folder = Some(app.id.clone());
                    self.rebuild_view(None);
                    return None;
                }
                if app.id == crate::app_center::TILE_ID {
                    self.app_center.show();
                    return None;
                }
                if app.is_system_settings() {
                    self.settings.show();
                    return None;
                }
                let name = app.name.clone();
                self.launching(&name);
                return Some(self.selected);
            }
            Action::Activate | Action::SelectAndActivate(_) | Action::System => {}
        }
        None
    }
    fn page(&mut self, forward: bool) {
        let page = self.page_start().checked_div(self.capacity).unwrap_or(0);
        let target = if forward {
            page.saturating_add(1)
                .min(self.page_count().saturating_sub(1))
        } else {
            page.saturating_sub(1)
        };
        let local = self.selected.checked_rem(self.capacity).unwrap_or(0);
        self.selected = target
            .saturating_mul(self.capacity)
            .saturating_add(local)
            .min(self.apps.len().saturating_sub(1));
    }
    pub const fn page_start(&self) -> usize {
        let local = match self.selected.checked_rem(self.capacity) {
            Some(index) => index,
            None => 0,
        };
        self.selected.saturating_sub(local)
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
    pub fn returned_home(&mut self) {
        self.opening = None;
        if self.phase == Phase::Running {
            self.phase = Phase::Ready;
            self.notify("App continues in the background - Escape: close it".into());
        }
    }
    pub fn failed(&mut self, message: String) {
        self.opening = None;
        self.phase = Phase::Ready;
        self.status = "LAUNCH FAILED - ENTER / TAP: DISMISS".into();
        self.status_notice = true;
        self.error = Some(message);
    }
    pub fn finished(&mut self, message: String) {
        self.opening = None;
        self.error = None;
        self.phase = Phase::Ready;
        self.notify(message);
    }
    /// Commit a validated replacement only while idle, retaining selection by ID.
    pub fn reload(&mut self, apps: Vec<AppEntry>) -> Result<bool, String> {
        if self.phase != Phase::Ready {
            return Err("cannot reload while an app is running".into());
        }
        let replacement = Self::new(apps, self.columns, self.capacity)?;
        if self.all_apps == replacement.apps {
            return Ok(false);
        }
        let selected = self.apps.get(self.selected).map(|app| app.id.clone());
        self.all_apps = replacement.apps;
        self.rebuild_view(selected.as_deref());
        Ok(true)
    }
    pub fn rebuild_view(&mut self, selected: Option<&str>) {
        let policy_id = self
            .settings
            .policy_apps
            .get(self.settings.policy_app)
            .map(|(id, _)| id.clone());
        self.settings.policy_apps = self
            .all_apps
            .iter()
            .filter(|app| {
                !matches!(
                    app.source,
                    crate::app::AppSource::System | crate::app::AppSource::Folder
                )
            })
            .map(|app| (app.id.clone(), app.name.clone()))
            .collect();
        self.settings.policy_app = policy_id
            .and_then(|id| {
                self.settings
                    .policy_apps
                    .iter()
                    .position(|(app, _)| app == &id)
            })
            .unwrap_or(0);
        if self
            .folder
            .as_ref()
            .is_some_and(|id| !self.folders.names.contains_key(id))
        {
            self.folder = None;
        }
        self.apps = self.folders.view(&self.all_apps, self.folder.as_deref());
        self.selected = selected
            .and_then(|id| self.apps.iter().position(|app| app.id == id))
            .unwrap_or(0);
        self.desktop.in_folder = self.folder.is_some();
        self.desktop.toolbar = None;
        self.view_changed = true;
    }
    pub fn leave_folder(&mut self) {
        let old = self.folder.take();
        self.rebuild_view(old.as_deref());
    }
    pub fn reveal(&mut self, id: &str) {
        self.folder = self.folders.members.get(id).cloned();
        self.rebuild_view(Some(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_selection_is_clamped_before_activation() -> Result<(), String> {
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let last = apps.len().checked_sub(1).ok_or("missing demo apps")?;
        let mut state = Launcher::new(apps, 3, 6)?;
        state.selected = usize::MAX;
        assert_eq!(state.input(Action::Activate), Some(last));
        assert_eq!(state.selected, last);
        state.finished("closed".into());
        state.apps.clear();
        assert_eq!(state.input(Action::Activate), None);
        Ok(())
    }
    #[test]
    fn reload_preserves_identity_and_rejects_invalid_replacements() -> Result<(), String> {
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let mut state = Launcher::new(apps.clone(), 3, 6)?;
        state.selected = 1;
        let mut reordered = apps.clone();
        reordered.reverse();
        assert!(
            state.reload(reordered)?,
            "reordering must update the app view"
        );
        assert_eq!(
            state
                .apps
                .get(state.selected)
                .ok_or("selected app missing")?
                .id,
            apps.get(1).ok_or("demo app missing")?.id
        );
        let mut invalid = apps.clone();
        invalid.first_mut().ok_or("demo app missing")?.name.clear();
        assert!(state.reload(invalid).is_err());
        assert_eq!(state.apps.len(), apps.len());
        state.launching("Test");
        state.launched("Test");
        assert!(state.reload(vec![]).is_err());
        state.finished("closed".into());
        assert!(state.reload(vec![])?, "clearing apps must update the view");
        assert_eq!(state.selected, 0);
        assert!(state.input(Action::Activate).is_none());
        assert!(state.reload(apps)?, "restoring apps must update the view");
        assert!(state.input(Action::Activate).is_some());
        Ok(())
    }
    #[test]
    fn system_settings_tile_opens_the_same_screen_without_spawning() -> Result<(), String> {
        let mut apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let settings_app = apps.first_mut().ok_or("demo app missing")?;
        settings_app.id = "vitrallis-wifi-settings".into();
        settings_app.name = "System Settings".into();
        let mut state = Launcher::new(apps, 3, 6)?;
        assert_eq!(state.input(Action::Activate), None);
        assert!(state.settings.open);
        assert_eq!(state.phase, Phase::Ready);
        assert_eq!(state.settings.input(Action::Back), None);
        assert_eq!(state.settings.input(Action::System), None);
        assert!(state.settings.open);
        Ok(())
    }
    #[test]
    fn activation_is_guarded_and_recovers() -> Result<(), String> {
        use crate::platform::Platform;
        let backend = crate::platform::generic::Mock;
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/mock/vitrallis"));
        assert_eq!(backend.resolution(), (480, 272));
        assert!(!backend.fullscreen());
        let mut state = Launcher::new(apps, 3, 6)?;
        assert_eq!(state.input(Action::SelectAndActivate(1)), Some(1));
        // The launch is acknowledged immediately; the menu stays interactive.
        assert_eq!(state.phase, Phase::Launching);
        assert!(state.status_notice);
        assert!(state.status.contains("launching"));
        assert_eq!(state.input(Action::Activate), None);
        assert_eq!(
            state.input(Action::Move(crate::navigation::Direction::Right)),
            None
        );
        assert_eq!(state.selected, 2);
        state.launching("Test");
        state.launched("Test");
        assert_eq!(state.phase, Phase::Running);
        // Returning home backgrounds the app instead of terminating it.
        assert_eq!(state.input(Action::Back), None);
        assert_eq!(state.phase, Phase::Ready);
        assert!(state.status.contains("background"));
        state.finished("done".into());
        assert_eq!(state.input(Action::Activate), Some(2));
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
        let duplicate_id = apps.first().ok_or("demo app missing")?.id.clone();
        apps.get_mut(1).ok_or("second demo app missing")?.id = duplicate_id;
        assert!(Launcher::new(apps, 3, 6).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod pagination_tests {
    use super::*;
    use crate::{input::PointerInput, layout::Layout, navigation::Direction};
    fn apps(count: usize) -> Result<Vec<AppEntry>, String> {
        let seed = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"))
            .into_iter()
            .next()
            .ok_or("demo app missing")?;
        Ok((0..count)
            .map(|i| {
                let mut app = seed.clone();
                app.id = format!("app-{i}");
                app
            })
            .collect())
    }
    #[test]
    fn every_page_keyboard_touch_hitbox_and_focus_persistence() -> Result<(), String> {
        use sdl2::{event::Event, mouse::MouseButton};
        for count in [0, 1, 5, 6, 7, 11, 12, 13, 61] {
            for (width, height) in [(480, 272), (800, 480), (1280, 720)] {
                let layout = Layout::home(width, height)?;
                let mut state = Launcher::new(apps(count)?, 3, 6)?;
                let mut pointer = PointerInput::default();
                assert_eq!(state.page_count(), count.div_ceil(6).max(1));
                for page in 0..state.page_count() {
                    assert_eq!(state.page_start(), page * 6);
                    for local in 0..state.visible_count() {
                        let tile = layout.tiles.get(local).ok_or("visible tile missing")?;
                        let down = Event::MouseButtonDown {
                            timestamp: 0,
                            window_id: 1,
                            which: 0,
                            mouse_btn: MouseButton::Left,
                            clicks: 1,
                            x: tile.x + tile.w / 2_i32,
                            y: tile.y + tile.h / 2_i32,
                        };
                        let up = Event::MouseButtonUp {
                            timestamp: 0,
                            window_id: 1,
                            which: 0,
                            mouse_btn: MouseButton::Left,
                            clicks: 1,
                            x: tile.x + tile.w / 2_i32,
                            y: tile.y + tile.h / 2_i32,
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
                        let index = state
                            .page_start()
                            .checked_add(hit)
                            .ok_or("page index overflow")?;
                        assert_eq!(state.input(Action::SelectAndActivate(index)), Some(index));
                        assert!(state.input(Action::Activate).is_none());
                        state.launching("Test");
                        state.launched("Test");
                        state.finished("closed".into());
                        assert_eq!(state.selected, index);
                        assert_eq!(state.page_start(), page * 6);
                    }
                    if state.visible_count() < 6 {
                        let tile = layout
                            .tiles
                            .get(state.visible_count())
                            .ok_or("unused tile missing")?;
                        assert_eq!(
                            layout.hit(
                                f64::from(tile.x + 1_i32),
                                f64::from(tile.y + 1_i32),
                                state.visible_count()
                            ),
                            None
                        );
                    }
                    assert_eq!(state.input(Action::Page(true)), None);
                }
                state.selected = 0;
                for expected in 1..count {
                    assert_eq!(state.input(Action::Move(Direction::Right)), None);
                    assert_eq!(state.selected, expected);
                }
                assert_eq!(state.input(Action::Move(Direction::Right)), None);
                assert_eq!(state.selected, count.saturating_sub(1));
                for expected in (0..count.saturating_sub(1)).rev() {
                    assert_eq!(state.input(Action::Move(Direction::Left)), None);
                    assert_eq!(state.selected, expected);
                }
            }
        }
        Ok(())
    }
    #[test]
    fn page_boundaries_and_partial_final_row_are_safe() -> Result<(), String> {
        let mut state = Launcher::new(apps(8)?, 3, 6)?;
        state.selected = 4;
        assert_eq!(state.input(Action::Move(Direction::Down)), None);
        assert_eq!(state.selected, 7);
        assert_eq!(state.input(Action::Move(Direction::Down)), None);
        assert_eq!(state.selected, 7);
        assert_eq!(state.input(Action::Move(Direction::Up)), None);
        assert_eq!(state.selected, 4);
        assert_eq!(state.input(Action::Page(true)), None);
        assert_eq!(state.selected, 7);
        assert_eq!(state.input(Action::Page(false)), None);
        assert_eq!(state.selected, 1);
        assert!(Launcher::new(apps(1)?, 3, 0).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;
    #[test]
    fn opening_folder_launches_real_app_and_back_restores_folder_selection() -> Result<(), String> {
        let apps = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis"));
        let mut state = Launcher::new(apps.clone(), 3, 6)?;
        let id = format!("{}{}", crate::folders::PREFIX, "a".repeat(64));
        assert!(
            state
                .folders
                .names
                .insert(id.clone(), "Utilities".into())
                .is_none()
        );
        let app = apps.first().ok_or("demo app missing")?;
        assert!(
            state
                .folders
                .members
                .insert(app.id.clone(), id.clone())
                .is_none()
        );
        state.rebuild_view(None);
        assert!(state.input(Action::Activate).is_none());
        assert_eq!(state.apps.as_slice(), std::slice::from_ref(app));
        assert_eq!(state.input(Action::Activate), Some(0));
        assert_eq!(
            state.apps.first().ok_or("folder app missing")?.manifest,
            app.manifest
        );
        state.launching("Test");
        state.launched("Test");
        state.returned_home();
        assert_eq!(state.input(Action::Back), None);
        assert_eq!(
            state
                .apps
                .get(state.selected)
                .ok_or("folder selection missing")?
                .id,
            id
        );
        state.reveal(&app.id);
        assert_eq!(state.folder.as_deref(), Some(id.as_str()));
        Ok(())
    }
}
