use super::*;

fn key(keycode: Keycode, keymod: Mod) -> Event {
    Event::KeyDown {
        timestamp: 0,
        window_id: 1,
        keycode: Some(keycode),
        scancode: None,
        keymod,
        repeat: false,
    }
}

#[test]
fn touch_keyboard_can_enter_every_printable_ascii_character() {
    let pages = [
        KeyboardPage::Lower,
        KeyboardPage::Upper,
        KeyboardPage::Symbols,
    ];
    for byte in b' '..=b'~' {
        assert!(
            pages
                .iter()
                .any(|page| page.keys().contains(char::from(byte))),
            "Missing key {}",
            char::from(byte)
        );
    }
}

#[test]
fn scrolling_a_short_final_page_keeps_focus_on_scroll_not_save() -> Result<(), String> {
    let layout = Layout::home(480, 272)?;
    let mut desktop = Desktop::default();
    desktop.add();
    desktop.selected = desktop
        .targets(&layout)
        .iter()
        .position(|(target, _, _)| *target == Target::Next)
        .ok_or("Down button")?;
    for _ in 0..3 {
        assert!(
            desktop
                .event(&key(Keycode::Return, Mod::NOMOD), &layout)
                .is_none()
        );
        assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Next);
    }
    Ok(())
}

#[test]
fn empty_desktop_toolbar_has_visible_keyboard_focus_and_matching_actions() -> Result<(), String> {
    use crate::input::DesktopAction;
    let layout = Layout::home(480, 272)?;
    let mut desktop = Desktop::default();
    for (target, action) in [(Toolbar::Actions, DesktopAction::Menu(None))] {
        desktop.toolbar = None;
        for _ in 0..3 {
            desktop.toolbar_event(&key(Keycode::Tab, Mod::NOMOD));
            if desktop.toolbar == Some(target) {
                break;
            }
        }
        assert_eq!(desktop.toolbar, Some(target));
        let bounds = target.bounds(&layout);
        assert!(bounds.w > 0 && bounds.h > 0);
        assert_eq!(
            desktop.toolbar_event(&key(Keycode::Return, Mod::NOMOD)),
            Some(action)
        );
    }
    Ok(())
}

#[test]
fn editing_shortcuts_keep_done_focused_and_cancel_does_not_commit() -> Result<(), String> {
    let layout = Layout::home(480, 272)?;
    let mut desktop = Desktop::default();
    desktop.add();
    desktop.draft.name = "Original".into();
    desktop.event(&key(Keycode::Return, Mod::NOMOD), &layout);
    assert!(desktop.editing());
    assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Done);
    desktop.event(&key(Keycode::A, Mod::LCTRLMOD), &layout);
    assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Done);
    desktop.event(
        &Event::TextInput {
            timestamp: 0,
            window_id: 1,
            text: "Edited".into(),
        },
        &layout,
    );
    desktop.event(&key(Keycode::Return, Mod::NOMOD), &layout);
    assert_eq!(desktop.draft.name, "Edited");
    desktop.event(&key(Keycode::Return, Mod::NOMOD), &layout);
    desktop.event(&key(Keycode::Backspace, Mod::NOMOD), &layout);
    desktop.event(&key(Keycode::Escape, Mod::NOMOD), &layout);
    assert_eq!(desktop.draft.name, "Edited");
    assert!(
        desktop
            .event(&key(Keycode::Escape, Mod::NOMOD), &layout)
            .is_none()
    );
    assert!(!desktop.open);
    Ok(())
}

