use super::{
    Page, PanelLayout, Request, Settings,
    footer::{BACK, NEXT, ZONE_BACK},
};
use crate::{
    layout::Layout,
    platform::system::{Control, Percent, Power},
};
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
};

fn key(settings: &mut Settings, keycode: Keycode) -> Option<Request> {
    let layout = Layout::home(480, 272).unwrap();
    settings.event(
        &Event::KeyDown {
            timestamp: 0,
            window_id: 1,
            keycode: Some(keycode),
            scancode: None,
            keymod: Mod::NOMOD,
            repeat: false,
        },
        &layout,
    )
}

fn move_keys(settings: &mut Settings, keys: &[Keycode]) {
    for &keycode in keys {
        assert_eq!(key(settings, keycode), None);
    }
}

#[test]
fn general_footer_moves_left_from_more_to_back_and_activates_with_enter() {
    let mut settings = Settings::default();
    settings.show();
    move_keys(
        &mut settings,
        &[
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
        ],
    );
    assert_eq!(settings.selected, NEXT);
    move_keys(&mut settings, &[Keycode::Left, Keycode::Left]);
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Right, Keycode::Right]);
    assert_eq!(settings.selected, NEXT);
    move_keys(&mut settings, &[Keycode::Up]);
    assert_eq!(settings.selected, 4);
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::Device);
    move_keys(
        &mut settings,
        &[Keycode::Down, Keycode::Down, Keycode::Down, Keycode::Down],
    );
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Up]);
    assert_eq!(settings.selected, 3);
    settings.pending = true;
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::General);
    move_keys(
        &mut settings,
        &[
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Left,
            Keycode::Left,
            Keycode::Return,
        ],
    );
    assert!(!settings.open);
}

#[test]
fn system_controls_and_confirmations_are_reachable_using_only_keys() -> Result<(), String> {
    let mut settings = Settings::default();
    settings.show();
    settings.status.brightness = Some(Percent::new(50)?);
    settings.status.volume = Some(Percent::new(50)?);
    settings.status.power_controls = true;
    settings.status.calibration = true;
    settings.status.screen_timeout = Some(600);
    settings.network_available = true;
    assert_eq!(
        key(&mut settings, Keycode::Right),
        Some(Request::Control(Control::Brightness(Percent::new(60)?)))
    );
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(
        key(&mut settings, Keycode::Left),
        Some(Request::Control(Control::Volume(Percent::new(40)?)))
    );
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(key(&mut settings, Keycode::Return), Some(Request::Network));
    move_keys(&mut settings, &[Keycode::Right, Keycode::Return]);
    assert!(settings.confirmation.is_some());
    move_keys(&mut settings, &[Keycode::Return]);
    assert!(settings.confirmation.is_none());
    move_keys(
        &mut settings,
        &[
            Keycode::Down,
            Keycode::Down,
            Keycode::Right,
            Keycode::Return,
            Keycode::Right,
        ],
    );
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::Control(Control::Power(Power::Reboot)))
    );
    move_keys(
        &mut settings,
        &[
            Keycode::Down,
            Keycode::Down,
            Keycode::Right,
            Keycode::Right,
            Keycode::Return,
            Keycode::Right,
        ],
    );
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::Control(Control::Power(Power::Shutdown)))
    );
    move_keys(
        &mut settings,
        &[
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Return,
        ],
    );
    assert_eq!(
        key(&mut settings, Keycode::Left),
        Some(Request::Control(Control::ScreenTimeout(300)))
    );
    move_keys(&mut settings, &[Keycode::Down, Keycode::Down]);
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::Calibration)
    );
    Ok(())
}

#[test]
fn updates_are_reachable_and_confirmable_using_only_keys() -> Result<(), String> {
    let mut settings = Settings::default();
    settings.show();
    move_keys(
        &mut settings,
        &[Keycode::PageDown, Keycode::Down, Keycode::Down],
    );
    move_keys(
        &mut settings,
        &[Keycode::Down, Keycode::Return, Keycode::Right],
    );
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::CheckUpdates)
    );
    settings.updater.state = crate::updater::State::Available(crate::updater::tests::release()?);
    move_keys(&mut settings, &[Keycode::Return]);
    assert!(settings.update_confirmation.is_some());
    assert_eq!(settings.selected, 0);
    move_keys(&mut settings, &[Keycode::Return]);
    assert!(settings.update_confirmation.is_none());
    move_keys(
        &mut settings,
        &[
            Keycode::Right,
            Keycode::Return,
            Keycode::Right,
            Keycode::Down,
        ],
    );
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Return]);
    assert!(settings.update_confirmation.is_none());
    assert_eq!(settings.page, Page::Updates);
    move_keys(
        &mut settings,
        &[Keycode::Right, Keycode::Return, Keycode::Right],
    );
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::InstallUpdate)
    );
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Up, Keycode::Right]);
    assert_eq!(settings.selected, 1);
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::Device);
    Ok(())
}

