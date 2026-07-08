//! Capture-thread worker: owns the camera + publisher lifecycle on a
//! dedicated OS thread, exchanging state with the GUI thread via
//! `SharedState` (latest frames + a live-editable transform config) and an
//! `mpsc` error channel.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use gemelli_core::frame::Frame;
use gemelli_core::transform::TransformConfig;

/// Shared between the GUI thread and the capture thread.
///
/// Not yet wired into `app.rs` (that lands in Task 6), hence
/// `allow(dead_code)` outside `cfg(test)` — same pattern as
/// `theme::contrast_ratio`.
#[cfg_attr(not(test), allow(dead_code))]
pub struct SharedState {
    pub transform: ArcSwap<TransformConfig>,
    pub latest_output: Mutex<Option<Frame>>,
    pub latest_raw: Mutex<Option<Frame>>,
    pub frames_published: AtomicU64,
}

#[cfg_attr(not(test), allow(dead_code))]
impl SharedState {
    pub fn new(config: TransformConfig) -> Self {
        Self {
            transform: ArcSwap::new(Arc::new(config)),
            latest_output: Mutex::new(None),
            latest_raw: Mutex::new(None),
            frames_published: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod shared_state_tests {
    use gemelli_core::transform::{Rotation, TransformConfig};
    use std::sync::atomic::Ordering;

    use super::SharedState;

    #[test]
    fn new_starts_empty_with_the_given_config() {
        let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
        let shared = SharedState::new(config.clone());

        assert_eq!(**shared.transform.load(), config);
        assert_eq!(*shared.latest_output.lock().unwrap(), None);
        assert_eq!(*shared.latest_raw.lock().unwrap(), None);
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
    }
}