#[test]
fn scroll_and_focus_reach_every_editor_action_at_both_sizes() -> Result<(), String> {
    for (width, height) in [(480, 272), (800, 480)] {
        let layout = Layout::home(width, height)?;
        let mut desktop = Desktop::default();
        desktop.add();
        let expected = desktop.rows();
        let mut seen = Vec::new();
        for _ in 0..3 {
            let targets = desktop.targets(&layout);
            for (_, _, bounds) in &targets {
                assert!(bounds.x >= 0 && bounds.y >= 0);
                assert!(bounds.x + bounds.w <= i32::from(width));
                assert!(bounds.y + bounds.h <= i32::from(height));
            }
            for _ in 0..targets.len() {
                seen.push(desktop.targets(&layout)[desktop.selected].0);
                desktop.event(&key(Keycode::Tab, Mod::NOMOD), &layout);
            }
            desktop.event(&key(Keycode::PageDown, Mod::NOMOD), &layout);
        }
        assert!(expected.iter().all(|(target, _)| seen.contains(target)));
        for required in [Target::Save, Target::Cancel] {
            assert!(seen.contains(&required));
        }
        desktop.page(Page::Remove);
        assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Cancel);
        assert!(
            desktop
                .event(&key(Keycode::Return, Mod::NOMOD), &layout)
                .is_none()
        );
        assert_eq!(desktop.page, Page::Menu);
    }
    Ok(())
}

#[test]
fn menu_labels_and_authority_come_from_provenance() {
    let mut app = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis")).remove(0);
    let mut desktop = Desktop::default();
    for source in [
        AppSource::Custom,
        AppSource::AppCenter,
        AppSource::PocketHome,
    ] {
        app.source = source;
        desktop.menu(Some(app.clone()));
        let rows = desktop.rows();
        assert!(rows.iter().any(|(target, _)| *target == Target::Add));
        assert_eq!(
            rows.iter().any(|(_, label)| label == "Edit shortcut"),
            source == AppSource::Custom
        );
        assert_eq!(
            rows.iter().any(|(_, label)| label == "Uninstall app"),
            source == AppSource::AppCenter
        );
        assert_eq!(
            rows.iter().any(|(_, label)| label == "Remove shortcut"),
            source != AppSource::AppCenter
        );
    }
    desktop.menu(None);
    assert_eq!(
        desktop.rows(),
        [
            (Target::Add, "Add shortcut".into()),
            (Target::CreateFolder, "Create folder".into())
        ]
    );
}

#[test]
fn folder_actions_keyboard_destinations_and_safe_delete_are_reachable() -> Result<(), String> {
    for (width, height) in [(480, 272), (800, 480)] {
        let layout = Layout::home(width, height)?;
        let mut desktop = Desktop::default();
        let id = format!("{}{}", crate::folders::PREFIX, "a".repeat(64));
        desktop.folders.names.insert(id.clone(), "Tools".into());
        desktop.folder_context = Some(id.clone());
        desktop.in_folder = true;
        desktop.menu(Some(
            crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis")).remove(0),
        ));
        for target in [
            Target::CreateFolder,
            Target::RenameFolder,
            Target::DeleteFolder,
            Target::MoveApp,
        ] {
            desktop.page(Page::Menu);
            let targets = desktop.targets(&layout);
            let index = targets
                .iter()
                .position(|(t, _, _)| *t == target)
                .ok_or("Missing folder action")?;
            for _ in 0..index {
                desktop.event(&key(Keycode::Tab, Mod::NOMOD), &layout);
            }
            desktop.event(&key(Keycode::Return, Mod::NOMOD), &layout);
            if target == Target::DeleteFolder {
                assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Cancel);
                assert!(
                    desktop
                        .event(&key(Keycode::Return, Mod::NOMOD), &layout)
                        .is_none()
                );
                assert_eq!(desktop.page, Page::Menu);
            } else if target == Target::MoveApp {
                assert_eq!(desktop.targets(&layout)[0].0, Target::Unfile);
                desktop.event(&key(Keycode::Down, Mod::NOMOD), &layout);
                assert!(
                    matches!(desktop.event(&key(Keycode::Return, Mod::NOMOD), &layout), Some(Request::Folder(crate::folders::Change::Move(_,Some(folder)))) if folder == id)
                );
            } else {
                assert!(desktop.editing());
                assert_eq!(desktop.targets(&layout)[desktop.selected].0, Target::Done);
            }
        }
        desktop.open = false;
        desktop.toolbar = None;
        desktop.toolbar_event(&key(Keycode::Tab, Mod::NOMOD));
        assert_eq!(desktop.toolbar, Some(Toolbar::Back));
        assert_eq!(
            desktop.toolbar_event(&key(Keycode::Return, Mod::NOMOD)),
            Some(crate::input::DesktopAction::Back)
        );
    }
    Ok(())
}
