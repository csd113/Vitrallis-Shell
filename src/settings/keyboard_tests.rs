//! Every Settings control must be reachable and operable with the keyboard
//! alone, with the same behaviour as touch. These tests walk the whole menu.
use super::{
    Page, PanelLayout, Request, Settings,
    footer::{BACK, NEXT, PREVIOUS, REFRESH},
};
use crate::{
    layout::Layout,
    platform::system::{Control, Percent, Power},
};
use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
};

/// The PocketCHIP-sized layout every keyboard walk is exercised against. Its
/// validity is covered by the layout tests, so a failure here is a regression.
fn layout() -> Layout {
    Layout::home(480, 272).unwrap_or_else(|error| {
        unreachable!("the 480x272 PocketCHIP layout is validated by the layout tests: {error}")
    })
}

fn key(settings: &mut Settings, keycode: Keycode) -> Option<Request> {
    let layout = layout();
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

/// Walk the home menu with arrow keys only and confirm every option opens.
#[test]
fn every_home_option_is_reachable_and_returns_with_escape() -> Result<(), String> {
    let mut settings = Settings::default();
    settings.show();
    for index in 0..super::HOME_ROWS {
        settings.selected = index;
        move_keys(&mut settings, &[Keycode::Return]);
        let page = super::home_page(index).ok_or_else(|| format!("home option {index}"))?;
        assert_eq!(settings.page, page);
        // One Escape always returns to the home menu, never out of Settings.
        move_keys(&mut settings, &[Keycode::Escape]);
        assert_eq!(settings.page, Page::Home);
        assert!(settings.open);
    }
    // The home menu is reached by arrows alone, without remembering indices.
    settings.selected = 0;
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::Wireless);
    move_keys(&mut settings, &[Keycode::Escape]);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert!(!settings.open);
    Ok(())
}

#[test]
fn display_and_datetime_use_only_arrow_keys() -> Result<(), String> {
    let mut settings = Settings::default();
    settings.show();
    settings.status.brightness = Some(Percent::new(50)?);
    settings.status.volume = Some(Percent::new(50)?);
    settings.status.screen_timeout = Some(600);
    settings.status.timezone = Some("America/Vancouver".into());
    settings.status.timezones = vec!["America/Vancouver".into()];
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Display);
    assert_eq!(
        key(&mut settings, Keycode::Right),
        Some(Request::Control(Control::Brightness(Percent::new(60)?)))
    );
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(
        key(&mut settings, Keycode::Left),
        Some(Request::Control(Control::Volume(Percent::new(40)?)))
    );
    // Down past the last slider reaches the visible footer Back control.
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Home);
    // Date & Time: clock format and the time-zone list.
    settings.selected = 1;
    move_keys(&mut settings, &[Keycode::Return, Keycode::Return]);
    assert_eq!(settings.page, Page::DateTime);
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::Timezones);
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::Control(Control::Timezone(0)))
    );
    assert_eq!(settings.page, Page::DateTime);
    Ok(())
}

#[test]
fn system_controls_and_confirmations_are_reachable_using_only_keys() {
    let mut settings = Settings::default();
    settings.show();
    settings.status.power_controls = true;
    settings.status.calibration = true;
    settings.status.screen_timeout = Some(600);
    settings.network_available = true;
    // Home option 2 is Wireless Network.
    settings.selected = 2;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Wireless);
    settings.status.wifi_enabled = Some(false);
    assert_eq!(
        key(&mut settings, Keycode::Right),
        Some(Request::Control(Control::Radio(
            crate::platform::system::Radio::Wifi,
            true
        )))
    );
    move_keys(&mut settings, &[Keycode::Down, Keycode::Down]);
    assert_eq!(key(&mut settings, Keycode::Return), Some(Request::Network));
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.page, Page::Tor);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert_eq!(settings.page, Page::Wireless);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert_eq!(settings.page, Page::Home);
    // Device: screen timeout, calibration and both power actions.
    settings.selected = 5;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Device);
    assert_eq!(
        key(&mut settings, Keycode::Left),
        Some(Request::Control(Control::ScreenTimeout(300)))
    );
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(
        key(&mut settings, Keycode::Return),
        Some(Request::Calibration)
    );
    // Restart and Power off each need a distinct confirmation; the default is
    // always Cancel, and cancelling keeps the page and lets the user retry.
    for (index, power) in [(2, Power::Reboot), (3, Power::Shutdown)] {
        settings.page(Page::Device);
        settings.selected = index;
        move_keys(&mut settings, &[Keycode::Return]);
        assert!(settings.confirmation.is_some());
        assert_eq!(key(&mut settings, Keycode::Return), None);
        assert!(settings.confirmation.is_none());
        assert_eq!(settings.page, Page::Device);
        settings.selected = index;
        move_keys(&mut settings, &[Keycode::Return, Keycode::Right]);
        assert_eq!(
            key(&mut settings, Keycode::Return),
            Some(Request::Control(Control::Power(power)))
        );
        assert_eq!(settings.page, Page::Device);
    }
    // Escape cancels a pending power confirmation before leaving the page.
    settings.selected = 2;
    move_keys(&mut settings, &[Keycode::Return, Keycode::Escape]);
    assert!(settings.confirmation.is_none());
    assert_eq!(settings.page, Page::Device);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert_eq!(settings.page, Page::Home);
}