#[test]
fn timezone_footer_handles_partial_pages_and_returns_without_applying() {
    for count in [1, 5, 6, 11] {
        let mut settings = Settings::default();
        settings.status.timezones = (0..count).map(|index| format!("Zone/{index}")).collect();
        settings.show();
        settings.page(Page::Device);
        move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
        assert_eq!(settings.page, Page::Timezones);
        for _ in 0..count.min(5) {
            move_keys(&mut settings, &[Keycode::Down]);
        }
        assert_eq!(settings.selected, ZONE_BACK);
        move_keys(&mut settings, &[Keycode::Right, Keycode::Return]);
        assert_eq!(settings.zone_start, if count > 5 { 5 } else { 0 });
        for _ in 0..settings.visible_zones() {
            move_keys(&mut settings, &[Keycode::Down]);
        }
        move_keys(&mut settings, &[Keycode::Left, Keycode::Return]);
        assert_eq!(settings.zone_start, 0);
        move_keys(&mut settings, &[Keycode::Right]);
        assert_eq!(settings.zone_start, if count > 5 { 5 } else { 0 });
        for _ in 0..settings.visible_zones() - settings.selected {
            move_keys(&mut settings, &[Keycode::Down]);
        }
        move_keys(&mut settings, &[Keycode::Up]);
        assert_eq!(settings.selected, settings.visible_zones() - 1);
        assert_eq!(
            key(&mut settings, Keycode::Return),
            Some(Request::Control(Control::Timezone(count.min(10) - 1)))
        );
        move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
        for _ in 0..settings.visible_zones() {
            move_keys(&mut settings, &[Keycode::Down]);
        }
        move_keys(&mut settings, &[Keycode::Return]);
        assert_eq!(settings.page, Page::Device);
    }
}

#[test]
fn visible_footer_targets_have_identical_touch_and_keyboard_actions() -> Result<(), String> {
    for (width, height) in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
        let layout = Layout::home(width, height)?;
        for page in [
            Page::General,
            Page::Device,
            Page::Timezones,
            Page::Updates,
            Page::Storage,
            Page::Wireless,
        ] {
            let mut keyboard = Settings::default();
            keyboard.show();
            keyboard.page(page);
            for (bounds, control) in PanelLayout::footer(&layout)
                .into_iter()
                .zip(keyboard.footer_controls())
            {
                let Some((index, _)) = control else { continue };
                let mut touch = Settings::default();
                touch.show();
                touch.page(page);
                touch.status.timezones = (0..6).map(|zone| format!("Zone/{zone}")).collect();
                keyboard.show();
                keyboard.page(page);
                keyboard.zone_start = 0;
                keyboard
                    .status
                    .timezones
                    .clone_from(&touch.status.timezones);
                keyboard.selected = index;
                let x = f32::from(u16::try_from(bounds.x + bounds.w / 2).map_err(|_| "x")?)
                    / f32::from(width);
                let y = f32::from(u16::try_from(bounds.y + bounds.h * 3 / 4).map_err(|_| "y")?)
                    / f32::from(height);
                let up = Event::FingerUp {
                    timestamp: 0,
                    touch_id: 1,
                    finger_id: 1,
                    x,
                    y,
                    dx: 0.,
                    dy: 0.,
                    pressure: 0.,
                };
                assert_eq!(touch.event(&up, &layout), None);
                assert_eq!(touch.page, page);
                touch.event(
                    &Event::FingerDown {
                        timestamp: 0,
                        touch_id: 1,
                        finger_id: 1,
                        x,
                        y,
                        dx: 0.,
                        dy: 0.,
                        pressure: 1.,
                    },
                    &layout,
                );
                assert_eq!(
                    touch.event(&up, &layout),
                    key(&mut keyboard, Keycode::Return)
                );
                assert_eq!(
                    (touch.open, touch.page, touch.selected, touch.zone_start),
                    (
                        keyboard.open,
                        keyboard.page,
                        keyboard.selected,
                        keyboard.zone_start
                    )
                );
            }
        }
    }
    Ok(())
}

#[test]
fn storage_navigation_empty_states_refresh_and_safe_back_are_keyboard_accessible() {
    let mut settings = Settings::default();
    settings.show();
    move_keys(
        &mut settings,
        &[
            Keycode::PageDown,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Down,
            Keycode::Right,
            Keycode::Right,
            Keycode::Return,
        ],
    );
    assert_eq!(settings.page, Page::Storage);
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.storage_view, super::StorageView::Apps);
    assert_eq!(settings.selected, BACK);
    move_keys(
        &mut settings,
        &[
            Keycode::Right,
            Keycode::Return,
            Keycode::Right,
            Keycode::Return,
        ],
    );
    assert_eq!(settings.storage_start, 0);
    move_keys(
        &mut settings,
        &[Keycode::Escape, Keycode::Down, Keycode::Return],
    );
    assert_eq!(settings.storage_view, super::StorageView::Categories);
    move_keys(
        &mut settings,
        &[
            Keycode::Return,
            Keycode::Down,
            Keycode::Down,
            Keycode::Right,
            Keycode::Return,
        ],
    );
    assert_eq!(settings.page, Page::Storage);
    move_keys(&mut settings, &[Keycode::Left, Keycode::Return]);
    assert_eq!(settings.page, Page::Device);
    assert!(settings.open);
}

#[test]
fn storage_app_paging_handles_partial_last_pages_and_details() {
    let mut settings = Settings::default();
    settings.show();
    settings.page(Page::Storage);
    settings.storage.report = Some(crate::storage::Report {
        apps: (0..7)
            .map(|index| crate::app_center::accounting::AppUsage {
                id: format!("io.test.app{index}"),
                name: format!("App {index}"),
                icon: None,
                parts: Default::default(),
                total: crate::storage::scan::Size::default(),
            })
            .collect(),
        ..Default::default()
    });
    move_keys(&mut settings, &[Keycode::Return, Keycode::PageDown]);
    assert_eq!(settings.storage_start, 4);
    assert_eq!(settings.storage_rows(), 3);
    move_keys(
        &mut settings,
        &[Keycode::Down, Keycode::Down, Keycode::Return],
    );
    assert_eq!(settings.storage_view, super::StorageView::App(6));
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Return, Keycode::PageUp]);
    assert_eq!(settings.storage_start, 0);
    assert_eq!(settings.storage_rows(), 4);
}
