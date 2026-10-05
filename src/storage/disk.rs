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
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "A u64 byte count times 100 fits in u128, and total.max(1) makes the divisor nonzero"
    )]
    pub fn percent(&self) -> u8 {
        u8::try_from(u128::from(self.used) * 100 / u128::from(self.total.max(1)))
            .unwrap_or(100)
            .min(100)
    }
}

pub fn query(path: &Path) -> Result<Disk, String> {
    let path_text = path.to_str().ok_or("Filesystem path is not UTF-8")?;
    if !path.is_absolute() {
        return Err("Filesystem path must be absolute".into());
    }
    parse(&crate::platform::command::run(
        "/bin/df",
        &["-kP", path_text],
    )?)
}

pub(super) fn parse(output: &str) -> Result<Disk, String> {
    // POSIX -P produces one record per filesystem; mount names may contain spaces.
    let row = output
        .lines()
        .nth(1)
        .ok_or("Filesystem usage unavailable")?;
    let fields: Vec<_> = row.split_whitespace().collect();
    let [
        filesystem,
        total_field,
        used_field,
        available_field,
        _percent,
        mount @ ..,
    ] = fields.as_slice()
    else {
        return Err("Unrecognized filesystem usage".into());
    };
    if mount.is_empty() {
        return Err("Missing filesystem mount".into());
    }
    let bytes = |field: &str| {
        field
            .parse::<u64>()
            .map_err(|error| format!("Invalid filesystem capacity {field}: {error}"))?
            .checked_mul(1024)
            .ok_or_else(|| "Invalid filesystem capacity".to_owned())
    };
    let total = bytes(total_field)?;
    let used = bytes(used_field)?;
    // Some filesystems report negative available space when reserved blocks are used.
    let available = if available_field.starts_with('-') {
        let _negative_available = available_field.parse::<i64>().map_err(|e| e.to_string())?;
        0
    } else {
        bytes(available_field)?
    };
    if total == 0 || used > total || available > total {
        return Err("Inconsistent filesystem capacity".into());
    }
    Ok(Disk {
        filesystem: (*filesystem).into(),
        mount: mount.join(" "),
        total,
        used,
        available,
    })
}

/// Decimal units match the labels and avoid floating-point precision loss.
#[allow(
    clippy::arithmetic_side_effects,
    reason = "Widening u64 to u128 leaves room for multiplication by ten and rounding; the selected divisor is one of 1000, 1000000 or 1000000000"
)]
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
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "Both u64 byte counts are widened to u128 before multiplying by at most 255, so neither product can overflow"
    )]
    pub fn critical(self, disk: &Disk) -> bool {
        disk.available <= self.bytes
            || u128::from(disk.available) * 100 <= u128::from(disk.total) * u128::from(self.percent)
    }
}
