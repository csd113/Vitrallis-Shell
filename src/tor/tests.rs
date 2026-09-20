use super::*;
#[test]
fn manifest_requirements_are_explicit_and_existing_apps_remain_unchanged() -> Result<(), String> {
    let manifest = include_str!("../../tests/fixtures/app-center/app.toml");
    assert_eq!(
        Requirement::parse(&metadata::manifest(manifest.as_bytes())?)?,
        Requirement::None
    );
    for (value, requirement) in [
        ("required", Requirement::Required),
        ("preferred", Requirement::Preferred),
        ("none", Requirement::None),
    ] {
        let bytes = format!("{manifest}\n[network]\ntor = \"{value}\"\n");
        assert_eq!(
            Requirement::parse(&metadata::manifest(bytes.as_bytes())?)?,
            requirement
        );
    }
    for value in ["true", "\"sometimes\"", "3", "[]"] {
        assert!(
            metadata::manifest(format!("{manifest}\n[network]\ntor = {value}\n").as_bytes())
                .is_err()
        );
    }
    assert!(
        metadata::manifest(format!("{manifest}\n[network]\ntro = \"required\"\n").as_bytes())
            .is_err()
    );
    Ok(())
}
#[test]
fn startup_mode_persistence_rejects_malformed_configuration() -> Result<(), String> {
    let scratch = crate::test_support::Scratch::new().map_err(|e| e.to_string())?;
    let root = scratch.0.canonicalize().map_err(|e| e.to_string())?;
    let paths = Paths {
        config: root.join("tor.json"),
        root,
    };
    assert_eq!(paths.load()?, Mode::OnDemand);
    for mode in [Mode::AlwaysOn, Mode::Disabled, Mode::OnDemand] {
        paths.save(mode)?;
        assert_eq!(paths.load()?, mode);
    }
    for value in [
        "{}",
        "{\"startup\":\"yes\"}",
        "{\"startup\":\"disabled\",\"startup\":\"always-on\"}",
    ] {
        std::fs::write(&paths.config, value).map_err(|e| e.to_string())?;
        assert!(paths.load().is_err());
    }
    Ok(())
}
#[test]
fn tor_actions_have_shared_keyboard_and_touch_targets() -> Result<(), String> {
    use crate::{
        input::Action,
        layout::Layout,
        navigation::Direction,
        settings::{Page, PanelLayout, Settings},
    };
    let mut settings = Settings::default();
    settings.show();
    settings.page(Page::Wireless);
    settings.selected = 6;
    settings.input(Action::Move(Direction::Right));
    settings.input(Action::Activate);
    assert_eq!(settings.page, Page::Tor);
    for (index, control) in [
        Control::Start,
        Control::Stop,
        Control::Restart,
        Control::Mode(Mode::Disabled),
        Control::Mode(Mode::AlwaysOn),
    ]
    .into_iter()
    .enumerate()
    {
        settings.selected = index;
        settings.input(Action::Activate);
        assert_eq!(settings.tor_control.take(), Some(control));
        settings.input(Action::SelectAndActivate(index));
        assert_eq!(settings.tor_control.take(), Some(control));
    }
    settings.input(Action::SelectAndActivate(5));
    assert_eq!(settings.page, Page::TorDetails);
    settings.input(Action::Activate);
    assert_eq!(settings.page, Page::Tor);
    let geometry = PanelLayout::tor_controls(&Layout::home(480, 272)?);
    assert!(geometry.iter().all(|rect| rect.w >= 100 && rect.h >= 30));
    Ok(())
}

