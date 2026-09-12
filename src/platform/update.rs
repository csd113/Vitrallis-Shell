//! Platform/ABI selection and executable installation, independent of GitHub metadata.
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::Installation;

/// The validated installation path survives Linux's `/proc/self/exe` deletion suffix.
#[derive(Debug)]
pub struct Relaunch {
    pub executable: std::path::PathBuf,
    pub sha256: [u8; 32],
}
impl Relaunch {
    pub fn execute(&self) -> Result<(), String> {
        #[cfg(unix)]
        {
            unix::relaunch(self, std::env::args_os().skip(1))
        }
        #[cfg(not(unix))]
        {
            Err("Shell relaunch is unsupported on this operating system".into())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Target {
    triple: &'static str,
    class: u8,
    machine: u16,
}
impl Target {
    pub fn current() -> Result<Self, String> {
        let target = Self::for_triple(env!("VITRALLIS_BUILD_TARGET"))?;
        let libc = super::command::run("/usr/bin/getconf", &["GNU_LIBC_VERSION"])?;
        let version = libc
            .trim()
            .strip_prefix("glibc ")
            .ok_or("Unsupported C library")?;
        let (major, minor) = version.split_once('.').ok_or("Invalid C library version")?;
        let major = major
            .parse::<u32>()
            .map_err(|_| "Invalid C library version")?;
        let minor = minor
            .parse::<u32>()
            .map_err(|_| "Invalid C library version")?;
        if (major, minor) < (2, 36) {
            return Err("Shell releases require glibc 2.36 or newer".into());
        }
        Ok(target)
    }
    pub fn for_triple(triple: &str) -> Result<Self, String> {
        let (triple, class, machine) = match triple {
            "x86_64-unknown-linux-gnu" => ("x86_64-unknown-linux-gnu", 2, 62),
            "aarch64-unknown-linux-gnu" => ("aarch64-unknown-linux-gnu", 2, 183),
            "armv7-unknown-linux-gnueabihf" => ("armv7-unknown-linux-gnueabihf", 1, 40),
            _ => return Err("Self-update is unsupported on this platform/architecture".into()),
        };
        Ok(Self {
            triple,
            class,
            machine,
        })
    }
    pub fn artifact(self) -> String {
        format!("vitrallis-{}-glibc2.36.vtrbundle", self.triple)
    }
    pub fn verify_header(self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < 52
            || &bytes[..4] != b"\x7fELF"
            || bytes[4] != self.class
            || bytes[5] != 1
            || bytes[6] != 1
            || !matches!(u16::from_le_bytes([bytes[16], bytes[17]]), 2 | 3)
            || u16::from_le_bytes([bytes[18], bytes[19]]) != self.machine
        {
            return Err("Downloaded executable does not match this platform".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platforms_require_an_exact_supported_abi() {
        for triple in [
            "x86_64-apple-darwin",
            "arm-unknown-linux-gnueabi",
            "x86_64-unknown-linux-musl",
            "unknown",
        ] {
            assert!(Target::for_triple(triple).is_err());
        }
        for triple in [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "armv7-unknown-linux-gnueabihf",
        ] {
            assert!(Target::for_triple(triple).is_ok());
        }
    }
}
