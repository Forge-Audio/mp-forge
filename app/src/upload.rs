//! A file the screen hands to the app: its bytes are the request body, and its names ride along as labels.

use tauri::ipc::{InvokeBody, Request};

const PLUGIN_NAME_LABEL: &str = "plugin-name";
const FILE_NAME_LABEL: &str = "file-name";
const NOTHING_ARRIVED: &str = "No file arrived.";

pub struct UploadedFile<'a> {
    pub plugin_name: String,
    pub file_name: String,
    pub bytes: &'a [u8],
}

impl<'a> UploadedFile<'a> {
    pub fn read_from(request: &'a Request<'a>) -> Result<Self, String> {
        let InvokeBody::Raw(bytes) = request.body() else { return Err(NOTHING_ARRIVED.to_string()) };
        Ok(Self { plugin_name: label(request, PLUGIN_NAME_LABEL)?, file_name: label(request, FILE_NAME_LABEL)?, bytes })
    }
}

fn label(request: &Request, name: &str) -> Result<String, String> {
    let written = request.headers().get(name).and_then(|value| value.to_str().ok());
    written.and_then(percent_decoded).ok_or_else(|| NOTHING_ARRIVED.to_string())
}

/// Labels travel percent-encoded, the way a web address carries any character.
fn percent_decoded(written: &str) -> Option<String> {
    let mut decoded = Vec::new();
    let mut rest = written.as_bytes();
    while let Some((&first, after)) = rest.split_first() {
        if first == b'%' {
            let code = std::str::from_utf8(after.get(..2)?).ok()?;
            decoded.push(u8::from_str_radix(code, 16).ok()?);
            rest = &after[2..];
        } else {
            decoded.push(first);
            rest = after;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_encoded_label_reads_back_as_the_name_that_was_sent() {
        assert_eq!(percent_decoded("Dusty%20Tape").unwrap(), "Dusty Tape");
        assert_eq!(percent_decoded("caf%C3%A9%20(1).wav").unwrap(), "café (1).wav");
    }

    #[test]
    fn a_broken_label_is_refused() {
        assert!(percent_decoded("kick%2").is_none());
        assert!(percent_decoded("kick%zz.wav").is_none());
        assert!(percent_decoded("%ff").is_none());
    }
}
