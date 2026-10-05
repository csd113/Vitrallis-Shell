//! Platform/ABI selection and executable installation, independent of GitHub metadata.
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::Installation;

/// Whether the running installation retains a structurally valid previous
/// generation that differs from the active one. The restore action is offered
/// only when this is true; the restore itself revalidates under the update lock.
#[must_use]
pub fn restore_available() -> bool {
    #[cfg(unix)]
    {
        unix::Installation::previous_available()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

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
        let (major_text, minor_text) =
            version.split_once('.').ok_or("Invalid C library version")?;
        let major = major_text
            .parse::<u32>()
            .map_err(|error| format!("Invalid C library version: {error}"))?;
        let minor = minor_text
            .parse::<u32>()
            .map_err(|error| format!("Invalid C library version: {error}"))?;
        if (major, minor) < (2, 36) {
            return Err("Shell releases require glibc 2.36 or newer".into());
        }
        Ok(target)
    }
    pub fn for_triple(requested_triple: &str) -> Result<Self, String> {
        let (triple, class, machine) = match requested_triple {
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
        format!("vitrallis-{}-glibc2.36-v2.vtrbundle", self.triple)
    }
    pub fn verify_header(self, bytes: &[u8]) -> Result<(), String> {
        let invalid = || "Downloaded executable does not match this platform".to_owned();
        let Some([0x7f, b'E', b'L', b'F', binary_class, 1, 1]) = bytes.get(..7) else {
            return Err(invalid());
        };
        let Some([kind_low, kind_high, machine_low, machine_high]) = bytes.get(16..20) else {
            return Err(invalid());
        };
        if bytes.len() < 52
            || *binary_class != self.class
            || !matches!(u16::from_le_bytes([*kind_low, *kind_high]), 2 | 3)
            || u16::from_le_bytes([*machine_low, *machine_high]) != self.machine
        {
            return Err(invalid());
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