#[derive(Default)]
struct FakeApp;
impl crate::process::Processes for FakeApp {
    fn start(&mut self, _: &crate::app::AppEntry) -> Result<(), String> {
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        Ok(None)
    }
}
#[test]
fn required_launch_waits_for_readiness_and_fails_closed_while_preferred_can_continue()
-> Result<(), String> {
    use crate::process::{ProcessSet, Processes};
    let mut app = crate::platform::generic::demo_apps(std::path::Path::new("/vitrallis")).remove(0);
    for requirement in [
        Requirement::Required,
        Requirement::Preferred,
        Requirement::None,
    ] {
        for state in [
            State::Disabled,
            State::Error,
            State::Bootstrapping,
            State::Connected,
        ] {
            let mut processes = ProcessSet::<FakeApp>::default();
            let (tx, _rx) = mpsc::sync_channel(32);
            processes.tor.tx = Some(tx);
            *processes.tor.snapshot.lock().map_err(|e| e.to_string())? = Snapshot {
                state,
                ..Snapshot::default()
            };
            app.manifest.tor = requirement;
            processes.start(&app)?;
            if requirement == Requirement::None {
                assert_eq!(processes.running_ids(), [app.id.clone()]);
                continue;
            }
            assert!(processes.running_ids().is_empty());
            let result = processes.poll_focus();
            if matches!(state, State::Disabled | State::Error)
                && requirement == Requirement::Required
            {
                assert!(result.is_err());
                assert!(processes.running_ids().is_empty());
            } else if state == State::Bootstrapping {
                assert_eq!(result?, None);
                assert!(processes.running_ids().is_empty());
                *processes.tor.snapshot.lock().map_err(|e| e.to_string())? = Snapshot {
                    state: State::Connected,
                    ..Snapshot::default()
                };
                processes.poll_focus()?;
                assert_eq!(processes.running_ids(), [app.id.clone()]);
            } else {
                result?;
                assert_eq!(processes.running_ids(), [app.id.clone()]);
            }
        }
    }
    Ok(())
}

#[test]
fn required_exported_launcher_cannot_become_a_direct_exec() -> Result<(), String> {
    let direct = b"#!/bin/sh\nexec '/usr/bin/app'\n".to_vec();
    let wrapped = String::from_utf8(wrap_launcher(
        direct.clone(),
        Requirement::Required,
        std::path::Path::new("/home/user/tor"),
    )?)
    .map_err(|e| e.to_string())?;
    assert!(wrapped.starts_with("#!/bin/sh\nexec /usr/bin/bwrap "));
    assert!(wrapped.contains("'--unshare-net'"));
    assert!(wrapped.contains("'--die-with-parent'"));
    assert_eq!(
        wrap_launcher(
            direct.clone(),
            Requirement::None,
            std::path::Path::new("/tmp")
        )?,
        direct
    );
    Ok(())
}

#[test]
fn tor_buttons_use_matched_pointer_release_and_visible_keyboard_focus() -> Result<(), String> {
    use crate::{
        input::Action,
        layout::Layout,
        navigation::Direction,
        settings::{Page, PanelLayout, Settings},
    };
    use sdl2::{event::Event, mouse::MouseButton};
    let layout = Layout::home(480, 272)?;
    for (index, bounds) in PanelLayout::tor_controls(&layout).into_iter().enumerate() {
        let mut touch = Settings::default();
        touch.show();
        touch.page(Page::Tor);
        let down = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 1,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: bounds.x + bounds.w / 2,
            y: bounds.y + bounds.h / 2,
        };
        let up = Event::MouseButtonUp {
            timestamp: 0,
            window_id: 1,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: bounds.x + bounds.w / 2,
            y: bounds.y + bounds.h / 2,
        };
        touch.event(&down, &layout);
        assert!(touch.tor_control.is_none());
        touch.event(&up, &layout);
        let mut keyboard = Settings::default();
        keyboard.show();
        keyboard.page(Page::Tor);
        for _ in 0..index {
            keyboard.input(Action::Move(Direction::Right));
        }
        assert_eq!(keyboard.selected, index);
        keyboard.input(Action::Activate);
        assert_eq!(keyboard.tor_control, touch.tor_control);
        assert_eq!(keyboard.page, touch.page);
    }
    Ok(())
}
