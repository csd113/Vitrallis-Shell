//! Precompiled Rust executable validation. Never invokes Cargo on the device.
use std::path::Path;

pub fn validate_binary(bytes: &[u8], target: &str) -> Result<(), String> {
    let (class, machine) = match target {
        "armv7-unknown-linux-gnueabihf" => (1, 40_u16),
        "aarch64-unknown-linux-gnu" => (2, 183),
        "x86_64-unknown-linux-gnu" => (2, 62),
        _ => return Err(format!("Unsupported native target: {target}")),
    };
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != class
        || bytes[5] != 1
        || bytes[6] != 1
        || !matches!(bytes[7], 0 | 3)
        || !matches!(u16::from_le_bytes([bytes[16], bytes[17]]), 2 | 3)
        || u16::from_le_bytes([bytes[18], bytes[19]]) != machine
    {
        return Err(format!(
            "Native binary is not a compatible ELF executable for {target}"
        ));
    }
    if class == 1 {
        let flags = u32::from_le_bytes(bytes[36..40].try_into().map_err(|_| "Invalid ELF flags")?);
        if flags & 0xff00_0000 != 0x0500_0000 || flags & 0x600 != 0x400 {
            return Err("ARM binary must use EABI5 hard-float".into());
        }
    }
    Ok(())
}

pub fn launcher(entry: &Path) -> Result<Vec<u8>, String> {
    let entry = entry
        .to_str()
        .ok_or("Native executable path must be UTF-8")?;
    Ok(format!("#!/bin/sh\nexec '{}'\n", entry.replace('\'', "'\\''")).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_foreign_architectures_scripts_and_soft_float() {
        let mut arm = vec![0; 64];
        arm[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
        arm[16] = 2;
        arm[18] = 40;
        arm[36..40].copy_from_slice(&0x0500_0400_u32.to_le_bytes());
        assert!(validate_binary(&arm, "armv7-unknown-linux-gnueabihf").is_ok());
        assert!(validate_binary(&arm, "aarch64-unknown-linux-gnu").is_err());
        arm[36..40].copy_from_slice(&0x0500_0200_u32.to_le_bytes());
        assert!(validate_binary(&arm, "armv7-unknown-linux-gnueabihf").is_err());
        assert!(validate_binary(b"#!/bin/sh\nexit 0\n", "x86_64-unknown-linux-gnu").is_err());
    }
}