#[test]
fn updates_are_reachable_and_confirmable_using_only_keys() -> Result<(), String> {
    let mut settings = Settings::default();
    settings.show();
    settings.selected = 6;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Updates);
    move_keys(&mut settings, &[Keycode::Right]);
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
    // Dismiss the pending confirmation, then leave through the footer Back.
    settings.update_confirmation = None;
    settings.selected = 0;
    move_keys(&mut settings, &[Keycode::Down]);
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Home);
    Ok(())
}

#[test]
fn timezone_selector_pages_and_applies_with_keys_only() {
    for count in [1, 5, 6, 11] {
        let mut settings = Settings::default();
        settings.status.timezones = (0..count).map(|index| format!("Zone/{index}")).collect();
        settings.show();
        settings.page(Page::Timezones);
        // Down past the last visible zone reaches the visible Back control.
        for _ in 0..count.min(5) {
            move_keys(&mut settings, &[Keycode::Down]);
        }
        assert_eq!(settings.selected, BACK);
        move_keys(&mut settings, &[Keycode::Return]);
        assert_eq!(settings.page, Page::DateTime);
        // The first zone applies straight back to Date & Time.
        settings.page(Page::Timezones);
        assert_eq!(
            key(&mut settings, Keycode::Return),
            Some(Request::Control(Control::Timezone(0)))
        );
        assert_eq!(settings.page, Page::DateTime);
    }
    // Previous and Next are visible footer controls with keyboard focus.
    let mut settings = Settings::default();
    settings.status.timezones = (0..11).map(|index| format!("Zone/{index}")).collect();
    settings.show();
    settings.page(Page::Timezones);
    settings.selected = NEXT;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.zone_start, 5);
    settings.selected = PREVIOUS;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.zone_start, 0);
    assert_eq!(settings.page, Page::Timezones);
}

#[test]
fn visible_footer_targets_have_identical_touch_and_keyboard_actions() -> Result<(), String> {
    for (width, height) in [(320, 200), (480, 272), (800, 480), (1280, 720)] {
        let layout = Layout::home(width, height)?;
        for page in [
            Page::Display,
            Page::DateTime,
            Page::Device,
            Page::Timezones,
            Page::Updates,
            Page::Storage,
            Page::Wireless,
            Page::Applications,
            Page::Tor,
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
fn storage_is_a_first_class_section_with_keyboard_navigation() {
    let mut settings = Settings::default();
    settings.show();
    settings.selected = 4;
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.page, Page::Storage);
    assert_eq!(settings.storage_parent, Page::Home);
    move_keys(&mut settings, &[Keycode::Return]);
    assert_eq!(settings.storage_view, super::StorageView::Apps);
    // With no report yet, focus stays on the visible Back control.
    assert_eq!(settings.selected, BACK);
    move_keys(&mut settings, &[Keycode::Right]);
    assert_eq!(settings.selected, REFRESH);
    move_keys(&mut settings, &[Keycode::Right]);
    assert_eq!(settings.selected, NEXT);
    move_keys(&mut settings, &[Keycode::Return, Keycode::Escape]);
    assert_eq!(settings.storage_view, super::StorageView::Overview);
    move_keys(&mut settings, &[Keycode::Down, Keycode::Return]);
    assert_eq!(settings.storage_view, super::StorageView::Categories);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert_eq!(settings.storage_view, super::StorageView::Overview);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert_eq!(settings.page, Page::Home);
    assert!(settings.open);
    move_keys(&mut settings, &[Keycode::Escape]);
    assert!(!settings.open);
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

/// A green path through Settings must never contain a dead end: every category
/// returns to the home menu, and only the home menu exits Settings.
#[test]
fn settings_navigation_has_no_dead_ends() {
    for index in 0..super::HOME_ROWS {
        let mut settings = Settings::default();
        settings.show();
        settings.status.timezones = (0..6).map(|zone| format!("Zone/{zone}")).collect();
        settings.selected = index;
        move_keys(&mut settings, &[Keycode::Return]);
        let opened = settings.page;
        assert_ne!(opened, Page::Home, "home option {index} did not open");
        // Walking the first rows of the category must not close Settings.
        move_keys(&mut settings, &[Keycode::Down, Keycode::Down]);
        move_keys(&mut settings, &[Keycode::Up, Keycode::Up]);
        assert!(settings.open, "home option {index} closed Settings");
        move_keys(&mut settings, &[Keycode::Escape]);
        if settings.page != Page::Home {
            // Nested pages (Tor details, time zones) need one more step back.
            move_keys(&mut settings, &[Keycode::Escape]);
        }
        assert_eq!(settings.page, Page::Home, "home option {index} dead end");
        assert!(settings.open, "home option {index} exited Settings early");
    }
}
