//! Applications page: background lifetime policy and the keep-running app list.
//! Persist local preferences before applying them to the running shell.
use super::{Page, Settings};
use crate::{input::Action, navigation::Direction};

/// Settings rows shown for application behaviour.
pub const APP_ROWS: usize = 3;

impl Settings {
    pub(super) fn preferences_input(&mut self, action: Action) {
        match action {
            Action::Back | Action::System | Action::Page(_) => self.page(Page::Home),
            Action::SelectAndActivate(index) if index < APP_ROWS => {
                self.selected = index;
                self.preferences_input(Action::Activate);
            }
            Action::Activate | Action::Move(Direction::Left | Direction::Right)
                if self.selected < APP_ROWS =>
            {
                let mut policy = self.policy.clone();
                let forward = action != Action::Move(Direction::Left);
                match self.selected {
                    0 => {
                        let choices = [0, 60, 300, 900, 1800, 3600, 86400];
                        policy.background_seconds = if forward {
                            choices
                                .into_iter()
                                .find(|v| *v > policy.background_seconds)
                                .unwrap_or(0)
                        } else {
                            choices
                                .into_iter()
                                .rev()
                                .find(|v| *v < policy.background_seconds)
                                .unwrap_or(86400)
                        };
                    }
                    1 if !self.policy_apps.is_empty() => {
                        self.policy_app = if forward {
                            self.policy_app
                                .saturating_add(1)
                                .checked_rem(self.policy_apps.len())
                                .unwrap_or(0)
                        } else {
                            self.policy_app
                                .checked_sub(1)
                                .unwrap_or_else(|| self.policy_apps.len().saturating_sub(1))
                        };
                        return;
                    }
                    2 => {
                        let Some((id, _)) = self.policy_apps.get(self.policy_app) else {
                            return;
                        };
                        if !policy.essential.remove(id) {
                            let _new_essential_app = policy.essential.insert(id.clone());
                        }
                    }
                    _ => return,
                }
                match policy.save() {
                    Ok(()) => {
                        self.policy = policy;
                        self.message.clear();
                    }
                    Err(error) => self.message = error,
                }
            }
            Action::SelectAndActivate(_) | Action::Activate => {}
            Action::Move(direction) => self.move_rows(direction),
        }
    }
    /// Background lifetime rows, shared by the renderer and the tests. Each row
    /// keeps the same two-line shape as every other Settings category.
    pub fn preference_rows(&self) -> [(String, String); APP_ROWS] {
        let app = self.policy_apps.get(self.policy_app);
        [
            (
                "Close background apps".into(),
                if self.policy.background_seconds == 0 {
                    "< never >".into()
                } else {
                    format!("< after {} >", seconds(self.policy.background_seconds))
                },
            ),
            (
                "App".into(),
                app.map_or_else(|| "None".into(), |(_, name)| name.clone()),
            ),
            (
                "Keep running".into(),
                if app.is_some_and(|(id, _)| self.policy.essential.contains(id)) {
                    "Yes - never closes it automatically".into()
                } else if self.policy.background_seconds == 0 {
                    "Not needed while closing is off".into()
                } else {
                    "No - asked to close first".into()
                },
            ),
        ]
    }
}

fn seconds(value: u32) -> String {
    match value {
        0 => "never".into(),
        _ if value.is_multiple_of(3600) => format!("{} hour", value / 3600),
        _ if value.is_multiple_of(60) => format!("{} min", value / 60),
        _ => format!("{value} sec"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_uses_the_selected_app_id_and_reaches_the_footer() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Applications);
        settings.policy_apps = vec![
            ("first".into(), "Same name".into()),
            ("second".into(), "Same name".into()),
        ];
        assert!(settings.policy.essential.insert("second".into()));
        settings.policy.background_seconds = 300;
        settings.selected = 1;
        assert_eq!(settings.input(Action::Move(Direction::Right)), None);
        assert_eq!(settings.policy_app, 1);
        assert_eq!(
            settings.preference_rows()[2].1,
            "Yes - never closes it automatically"
        );
        assert_eq!(settings.input(Action::Move(Direction::Left)), None);
        assert_eq!(settings.policy_app, 0);
        assert!(settings.preference_rows()[2].1.contains("asked to close"));
        assert_eq!(settings.input(Action::Move(Direction::Down)), None);
        assert_eq!(settings.selected, 2);
        assert_eq!(settings.input(Action::Move(Direction::Down)), None);
        assert_eq!(settings.selected, super::super::footer::BACK);
        assert_eq!(settings.input(Action::Move(Direction::Up)), None);
        assert_eq!(settings.selected, 2);
        assert_eq!(settings.input(Action::Back), None);
        assert_eq!(settings.page, Page::Home);
    }
    #[test]
    fn background_lifetime_labels_are_human_readable() {
        assert_eq!(seconds(0), "never");
        assert_eq!(seconds(60), "1 min");
        assert_eq!(seconds(900), "15 min");
        assert_eq!(seconds(3600), "1 hour");
        assert_eq!(seconds(45), "45 sec");
    }
}
