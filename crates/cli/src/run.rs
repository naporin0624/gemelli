//! Wires argument parsing, device resolution, capture, transform, and publish into one run.

use std::io::IsTerminal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use webcam_sharedtexture_core::capture::{CaptureError, DeviceInfo, NokhwaSource, list_devices};
use webcam_sharedtexture_core::pipeline::{PipelineError, run_pipeline};
use webcam_sharedtexture_core::publish::{PublishError, TexturePublisher};

use crate::args::Args;
use crate::select::{choose_device, format_devices};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Publish(#[from] PublishError),
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error("no device specified and stdin is not a TTY")]
    NonInteractive,
    #[error("device selection cancelled")]
    SelectionCancelled,
    /// Contract addition (Task 13): SyphonPublisher only exists on macOS.
    /// On a macOS build, `create_publisher`'s `#[cfg(not(target_os = "macos"))]`
    /// arm (the only constructor of this variant) is compiled out entirely, so
    /// dead-code analysis on that target sees no constructor for it.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    #[error("Syphon/Spout publishing is not supported on this platform")]
    UnsupportedPlatform,
    /// Contract addition (Task 13): surfaces a failed Ctrl+C handler install
    /// instead of `unwrap`/`expect`ing it away.
    #[error(transparent)]
    CtrlcSetup(#[from] ctrlc::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceResolution {
    Index(u32),
    NeedsPrompt,
}

pub fn resolve_device(
    requested: Option<u32>,
    available: &[DeviceInfo],
    interactive: bool,
) -> Result<DeviceResolution, CliError> {
    let Some(index) = requested else {
        if interactive {
            return Ok(DeviceResolution::NeedsPrompt);
        }
        return Err(CliError::NonInteractive);
    };

    let is_available = available.iter().any(|device| device.index == index);
    if !is_available {
        return Err(CliError::Capture(CaptureError::DeviceNotFound {
            index,
            available: available.len(),
        }));
    }

    Ok(DeviceResolution::Index(index))
}

#[cfg(target_os = "macos")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = webcam_sharedtexture_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(target_os = "macos"))]
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
    let index = match resolve_device(args.device, &devices, interactive)? {
        DeviceResolution::Index(index) => index,
        DeviceResolution::NeedsPrompt => choose_device(&devices)?,
    };

    let mut source = NokhwaSource::open(index, args.fps)?;
    let config = args.transform_config();
    let mut publisher = create_publisher(&args.server_name)?;

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
            DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() },
            DeviceInfo { index: 1, name: "USB Webcam".to_string() },
        ]
    }

    #[test]
    fn resolves_requested_index_when_available() {
        let Ok(DeviceResolution::Index(index)) = resolve_device(Some(1), &devices(), true) else {
            panic!("expected DeviceResolution::Index(1)");
        };
        assert_eq!(index, 1);
    }

    #[test]
    fn rejects_requested_index_not_in_device_list() {
        let Err(error) = resolve_device(Some(5), &devices(), true) else {
            panic!("expected error for out-of-range index");
        };
        assert!(matches!(
            error,
            CliError::Capture(CaptureError::DeviceNotFound { index: 5, available: 2 })
        ));
    }

    #[test]
    fn needs_prompt_when_no_index_and_interactive() {
        assert!(matches!(
            resolve_device(None, &devices(), true),
            Ok(DeviceResolution::NeedsPrompt)
        ));
    }

    #[test]
    fn errors_when_no_index_and_not_interactive() {
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
                `cargo test -p webcam-sharedtexture-cli run_smoke_test -- --ignored`, \
                then Ctrl+C after a few seconds and confirm it exits 0"]
    fn run_smoke_test() {
        let args = Args {
            device: Some(0),
            list_devices: false,
            rotate: webcam_sharedtexture_core::transform::Rotation::R0,
            flip: None,
            crop: None,
            scale: None,
            server_name: "webcam-sharedtexture-smoke-test".to_string(),
            fps: None,
        };

        let Ok(()) = run(args) else {
            panic!("expected run() to succeed against a real camera + publisher");
        };
    }
}
