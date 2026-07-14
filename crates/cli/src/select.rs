//! Device listing output and interactive device selection.
//!
//! `parse_device` is pure and fully unit-tested; `choose_device` is a thin stdin/stdout
//! loop around `parse_device` + `DeviceSelector::resolve` and is exercised manually (see
//! Task 13's ignored smoke test and Task 14's E2E checklist) — piping fake stdin into a
//! blocking `read_line` loop under `cargo test` is not worth the added indirection here.

use std::io::Write;

use gemelli_core::capture::DeviceInfo;
use gemelli_core::selector::{DeviceSelector, format_devices};

use crate::run::CliError;

/// Interprets interactive/positional CLI input as an index or a name/id query.
/// All-ASCII-digit input that parses as `u32` is `Index`; everything else
/// (including a pasted UUID, which is not all-digits) is a `Query` and flows
/// through `DeviceSelector::resolve`'s exact-id tier.
pub fn parse_device(input: &str) -> DeviceSelector {
    let trimmed = input.trim();

    if !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_digit())
        && let Ok(index) = trimmed.parse::<u32>()
    {
        return DeviceSelector::Index(index);
    }

    DeviceSelector::Query(trimmed.to_string())
}

pub fn choose_device(devices: &[DeviceInfo]) -> Result<DeviceInfo, CliError> {
    println!("{}", format_devices(devices));

    loop {
        print!("select device (index or name): ");
        std::io::stdout().flush().map_err(|_| CliError::SelectionCancelled)?;

        let mut line = String::new();
        let bytes_read =
            std::io::stdin().read_line(&mut line).map_err(|_| CliError::SelectionCancelled)?;
        if bytes_read == 0 {
            return Err(CliError::SelectionCancelled);
        }

        match parse_device(&line).resolve(devices) {
            Ok(device) => return Ok(device.clone()),
            Err(error) => println!("{error}, try again"),
        }
    }
}

#[cfg(test)]
mod parse_device_tests {
    use super::*;

    #[test]
    fn digit_input_is_index() {
        let cases = [("0", 0), (" 7 ", 7), ("007", 7)];
        for (input, expected) in cases {
            assert_eq!(parse_device(input), DeviceSelector::Index(expected), "input: {input}");
        }
    }

    #[test]
    fn text_input_is_query() {
        assert_eq!(parse_device("OBS"), DeviceSelector::Query("OBS".to_string()));
        assert_eq!(
            parse_device("1080P USB Camera"),
            DeviceSelector::Query("1080P USB Camera".to_string())
        );
    }

    #[test]
    fn uuid_input_is_query() {
        let uuid = "7626645E-1E13-4E6F-8B77-71B2A5B5F1C7";
        assert_eq!(parse_device(uuid), DeviceSelector::Query(uuid.to_string()));
    }

    #[test]
    fn mixed_digit_text_is_query() {
        assert_eq!(parse_device("0abc"), DeviceSelector::Query("0abc".to_string()));
    }

    #[test]
    fn empty_input_is_query() {
        assert_eq!(parse_device("  "), DeviceSelector::Query(String::new()));
    }
}
