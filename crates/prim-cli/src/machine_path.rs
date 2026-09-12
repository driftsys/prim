//! Stable path projection for machine-readable output.

use std::path::Path;

/// Render the path for a human reader. A non-UTF-8 name is lossy; [`encoded`]
/// carries the exact bytes alongside it where the platform exposes them.
pub(crate) fn display(path: &Path) -> String {
    path.display().to_string()
}

/// Percent-encode the path bytes when the path is not valid UTF-8.
pub(crate) fn encoded(path: &Path) -> Option<String> {
    if path.to_str().is_some() {
        return None;
    }

    Some(percent_encode(path_bytes(path)?))
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Option<&[u8]> {
    use std::os::unix::ffi::OsStrExt;

    Some(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn path_bytes(_path: &Path) -> Option<&[u8]> {
    None
}

fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}
