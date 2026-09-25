//! Small terminal utilities shared by the interactive session.

use anyhow::{bail, Result};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};

/// Enables raw mode and restores the previous state when dropped, including on panic unwind.
pub struct RawModeGuard {
    was_raw: bool,
}

impl RawModeGuard {
    pub fn enable() -> Result<Self> {
        let was_raw = is_raw_mode_enabled().unwrap_or(false);
        enable_raw_mode()?;
        Ok(Self { was_raw })
    }

    /// Run `f` with the terminal back in normal (cooked) mode, e.g. for an editor or a prompt.
    pub fn suspend<T>(&self, f: impl FnOnce() -> T) -> T {
        let _ = disable_raw_mode();
        let result = f();
        let _ = enable_raw_mode();
        result
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if !self.was_raw {
            let _ = disable_raw_mode();
        }
    }
}

/// Raw mode doesn't translate `\n`; convert so multi-line text doesn't staircase.
pub fn to_crlf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut prev = '\0';
    for c in text.chars() {
        if c == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(c);
        prev = c;
    }
    out
}

/// Parse a palette key like `ctrl-t`, `ctrl-]`, or `ctrl-space` into the byte the terminal sends.
pub fn parse_key_spec(spec: &str) -> Result<u8> {
    let spec = spec.trim().to_ascii_lowercase();
    let Some(key) = spec
        .strip_prefix("ctrl-")
        .or_else(|| spec.strip_prefix("ctrl+"))
    else {
        bail!("palette key must be ctrl-<key> (for example ctrl-t), got `{spec}`");
    };
    let byte = match key {
        "space" | "@" => 0x00,
        "[" => bail!("ctrl-[ is Escape; pick another palette key"),
        "\\" => 0x1c,
        "]" => 0x1d,
        "^" => 0x1e,
        "_" => 0x1f,
        k if k.len() == 1 && k.as_bytes()[0].is_ascii_lowercase() => {
            let b = k.as_bytes()[0] - b'a' + 1;
            // These are Tab, Enter, Backspace and friends; stealing them breaks the shell.
            if matches!(b, 0x03 | 0x04 | 0x08 | 0x09 | 0x0a | 0x0d) {
                bail!("ctrl-{k} is needed by the shell; pick another palette key");
            }
            b
        }
        _ => bail!("unsupported palette key `{spec}`"),
    };
    Ok(byte)
}

/// Human label for a palette key byte, e.g. `Ctrl+T`.
pub fn key_label(byte: u8) -> String {
    match byte {
        0x00 => "Ctrl+Space".into(),
        0x01..=0x1a => format!("Ctrl+{}", (b'A' + byte - 1) as char),
        0x1c => "Ctrl+\\".into(),
        0x1d => "Ctrl+]".into(),
        0x1e => "Ctrl+^".into(),
        0x1f => "Ctrl+_".into(),
        _ => format!("0x{byte:02x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_conversion_is_idempotent() {
        assert_eq!(to_crlf("a\nb\r\nc"), "a\r\nb\r\nc");
        assert_eq!(to_crlf(&to_crlf("x\ny")), "x\r\ny");
    }

    #[test]
    fn key_specs() {
        assert_eq!(parse_key_spec("ctrl-t").unwrap(), 0x14);
        assert_eq!(parse_key_spec("Ctrl+G").unwrap(), 0x07);
        assert_eq!(parse_key_spec("ctrl-]").unwrap(), 0x1d);
        assert!(parse_key_spec("ctrl-c").is_err());
        assert!(parse_key_spec("ctrl-m").is_err());
        assert!(parse_key_spec("alt-t").is_err());
        assert_eq!(key_label(0x14), "Ctrl+T");
    }
}
