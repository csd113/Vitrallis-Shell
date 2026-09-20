//! Filesystem capacity queried with the existing bounded command runner.
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub filesystem: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
}
impl Disk {
    pub fn percent(&self) -> u8 {
        u8::try_from(u128::from(self.used) * 100 / u128::from(self.total.max(1)))
            .unwrap_or(100)
            .min(100)
    }
}

pub fn query(path: &Path) -> Result<Disk, String> {
    let path = path.to_str().ok_or("Filesystem path is not UTF-8")?;
    if !Path::new(path).is_absolute() {
        return Err("Filesystem path must be absolute".into());
    }
    parse(&crate::platform::command::run("/bin/df", &["-kP", path])?)
}

pub(super) fn parse(output: &str) -> Result<Disk, String> {
    // POSIX -P produces one record per filesystem; mount names may contain spaces.
    let row = output
        .lines()
        .nth(1)
        .ok_or("Filesystem usage unavailable")?;
    let fields: Vec<_> = row.split_whitespace().collect();
    if fields.len() < 6 {
        return Err("Unrecognized filesystem usage".into());
    }
    let bytes = |index: usize| {
        fields[index]
            .parse::<u64>()
            .ok()
            .and_then(|value| value.checked_mul(1024))
            .ok_or_else(|| "Invalid filesystem capacity".to_owned())
    };
    let total = bytes(1)?;
    let used = bytes(2)?;
    // Some filesystems report negative available space when reserved blocks are used.
    let available = if fields[3].starts_with('-') {
        fields[3].parse::<i64>().map_err(|e| e.to_string())?;
        0
    } else {
        bytes(3)?
    };
    if total == 0 || used > total || available > total {
        return Err("Inconsistent filesystem capacity".into());
    }
    Ok(Disk {
        filesystem: fields[0].into(),
        mount: fields[5..].join(" "),
        total,
        used,
        available,
    })
}

/// Decimal units match the labels and avoid floating-point precision loss.
pub fn format_bytes(bytes: u64) -> String {
    let (unit, divisor) = if bytes >= 1_000_000_000 {
        ("GB", 1_000_000_000)
    } else if bytes >= 1_000_000 {
        ("MB", 1_000_000)
    } else if bytes >= 1_000 {
        ("KB", 1_000)
    } else {
        return format!("{bytes} B");
    };
    let tenths = (u128::from(bytes) * 10 + divisor / 2) / divisor;
    format!("{}.{:01} {unit}", tenths / 10, tenths % 10)
}

#[derive(Debug, Clone, Copy)]
pub struct LowSpace {
    pub bytes: u64,
    pub percent: u8,
}
impl Default for LowSpace {
    fn default() -> Self {
        Self {
            bytes: 100_000_000,
            percent: 5,
        }
    }
}
impl LowSpace {
    pub fn critical(self, disk: &Disk) -> bool {
        disk.available <= self.bytes
            || u128::from(disk.available) * 100 <= u128::from(disk.total) * u128::from(self.percent)
    }
}
