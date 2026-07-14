//! Wires argument parsing, device resolution, capture, transform, and publish into one run.

use std::io::IsTerminal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gemelli_core::capture::{CaptureError, DeviceInfo, NokhwaSource, list_devices};
use gemelli_core::pipeline::{PipelineError, run_pipeline};
use gemelli_core::publish::{PublishError, TexturePublisher};
use gemelli_core::selector::{SelectError, format_devices};

use crate::args::Args;
use crate::select::{choose_device, parse_device};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Publish(#[from] PublishError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Select(#[from] SelectError),
    #[error("no device specified and stdin is not a TTY")]
    NonInteractive,
    #[error("device selection cancelled")]
    SelectionCancelled,
    /// Contract addition (Task 13): SyphonPublisher only exists on macOS.
    /// On a macOS build, `create_publisher`'s `#[cfg(not(target_os = "macos"))]`
    /// arm (the only constructor of this variant) is compiled out entirely, so
    /// dead-code analysis on that target sees no constructor for it.
    // Constructed only on platforms without a native publisher; on macOS
    // (Syphon) and Windows (Spout) the constructing arm is compiled out.
    #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
    #[error("Syphon/Spout publishing is not supported on this platform")]
    UnsupportedPlatform,
    /// Contract addition (Task 13): surfaces a failed Ctrl+C handler install
    /// instead of `unwrap`/`expect`ing it away.
    #[error(transparent)]
    CtrlcSetup(#[from] ctrlc::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceResolution {
    Device(DeviceInfo),
    NeedsPrompt,
}

pub fn resolve_device(
    requested: Option<&str>,
    available: &[DeviceInfo],
    interactive: bool,
) -> Result<DeviceResolution, CliError> {
    let Some(text) = requested else {
        if interactive {
            return Ok(DeviceResolution::NeedsPrompt);
        }
        return Err(CliError::NonInteractive);
    };

    let device = parse_device(text).resolve(available)?;

    Ok(DeviceResolution::Device(device.clone()))
}

#[cfg(target_os = "macos")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(target_os = "windows")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = gemelli_spout::SpoutPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn create_publisher(_server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    Err(CliError::UnsupportedPlatform)
}

pub fn run(args: Args) -> Result<(), CliError> {
    let devices = list_devices()?;

    if args.list_devices {
        println!("{}", format_devices(&devices));
        return Ok(());
    }

    let interactive = std::io::stdin().is_terminal();
    let device = match resolve_device(args.device.as_deref(), &devices, interactive)? {
        DeviceResolution::Device(device) => device,
        DeviceResolution::NeedsPrompt => choose_device(&devices)?,
    };

    let mut publisher = create_publisher(&args.server_name)?;
    let mut source = NokhwaSource::open(&device, args.fps)?;
    let config = args.transform_config();

    let stop = Arc::new(AtomicBool::new(false));
    let handler_stop = Arc::clone(&stop);
    ctrlc::set_handler(move || handler_stop.store(true, Ordering::SeqCst))?;

    run_pipeline(&mut source, &config, publisher.as_mut(), &stop)?;

    Ok(())
}

#[cfg(test)]
mod resolve_device_tests {
    use super::*;

    fn devices() -> Vec<DeviceInfo> {
        vec![
            DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string(), id: None },
            DeviceInfo { index: 1, name: "USB Webcam".to_string(), id: None },
        ]
    }

    #[test]
    fn resolves_requested_query_when_available() {
        let Ok(DeviceResolution::Device(device)) =
            resolve_device(Some("USB Webcam"), &devices(), true)
        else {
            panic!("expected DeviceResolution::Device");
        };
        assert_eq!(device.index, 1);
    }

    #[test]
    fn rejects_requested_index_not_in_device_list() {
        let Err(error) = resolve_device(Some("5"), &devices(), true) else {
            panic!("expected error for out-of-range index");
        };
        assert!(matches!(error, CliError::Select(SelectError::IndexOutOfRange { index: 5, .. })));
    }

    #[test]
    fn passes_through_select_error() {
        let Err(error) = resolve_device(Some("nonexistent"), &devices(), true) else {
            panic!("expected error for unmatched query");
        };
        assert!(matches!(error, CliError::Select(SelectError::NoMatch { .. })));
    }

    #[test]
    fn needs_prompt_when_no_selector_and_interactive() {
        assert!(matches!(
            resolve_device(None, &devices(), true),
            Ok(DeviceResolution::NeedsPrompt)
        ));
    }

    #[test]
    fn errors_when_no_selector_and_not_interactive() {
        assert!(matches!(resolve_device(None, &devices(), false), Err(CliError::NonInteractive)));
    }
}

#[cfg(test)]
mod cli_error_display_tests {
    use super::*;

    #[test]
    fn non_interactive_message() {
        assert_eq!(
            CliError::NonInteractive.to_string(),
            "no device specified and stdin is not a TTY"
        );
    }

    #[test]
    fn selection_cancelled_message() {
        assert_eq!(CliError::SelectionCancelled.to_string(), "device selection cancelled");
    }

    #[test]
    fn unsupported_platform_message() {
        assert_eq!(
            CliError::UnsupportedPlatform.to_string(),
            "Syphon/Spout publishing is not supported on this platform"
        );
    }
}

#[cfg(test)]
mod run_smoke_tests {
    use super::*;

    #[test]
    #[ignore = "requires a real camera; run manually with \
                `cargo test -p gemelli-cli run_smoke_test -- --ignored`, \
                then Ctrl+C after a few seconds and confirm it exits 0"]
    fn run_smoke_test() {
        let args = Args {
            device: Some("0".to_string()),
            list_devices: false,
            rotate: gemelli_core::transform::Rotation::R0,
            flip: None,
            crop: None,
            scale: None,
            server_name: "gemelli-smoke-test".to_string(),
            fps: None,
        };

        let Ok(()) = run(args) else {
            panic!("expected run() to succeed against a real camera + publisher");
        };
    }
}
