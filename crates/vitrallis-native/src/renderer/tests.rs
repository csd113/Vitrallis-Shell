use super::*;

fn info(name: &'static str, flags: u32) -> SdlInfo {
    SdlInfo {
        name,
        flags,
        texture_formats: vec![sdl2::pixels::PixelFormatEnum::RGB24],
        max_texture_width: 2048,
        max_texture_height: 2048,
    }
}

fn drivers() -> Vec<(u32, SdlInfo)> {
    vec![
        (0, info("metal", ACCELERATED | VSYNC)),
        (1, info("software", SOFTWARE)),
        (2, info("opengles2", ACCELERATED | VSYNC)),
    ]
}

#[test]
fn auto_prefers_advertised_gles2_and_accepts_hardware() -> Result<(), String> {
    let (selected, actual, fallback) = select(RendererMode::Auto, &drivers(), |attempt| {
        assert_eq!(attempt.index, 2);
        assert!(attempt.vsync);
        Ok((attempt, info("opengles2", ACCELERATED | VSYNC)))
    })?;
    assert_eq!(selected.mode, RendererMode::Hardware);
    assert_eq!(actual.name, "opengles2");
    assert!(fallback.is_none());
    Ok(())
}

#[test]
fn acceleration_survives_vsync_failure_in_auto_and_hardware_modes() -> Result<(), String> {
    for mode in [RendererMode::Auto, RendererMode::Hardware] {
        let mut attempts = Vec::new();
        let ((), actual, fallback) = select(mode, &drivers(), |attempt| {
            attempts.push(attempt);
            if attempt.vsync {
                Err("vsync unavailable".into())
            } else {
                Ok(((), info("opengles2", ACCELERATED)))
            }
        })?;
        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts[1].index, 0);
        assert!(attempts[1].vsync);
        assert_eq!(attempts[2].index, 2);
        assert_eq!(actual.flags, ACCELERATED);
        assert!(fallback.is_none());
    }
    Ok(())
}

#[test]
fn failed_gles2_tries_other_accelerated_drivers() -> Result<(), String> {
    let (selected, _, fallback) = select(RendererMode::Auto, &drivers(), |attempt| {
        if attempt.index == 2 {
            Err("GLES2 unavailable on this video driver".into())
        } else {
            Ok((attempt, info("metal", ACCELERATED | VSYNC)))
        }
    })?;
    assert_eq!(selected.index, 0);
    assert!(selected.vsync);
    assert!(fallback.is_none());
    Ok(())
}

#[test]
fn auto_falls_back_after_all_hardware_attempts_and_retains_errors() -> Result<(), String> {
    let mut attempts = Vec::new();
    let (selected, _, fallback) = select(RendererMode::Auto, &drivers(), |attempt| {
        attempts.push(attempt);
        if attempt.mode == RendererMode::Hardware {
            Err("GPU unavailable".into())
        } else {
            Ok((attempt, info("software", SOFTWARE)))
        }
    })?;
    assert_eq!(attempts.len(), 5);
    assert_eq!(selected.mode, RendererMode::Software);
    assert!(selected.vsync);
    let error = fallback.ok_or("missing hardware failure")?;
    assert!(error.contains("opengles2"));
    assert!(error.contains("metal"));
    assert!(error.contains("vsync=false: GPU unavailable"));
    Ok(())
}

#[test]
fn hardware_required_never_calls_software_and_reports_recovery() -> Result<(), String> {
    let result = select::<()>(RendererMode::Hardware, &drivers(), |attempt| {
        assert_eq!(attempt.mode, RendererMode::Hardware);
        Err("no GPU".into())
    });
    let error = result
        .err()
        .ok_or("a hardware-only selection without a GPU must fail")?;
    assert!(error.contains("no GPU"));
    assert!(error.contains("--renderer software"));
    Ok(())
}

#[test]
fn explicit_software_skips_hardware() -> Result<(), String> {
    let ((), _, fallback) = select(RendererMode::Software, &drivers(), |attempt| {
        assert_eq!(attempt.mode, RendererMode::Software);
        assert_eq!(attempt.index, 1);
        assert!(attempt.vsync);
        Ok(((), info("software", SOFTWARE)))
    })?;
    assert!(fallback.is_none());
    Ok(())
}

#[test]
fn advertised_acceleration_is_not_enough() -> Result<(), String> {
    let (selected, _, fallback) = select(RendererMode::Auto, &drivers(), |attempt| {
        Ok((attempt, info("software", SOFTWARE)))
    })?;
    assert_eq!(selected.mode, RendererMode::Software);
    assert!(
        fallback
            .ok_or("missing rejection")?
            .contains("do not satisfy hardware")
    );
    for flags in [0, SOFTWARE, ACCELERATED | SOFTWARE] {
        assert!(verify(RendererMode::Hardware, &info("unexpected", flags)).is_err());
    }
    assert!(verify(RendererMode::Software, &info("unexpected", ACCELERATED)).is_err());
    Ok(())
}

