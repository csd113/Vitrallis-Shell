//! Precompiled Rust executable validation. Never invokes Cargo on the device.
use std::path::Path;

pub fn validate_binary(bytes: &[u8], target: &str) -> Result<(), String> {
    let (class, machine) = match target {
        "armv7-unknown-linux-gnueabihf" => (1, 40_u16),
        "aarch64-unknown-linux-gnu" => (2, 183),
        "x86_64-unknown-linux-gnu" => (2, 62),
        _ => return Err(format!("Unsupported native target: {target}")),
    };
    let invalid = || format!("Native binary is not a compatible ELF executable for {target}");
    let Some([0x7f, b'E', b'L', b'F', binary_class, 1, 1, abi]) = bytes.get(..8) else {
        return Err(invalid());
    };
    let Some([kind_low, kind_high, machine_low, machine_high]) = bytes.get(16..20) else {
        return Err(invalid());
    };
    if bytes.len() < 64
        || *binary_class != class
        || !matches!(*abi, 0 | 3)
        || !matches!(u16::from_le_bytes([*kind_low, *kind_high]), 2 | 3)
        || u16::from_le_bytes([*machine_low, *machine_high]) != machine
    {
        return Err(invalid());
    }
    if class == 1 {
        let flags = u32::from_le_bytes(
            bytes
                .get(36..40)
                .ok_or("Missing ELF flags")?
                .try_into()
                .map_err(|error| format!("Invalid ELF flags: {error}"))?,
        );
        if flags & 0xff00_0000 != 0x0500_0000 || flags & 0x600 != 0x400 {
            return Err("ARM binary must use EABI5 hard-float".into());
        }
    }
    Ok(())
}

pub fn launcher(executable_path: &Path) -> Result<Vec<u8>, String> {
    let entry = executable_path
        .to_str()
        .ok_or("Native executable path must be UTF-8")?;
    Ok(format!("#!/bin/sh\nexec '{}'\n", entry.replace('\'', "'\\''")).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_foreign_architectures_scripts_and_soft_float() -> Result<(), String> {
        let mut arm = vec![0; 64];
        arm.get_mut(..7)
            .ok_or("Missing ELF fixture header")?
            .copy_from_slice(b"\x7fELF\x01\x01\x01");
        *arm.get_mut(16).ok_or("Missing ELF fixture type")? = 2;
        *arm.get_mut(18).ok_or("Missing ELF fixture machine")? = 40;
        arm.get_mut(36..40)
            .ok_or("Missing ELF fixture flags")?
            .copy_from_slice(&0x0500_0400_u32.to_le_bytes());
        assert!(validate_binary(&arm, "armv7-unknown-linux-gnueabihf").is_ok());
        assert!(validate_binary(&arm, "aarch64-unknown-linux-gnu").is_err());
        arm.get_mut(36..40)
            .ok_or("Missing ELF fixture flags")?
            .copy_from_slice(&0x0500_0200_u32.to_le_bytes());
        assert!(validate_binary(&arm, "armv7-unknown-linux-gnueabihf").is_err());
        assert!(validate_binary(b"#!/bin/sh\nexit 0\n", "x86_64-unknown-linux-gnu").is_err());
        Ok(())
    }
}
