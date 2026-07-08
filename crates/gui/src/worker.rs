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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{MutexGuard, PoisonError, mpsc};

use gemelli_core::capture::{CaptureError, CaptureSource};
use gemelli_core::publish::{PublishError, TexturePublisher};
use gemelli_core::transform::{self, TransformError};

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Publish(#[from] PublishError),
}

/// Recovers a possibly-poisoned lock instead of propagating the poison:
/// the guarded value is a plain `Option<Frame>`, so a panic elsewhere
/// while holding the lock never leaves it in a state unsafe to read.
fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn run_capture_step(
    source: &mut dyn CaptureSource,
    publisher: &mut dyn TexturePublisher,
    shared: &SharedState,
) -> Result<(), WorkerError> {
    let raw = source.next_frame()?;
    *recover_lock(&shared.latest_raw) = Some(raw.clone());

    let config = shared.transform.load();
    let output = transform::apply(&raw, &config)?;
    publisher.publish(&output)?;

    *recover_lock(&shared.latest_output) = Some(output);
    shared.frames_published.fetch_add(1, Ordering::SeqCst);

    Ok(())
}

/// Loops until `stop`: next_frame -> store raw -> apply(shared.transform
/// snapshot) -> publish -> store output -> frames_published += 1. On
/// error: send it on `errors` and return (the thread ends; the GUI
/// decides whether to respawn).
#[cfg_attr(not(test), allow(dead_code))]
pub fn run_capture(
    source: &mut dyn CaptureSource,
    publisher: &mut dyn TexturePublisher,
    shared: &SharedState,
    stop: &AtomicBool,
    errors: &mpsc::Sender<WorkerError>,
) {
    while !stop.load(Ordering::SeqCst) {
        if let Err(error) = run_capture_step(source, publisher, shared) {
            // If the GUI already dropped its receiver there is nothing
            // left to notify; the thread still ends, which is what
            // matters.
            let _ = errors.send(error);
            return;
        }
    }
}

#[cfg(test)]
mod run_capture_tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    use gemelli_core::capture::{CaptureError, CaptureSource};
    use gemelli_core::frame::Frame;
    use gemelli_core::publish::{PublishError, TexturePublisher};
    use gemelli_core::transform::{self, Rotation, TransformConfig};

    use super::{SharedState, WorkerError, run_capture};

    struct FakeSource {
        frames: VecDeque<Frame>,
    }

    impl FakeSource {
        fn new(frames: Vec<Frame>) -> Self {
            Self { frames: frames.into() }
        }
    }

    impl CaptureSource for FakeSource {
        fn next_frame(&mut self) -> Result<Frame, CaptureError> {
            self.frames
                .pop_front()
                .ok_or_else(|| CaptureError::FrameRead { reason: "exhausted".to_string() })
        }
    }

    /// Records every frame handed to `publish`, then runs `hook` with the
    /// running publish count. Tests use the hook to flip `stop` or swap
    /// `shared.transform` after a chosen number of publishes, instead of
    /// each scenario needing its own publisher type.
    struct CollectingPublisher<F: FnMut(usize)> {
        published: Vec<Frame>,
        hook: F,
    }

    impl<F: FnMut(usize)> CollectingPublisher<F> {
        fn new(hook: F) -> Self {
            Self { published: Vec::new(), hook }
        }
    }

    impl<F: FnMut(usize)> TexturePublisher for CollectingPublisher<F> {
        fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
            self.published.push(frame.clone());
            (self.hook)(self.published.len());
            Ok(())
        }
    }

    // Not used until Cycle 6's publish-error test; remove this `allow` once
    // that test lands and calls it.
    #[allow(dead_code)]
    struct FailingPublisher;

    impl TexturePublisher for FailingPublisher {
        fn publish(&mut self, _frame: &Frame) -> Result<(), PublishError> {
            Err(PublishError::Publish { reason: "sink closed".to_string() })
        }
    }

    fn asymmetric_frame() -> Frame {
        // 2 wide x 3 tall, every pixel a unique BGRA value, row-major —
        // copied from crates/core/src/pipeline.rs's test fixture so a
        // rotation visibly changes both dimensions and pixel order.
        let data = vec![
            10, 20, 30, 255, 40, 50, 60, 255, // row 0
            70, 80, 90, 255, 100, 110, 120, 255, // row 1
            130, 140, 150, 255, 160, 170, 180, 255, // row 2
        ];
        Frame::new(2, 3, data).unwrap()
    }

    #[test]
    fn stores_raw_and_output_frames_and_counts_published() {
        let frame = asymmetric_frame();
        let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
        let expected_output = transform::apply(&frame, &config).unwrap();
        let shared = SharedState::new(config);
        let mut source = FakeSource::new(vec![frame.clone()]);
        let stop = AtomicBool::new(false);
        let mut publisher = CollectingPublisher::new(|n| {
            if n == 1 {
                stop.store(true, Ordering::SeqCst);
            }
        });
        let (tx, rx) = mpsc::channel::<WorkerError>();

        run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

        assert_eq!(*shared.latest_raw.lock().unwrap(), Some(frame));
        assert_eq!(*shared.latest_output.lock().unwrap(), Some(expected_output.clone()));
        assert_eq!(publisher.published, vec![expected_output]);
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), 1);
        assert!(rx.try_recv().is_err(), "no error should have been sent");
    }

    #[test]
    fn config_swap_mid_run_affects_later_output_only() {
        // Same frame content published twice; the second config rotates
        // it 90°, so a changed *shape* (3x2 vs 2x3) proves the swap took
        // effect, independent of any pixel-order subtlety.
        let frame = asymmetric_frame();
        let old_config = TransformConfig::default();
        let new_config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
        let expected_first = transform::apply(&frame, &old_config).unwrap();
        let expected_second = transform::apply(&frame, &new_config).unwrap();
        let shared = SharedState::new(old_config);
        let mut source = FakeSource::new(vec![frame.clone(), frame]);
        let stop = AtomicBool::new(false);
        let mut publisher = CollectingPublisher::new(|n| {
            if n == 1 {
                shared.transform.store(std::sync::Arc::new(new_config.clone()));
            }
            if n == 2 {
                stop.store(true, Ordering::SeqCst);
            }
        });
        let (tx, rx) = mpsc::channel::<WorkerError>();

        run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

        assert_eq!(publisher.published, vec![expected_first, expected_second.clone()]);
        assert_eq!(*shared.latest_output.lock().unwrap(), Some(expected_second));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn stop_flag_ends_loop_with_no_error() {
        let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap();
        let frames = vec![pixel.clone(), pixel.clone(), pixel];
        let mut source = FakeSource::new(frames);
        let shared = SharedState::new(TransformConfig::default());
        let stop = AtomicBool::new(false);
        let mut publisher = CollectingPublisher::new(|n| {
            if n == 2 {
                stop.store(true, Ordering::SeqCst);
            }
        });
        let (tx, rx) = mpsc::channel::<WorkerError>();

        // 3 frames are available but stop_after=2 must end the loop before
        // the 3rd next_frame() call — if run_capture ignored `stop` this
        // would instead exhaust FakeSource and send a Capture error.
        run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

        assert_eq!(publisher.published.len(), 2);
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), 2);
        assert!(rx.try_recv().is_err());
    }
}
