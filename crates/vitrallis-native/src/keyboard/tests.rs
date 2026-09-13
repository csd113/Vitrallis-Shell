use super::*;

fn key(keycode: Keycode, keymod: Mod) -> Event {
    Event::KeyDown {
        timestamp: 123,
        window_id: 1,
        keycode: Some(keycode),
        scancode: None,
        keymod,
        repeat: false,
    }
}

#[test]
fn function_row_consumes_fn_and_preserves_other_modifiers_and_repeats() {
    let mut keyboard = Keyboard::from_compatible(b"nextthing,pocketchip\0nextthing,chip\0");
    let row = [
        (Keycode::Num1, Keycode::F1),
        (Keycode::Num2, Keycode::F2),
        (Keycode::Num3, Keycode::F3),
        (Keycode::Num4, Keycode::F4),
        (Keycode::Num5, Keycode::F5),
        (Keycode::Num6, Keycode::F6),
        (Keycode::Num7, Keycode::F7),
        (Keycode::Num8, Keycode::F8),
        (Keycode::Num9, Keycode::F9),
        (Keycode::Num0, Keycode::F10),
        (Keycode::Minus, Keycode::F11),
        (Keycode::Equals, Keycode::F12),
    ];
    for fn_mod in [Mod::RALTMOD, Mod::MODEMOD] {
        for modifiers in [Mod::NOMOD, Mod::LSHIFTMOD, Mod::LCTRLMOD | Mod::LALTMOD] {
            for repeat in [false, true] {
                for (base, function) in row {
                    let mut event = key(base, fn_mod | modifiers);
                    let mut expected = key(function, modifiers);
                    for event in [&mut event, &mut expected] {
                        if let Event::KeyDown { repeat: value, .. } = event {
                            *value = repeat;
                        }
                    }
                    keyboard.event(&mut event);
                    assert_eq!(event, expected);
                }
            }
        }
    }
}

#[test]
fn desktop_altgr_and_unmodified_keys_are_unchanged() {
    for compatible in [
        b"".as_slice(),
        b"nextthing,chip\0",
        b"nextthing,pocketchip-other\0",
    ] {
        let mut keyboard = Keyboard::from_compatible(compatible);
        for code in [Keycode::Num2, Keycode::Y, Keycode::F2] {
            let mut event = key(code, Mod::RALTMOD);
            keyboard.event(&mut event);
            assert_eq!(event, key(code, Mod::RALTMOD));
        }
    }
    let mut keyboard = Keyboard::from_compatible(b"nextthing,pocketchip\0");
    for modifiers in [Mod::NOMOD, Mod::LSHIFTMOD, Mod::LALTMOD] {
        for code in [Keycode::Num2, Keycode::Minus, Keycode::Y, Keycode::F2] {
            let mut event = key(code, modifiers);
            keyboard.event(&mut event);
            assert_eq!(event, key(code, modifiers));
        }
    }
}

#[test]
fn fn_text_keeps_x11_punctuation_and_uses_event_modifiers() {
    let mut keyboard = Keyboard::from_compatible(b"nextthing,pocketchip\0");
    for modifiers in [Mod::NOMOD, Mod::LALTMOD, Mod::LSHIFTMOD] {
        keyboard.event(&mut key(Keycode::Y, Mod::RALTMOD | modifiers));
        let mut text = Event::TextInput {
            timestamp: 123,
            window_id: 1,
            text: "{}[]|<>\"'`~:;\\".into(),
        };
        let expected = text.clone();
        keyboard.event(&mut text);
        assert_eq!(text, expected);
        assert_eq!(keyboard.text_modifiers(), modifiers);
    }
    // SDL can already have processed key-up when queued text is consumed.
    // Keep its producing key-down's modifiers until the next key-down.
    keyboard.event(&mut key(Keycode::X, Mod::LALTMOD));
    keyboard.event(&mut Event::KeyUp {
        timestamp: 124,
        window_id: 1,
        keycode: Some(Keycode::LAlt),
        scancode: None,
        keymod: Mod::NOMOD,
        repeat: false,
    });
    assert_eq!(keyboard.text_modifiers(), Mod::LALTMOD);
    keyboard.event(&mut key(Keycode::Y, Mod::NOMOD));
    assert_eq!(keyboard.text_modifiers(), Mod::NOMOD);
    keyboard.event(&mut key(Keycode::X, Mod::LCTRLMOD));
    keyboard.event(&mut Event::Window {
        timestamp: 125,
        window_id: 1,
        win_event: WindowEvent::FocusLost,
    });
    assert_eq!(keyboard.text_modifiers(), Mod::NOMOD);
}