#[test]
fn no_advertised_hardware_falls_back_and_total_failure_preserves_context() -> Result<(), String> {
    let software = [(0, info("software", SOFTWARE))];
    let ((), _, fallback) = select(RendererMode::Auto, &software, |_| {
        Ok(((), info("software", SOFTWARE)))
    })?;
    assert!(fallback.ok_or("missing reason")?.contains("no accelerated"));
    let error = select::<()>(RendererMode::Auto, &drivers(), |attempt| {
        Err(format!("{} failed", attempt.mode.as_str()))
    })
    .err()
    .ok_or("a total renderer failure must be reported")?;
    assert!(error.contains("software failed"));
    assert!(error.contains("hardware failed"));
    assert!(select::<()>(RendererMode::Auto, &[], |_| unreachable!()).is_err());
    Ok(())
}

#[test]
fn diagnostics_report_actual_flags_dimensions_and_escape_fallback_errors() {
    let mut snapshot = RendererInfo {
        requested: RendererMode::Auto,
        actual: RendererMode::Hardware,
        sdl: info("opengles2", ACCELERATED | VSYNC),
        video_driver: "x11".into(),
        window_size: (480, 272),
        output_size: (480, 272),
        display_size: Some((480, 272)),
        hardware_error: None,
        gl: None,
    };
    let log = snapshot.to_string();
    assert!(log.contains("event=renderer_initialized requested=auto mode=hardware"));
    assert!(log.contains("accelerated=true software=false vsync=true"));
    assert!(log.contains("max_texture_width=2048 max_texture_height=2048"));
    assert!(log.contains("video_driver=\"x11\" window_width=480 window_height=272"));
    assert!(
        log.contains("output_width=480 output_height=272 display_width=480 display_height=272")
    );
    assert!(log.ends_with("fallback=false"));
    snapshot.actual = RendererMode::Software;
    snapshot.sdl = info("software", SOFTWARE);
    snapshot.display_size = None;
    snapshot.hardware_error = Some("GPU \"failed\"\ninjected=event".into());
    let log = snapshot.to_string();
    assert!(log.contains("accelerated=false software=true vsync=false"));
    assert!(log.contains("display_width=unknown display_height=unknown"));
    assert!(log.contains("fallback=true hardware_error=\"GPU \\\"failed\\\"\\ninjected=event\""));
    assert_eq!(log.lines().count(), 1);
}

#[test]
fn software_mesa_rejects_hardware_and_auto_keeps_reason() -> Result<(), String> {
    let gl = graphics::GlInfo {
        vendor: "Mesa".into(),
        renderer: "llvmpipe (LLVM 19)".into(),
        version: "OpenGL ES 3.2 Mesa".into(),
        egl_version: None,
        egl_display_driver: None,
    };
    for requested in [RendererMode::Auto, RendererMode::Hardware] {
        let result = select(requested, &drivers(), |attempt| {
            if attempt.mode == RendererMode::Hardware {
                reject_software_gl(Some(&gl))?;
            }
            Ok(((), info("software", SOFTWARE)))
        });
        if requested == RendererMode::Hardware {
            let error = result
                .err()
                .ok_or("hardware-only selection must reject llvmpipe")?;
            assert!(error.contains("llvmpipe"));
        } else {
            let ((), actual, error) = result?;
            assert_eq!(actual.name, "software");
            assert!(
                error
                    .ok_or("missing software Mesa failure")?
                    .contains("llvmpipe")
            );
        }
    }
    assert!(reject_software_gl(None).is_ok());
    Ok(())
}

#[test]
fn synchronized_backend_wins_over_unsynchronized_preferred_driver() -> Result<(), String> {
    let (selected, _, _) = select(RendererMode::Auto, &drivers(), |attempt| {
        Ok((
            attempt,
            info(
                "driver",
                ACCELERATED | if attempt.index == 0 { VSYNC } else { 0 },
            ),
        ))
    })?;
    assert_eq!(selected.index, 0);
    assert!(selected.vsync);
    Ok(())
}

#[test]
fn software_without_vsync_still_has_a_buffered_fallback() -> Result<(), String> {
    let (selected, _, _) = select(RendererMode::Software, &drivers(), |attempt| {
        if attempt.vsync {
            Err("VSync unavailable".into())
        } else {
            Ok((attempt, info("software", SOFTWARE)))
        }
    })?;
    assert!(!selected.vsync);
    Ok(())
}
