//! Persist local preferences before applying them to the running shell.
use super::{Page, Settings, footer::BACK};
use crate::{input::Action, navigation::Direction};
pub(super) const PREFERENCES: usize = 9;
impl Settings {
    pub(super) fn preferences_input(&mut self, action: Action) {
        match action {
            Action::Back | Action::System | Action::Page(_) => self.page(Page::Device),
            Action::Move(Direction::Up) => self.selected = self.selected.saturating_sub(1),
            Action::Move(Direction::Down) => {
                self.selected = if self.selected >= 3 {
                    BACK
                } else {
                    self.selected + 1
                }
            }
            Action::SelectAndActivate(index) if index < 4 => {
                self.selected = index;
                self.preferences_input(Action::Activate);
            }
            Action::Activate | Action::Move(Direction::Left | Direction::Right) => {
                let mut policy = self.policy.clone();
                let forward = action != Action::Move(Direction::Left);
                match self.selected {
                    0 => policy.ampm = !policy.ampm,
                    1 => {
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
                    2 if !self.policy_apps.is_empty() => {
                        self.policy_app = if forward {
                            (self.policy_app + 1) % self.policy_apps.len()
                        } else {
                            self.policy_app
                                .checked_sub(1)
                                .unwrap_or(self.policy_apps.len() - 1)
                        };
                        return;
                    }
                    3 => {
                        let Some((id, _)) = self.policy_apps.get(self.policy_app) else {
                            return;
                        };
                        if !policy.essential.remove(id) {
                            policy.essential.insert(id.clone());
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
            _ => (),
        }
    }
    pub fn preference_rows(&self) -> [String; 4] {
        let app = self.policy_apps.get(self.policy_app);
        [
            format!(
                "Clock: {}",
                if self.policy.ampm {
                    "12 hour"
                } else {
                    "24 hour"
                }
            ),
            if self.policy.background_seconds == 0 {
                "Background timeout: Disabled".into()
            } else {
                format!("Background timeout: {} sec", self.policy.background_seconds)
            },
            format!("App: {}", app.map_or("None", |(_, name)| name)),
            if app.is_some_and(|(id, _)| self.policy.essential.contains(id)) {
                "Essential / Keep Running: Yes".into()
            } else if self.policy.background_seconds == 0 {
                "Essential: No (timeout disabled)".into()
            } else {
                "Essential: No / safe close on timeout".into()
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_navigation_and_effective_policy_use_selected_app_id() {
        let mut settings = Settings::default();
        settings.show();
        settings.page(Page::Device);
        settings.input(Action::SelectAndActivate(PREFERENCES));
        assert_eq!(settings.page, Page::Preferences);
        settings.policy_apps = vec![
            ("first".into(), "Same name".into()),
            ("second".into(), "Same name".into()),
        ];
        settings.policy.essential.insert("second".into());
        settings.policy.background_seconds = 300;
        settings.input(Action::SelectAndActivate(2));
        assert_eq!(settings.policy_app, 1);
        assert_eq!(
            settings.preference_rows()[3],
            "Essential / Keep Running: Yes"
        );
        settings.input(Action::Move(Direction::Left));
        assert_eq!(settings.policy_app, 0);
        assert!(settings.preference_rows()[3].contains("safe close"));
        settings.input(Action::Move(Direction::Down));
        assert_eq!(settings.selected, 3);
        settings.input(Action::Move(Direction::Down));
        assert_eq!(settings.selected, BACK);
        settings.input(Action::Move(Direction::Up));
        assert_eq!(settings.selected, 3);
        settings.input(Action::Back);
        assert_eq!(settings.page, Page::Device);
    }
}
