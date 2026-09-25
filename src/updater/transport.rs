//! Optional system curl transport. No shell, curl configuration, or credentials.
use std::{
    io::{self, Read, Write},
    process::{Command, Stdio},
};

pub(super) trait Transport {
    fn fetch(&self, url: &str, limit: u64, output: &mut dyn Write) -> Result<(), String>;
}

pub(super) struct Curl;
impl Transport for Curl {
    fn fetch(&self, url: &str, limit: u64, output: &mut dyn Write) -> Result<(), String> {
        let mut child = Command::new("/usr/bin/curl")
            // -q must be first: an untrusted ~/.curlrc must not change this request.
            .args([
                "-q",
                "--fail",
                "--silent",
                "--location",
                "--max-redirs",
                "5",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "15",
                "--max-time",
                "180",
                "--speed-limit",
                "1024",
                "--speed-time",
                "30",
                "--user-agent",
                concat!("Vitrallis-Shell/", env!("CARGO_PKG_VERSION")),
                "--header",
                "Accept: application/vnd.github+json",
                "--max-filesize",
                &limit.to_string(),
                "--url",
                url,
            ])
            .env_remove("CURL_CA_BUNDLE")
            .env_remove("SSL_CERT_FILE")
            .env_remove("SSL_CERT_DIR")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start /usr/bin/curl: {e}"))?;
        let result = child
            .stdout
            .take()
            .ok_or_else(|| "Download output unavailable".to_owned())
            .and_then(|stdout| copy_bounded(stdout, output, limit));
        if result.is_err() {
            let _ = child.kill();
        }
        let status = child
            .wait()
            .map_err(|e| format!("Download wait failed: {e}"))?;
        result?;
        if !status.success() {
            return Err(match status.code() {
                Some(22) => "GitHub request failed (HTTP error, rate limit or missing release)",
                Some(28) => "Download timed out; check your connection",
                Some(6 | 7) => "Cannot reach GitHub; check your connection",
                Some(60) => "GitHub TLS certificate verification failed",
                Some(63) => "Download exceeds the allowed size",
                _ => "Download failed or was interrupted",
            }
            .into());
        }
        Ok(())
    }
}

fn copy_bounded(input: impl Read, output: &mut dyn Write, limit: u64) -> Result<(), String> {
    let mut input = input.take(limit.saturating_add(1));
    // Do not write even the extra sentinel byte into staging.
    let copied = io::copy(&mut input.by_ref().take(limit), output)
        .map_err(|e| format!("Download read/write failed: {e}"))?;
    let mut extra = [0];
    if copied == limit && input.read(&mut extra).map_err(|e| e.to_string())? != 0 {
        return Err("Download exceeds the allowed size".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_stream_refuses_extra_bytes_and_write_failures() {
        let mut output = Vec::new();
        assert!(copy_bounded(&b"12345"[..], &mut output, 4).is_err());
        assert_eq!(output, b"1234");
        assert!(copy_bounded(&b"12"[..], &mut &mut [0_u8; 1][..], 4).is_err());
    }
    #[test]
    fn maximum_limit_does_not_overflow_the_extra_byte_probe() {
        let mut output = Vec::new();
        assert!(copy_bounded(&b"abc"[..], &mut output, u64::MAX).is_ok());
        assert_eq!(output, b"abc");
    }
}
