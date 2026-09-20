//! Fixed inventory streaming bundle: no archive paths, compression, or arbitrary entries.
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
pub const MAGIC: &[u8; 16] = b"VITRALLIS-BUNDLE";
pub const BINARIES: [&str; 5] = [
    "vitrallis",
    "vitrallis-terminal",
    "vitrallis-notepad",
    "vitrallis-files",
    "arti",
];
pub const MAX_BINARY: u64 = 64 * 1024 * 1024;
pub const MAX_BUNDLE: u64 = 16 + 5 * (40 + MAX_BINARY);

pub fn unpack(
    input: &mut impl Read,
    directory: &Path,
    verify: impl Fn(&[u8]) -> Result<(), String>,
) -> Result<[u8; 32], String> {
    let mut magic = [0; 16];
    input.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC {
        return Err("Invalid Vitrallis bundle header".into());
    }
    let mut shell_digest = [0; 32];
    for (index, name) in BINARIES.iter().enumerate() {
        let digest = extract(input, &directory.join(name), &verify)?;
        if index == 0 {
            shell_digest = digest;
        }
    }
    let mut extra = [0; 1];
    if input.read(&mut extra).map_err(|e| e.to_string())? != 0 {
        return Err("Unexpected trailing bundle content".into());
    }
    File::open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(shell_digest)
}
fn extract(
    input: &mut impl Read,
    path: &Path,
    verify: &impl Fn(&[u8]) -> Result<(), String>,
) -> Result<[u8; 32], String> {
    let mut size = [0; 8];
    let mut expected = [0; 32];
    input
        .read_exact(&mut size)
        .and_then(|()| input.read_exact(&mut expected))
        .map_err(|e| e.to_string())?;
    let size = u64::from_le_bytes(size);
    if !(64..=MAX_BINARY).contains(&size) {
        return Err("Bundle executable has an invalid size".into());
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut buffer = [0; 16384];
    let mut hash = Sha256::new();
    let mut remaining = size;
    let mut header = [0; 64];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    verify(&header)?;
    output.write_all(&header).map_err(|e| e.to_string())?;
    hash.update(header);
    remaining -= 64;
    while remaining > 0 {
        let count =
            usize::try_from(remaining.min(buffer.len() as u64)).map_err(|_| "Bundle size")?;
        input
            .read_exact(&mut buffer[..count])
            .and_then(|()| output.write_all(&buffer[..count]))
            .map_err(|e| e.to_string())?;
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    let digest: [u8; 32] = hash.finalize().into();
    if digest != expected {
        return Err(format!("Bundle checksum failed: {}", path.display()));
    }
    output
        .set_permissions(std::fs::Permissions::from_mode(0o755))
        .and_then(|()| output.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(digest)
}

#[cfg(test)]
pub fn fixture(payloads: &[Vec<u8>; 5]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    for payload in payloads {
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&Sha256::digest(payload));
        bytes.extend_from_slice(payload);
    }
    bytes
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_inventory_hashes_and_truncation() -> Result<(), Box<dyn std::error::Error>> {
        let data = fixture(&[
            vec![0; 64],
            vec![1; 64],
            vec![2; 64],
            vec![3; 64],
            vec![4; 64],
        ]);
        let scratch = crate::test_support::Scratch::new()?;
        let target = scratch.0.join("valid");
        std::fs::create_dir(&target)?;
        unpack(&mut data.as_slice(), &target, |_| Ok(()))?;
        for (i, name) in BINARIES.iter().enumerate() {
            assert_eq!(
                std::fs::read(target.join(name))?,
                vec![u8::try_from(i)?; 64]
            );
        }
        for (i, bad) in [
            data[..data.len() - 1].to_vec(),
            {
                let mut d = data.clone();
                d.push(1);
                d
            },
            {
                let mut d = data.clone();
                d[56] ^= 1;
                d
            },
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch.0.join(format!("bad-{i}"));
            std::fs::create_dir(&dir)?;
            assert!(unpack(&mut bad.as_slice(), &dir, |_| Ok(())).is_err());
        }
        Ok(())
    }
}
