//! Device listing output and interactive device selection.
//!
//! `parse_selection` and `format_devices` are pure and fully unit-tested; `choose_device`
//! is a thin stdin/stdout loop around `parse_selection` and is exercised manually (see
//! Task 13's ignored smoke test and Task 14's E2E checklist) — piping fake stdin into a
//! blocking `read_line` loop under `cargo test` is not worth the added indirection here.

use std::io::Write;

use webcam_sharedtexture_core::capture::DeviceInfo;

use crate::run::CliError;

pub fn format_devices(devices: &[DeviceInfo]) -> String {
    devices
        .iter()
        .map(|device| format!("{}: {}", device.index, device.name))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_selection(input: &str, device_count: usize) -> Result<u32, String> {
    let trimmed = input.trim();

    let Ok(index) = trimmed.parse::<u32>() else {
        return Err(format!("\"{trimmed}\" is not a valid device number"));
    };

    let Ok(count) = u32::try_from(device_count) else {
        return Err("no devices available".to_string());
    };

    if index >= count {
        return Err(format!("{index} is out of range (0..{count})"));
    }

    Ok(index)
}

pub fn choose_device(devices: &[DeviceInfo]) -> Result<u32, CliError> {
    println!("{}", format_devices(devices));

    loop {
        print!("select device index: ");
        std::io::stdout().flush().map_err(|_| CliError::SelectionCancelled)?;

        let mut line = String::new();
        let bytes_read =
            std::io::stdin().read_line(&mut line).map_err(|_| CliError::SelectionCancelled)?;
        if bytes_read == 0 {
            return Err(CliError::SelectionCancelled);
        }

        match parse_selection(&line, devices.len()) {
            Ok(index) => return Ok(index),
            Err(message) => println!("{message}, try again"),
        }
    }
}

#[cfg(test)]
mod format_devices_tests {
    use super::*;

    #[test]
    fn formats_empty_list() {
        assert_eq!(format_devices(&[]), "");
    }

    #[test]
    fn formats_single_device() {
        let devices = [DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() }];
        assert_eq!(format_devices(&devices), "0: FaceTime HD Camera");
    }

    #[test]
    fn formats_multiple_devices_joined_by_newline() {
        let devices = [
            DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() },
            DeviceInfo { index: 1, name: "USB Webcam".to_string() },
        ];
        assert_eq!(format_devices(&devices), "0: FaceTime HD Camera\n1: USB Webcam");
    }
}

#[cfg(test)]
mod parse_selection_tests {
    use super::*;

    #[test]
    fn accepts_valid_index() {
        let cases = [("0", 0), ("2", 2)];
        for (input, expected) in cases {
            assert_eq!(parse_selection(input, 3), Ok(expected), "input: {input}");
        }
    }

    #[test]
    fn trims_surrounding_whitespace_and_newline() {
        assert_eq!(parse_selection(" 1 \n", 3), Ok(1));
    }

    #[test]
    fn rejects_out_of_range_index() {
        assert!(parse_selection("3", 3).is_err());
    }

    #[test]
    fn rejects_non_numeric_input() {
        for input in ["abc", "-1", ""] {
            assert!(parse_selection(input, 3).is_err(), "expected \"{input}\" to be rejected");
        }
    }
}
