//! Manual verification only — exercises real camera hardware and is
//! excluded from the default `cargo test` run. Run explicitly with:
//!   cargo test -p gemelli-core --test camera_smoke -- --ignored
use gemelli_core::capture::{CaptureSource, NokhwaSource, list_devices};

#[test]
#[ignore = "requires physical camera hardware"]
fn lists_devices_and_grabs_one_frame() {
    let devices = list_devices().expect("at least one camera connected");
    assert!(!devices.is_empty());

    let mut source = NokhwaSource::open(devices[0].index, None).expect("camera opens");
    let frame = source.next_frame().expect("frame captured");

    assert!(frame.width() > 0);
    assert!(frame.height() > 0);
}
