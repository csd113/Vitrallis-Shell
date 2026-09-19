//! Read-only installer notices. Privileged GPU setup lives in the platform helper.
use std::{fs, io::Read, path::Path};

const STATUS: &str = "/var/lib/vitrallis-pocketchip/gpu-status.json";
const INVALID: &str = "GPU setup status invalid; rerun PocketCHIP installer.";
const SETUP: &str = "GPU setup required; rerun PocketCHIP installer.";

pub fn setup_notice() -> Option<&'static str> {
    let path = Path::new(STATUS);
    let Ok(metadata) = fs::symlink_metadata(path) else {
        let compatible = read_regular(Path::new("/sys/firmware/devicetree/base/compatible"))?;
        return compatible
            .split(|byte| *byte == 0)
            .any(|value| value == b"nextthing,pocketchip")
            .then_some(SETUP);
    };
    // A malformed/restored status must not make the UI open a FIFO or device.
    if !metadata.file_type().is_file() || metadata.len() > 4096 {
        return Some(INVALID);
    }
    read_regular(path).map_or(Some(INVALID), |bytes| notice(&bytes))
}

fn read_regular(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > 4096 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(4097)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 4096).then_some(bytes)
}

fn notice(bytes: &[u8]) -> Option<&'static str> {
    let Ok(state) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Some(INVALID);
    };
    match state
        .get("reboot_required")
        .and_then(serde_json::Value::as_bool)
    {
        Some(true) => Some("Reboot PocketCHIP to activate GPU utilization support."),
        Some(false) => {
            if state
                .get("trace_configured")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                None
            } else {
                Some("GPU tracing unavailable; rerun PocketCHIP installer.")
            }
        }
        None => Some(INVALID),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reboot_notice_survives_missing_trace_and_requires_a_system_restart() {
        let message = notice(br#"{"reboot_required":true,"trace_configured":false}"#);
        assert!(message.is_some_and(|text| text.contains("Reboot PocketCHIP")));
        assert!(notice(br#"{"reboot_required":false,"trace_configured":true}"#).is_none());
        assert!(
            notice(br#"{"reboot_required":false,"trace_configured":false}"#)
                .is_some_and(|text| text.contains("tracing unavailable"))
        );
    }

    #[test]
    fn invalid_status_does_not_claim_success() {
        for bytes in [
            b"".as_slice(),
            b"{}",
            b"{",
            br#"{"reboot_required":"false"}"#,
        ] {
            assert_eq!(notice(bytes), Some(INVALID));
        }
    }
}
