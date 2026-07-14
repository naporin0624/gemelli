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
/// `latest_output`/`latest_raw` hold `Arc<Frame>` rather than `Frame`: the GUI thread clones
/// them out of the mutex on every repaint (`GemelliApp::refresh_preview`, up to the display's
/// refresh rate), and a `Frame` clone deep-copies its pixel buffer. Wrapping in `Arc` makes that
/// clone a refcount bump instead, and shrinks how long the capture thread holds each lock too
/// (`run_capture_step` stores the same `Arc` it already built for the other field/publisher,
/// rather than deep-cloning the frame again per store).
pub struct SharedState {
    pub transform: ArcSwap<TransformConfig>,
    pub latest_output: Mutex<Option<Arc<Frame>>>,
    pub latest_raw: Mutex<Option<Arc<Frame>>>,
    pub frames_published: AtomicU64,
}

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
    let raw = Arc::new(source.next_frame()?);
    *recover_lock(&shared.latest_raw) = Some(Arc::clone(&raw));

    let config = shared.transform.load();
    let output = Arc::new(transform::apply(&raw, &config)?);
    publisher.publish(&output)?;

    *recover_lock(&shared.latest_output) = Some(Arc::clone(&output));
    shared.frames_published.fetch_add(1, Ordering::SeqCst);

    Ok(())
}

/// Loops until `stop`: next_frame -> store raw -> apply(shared.transform
/// snapshot) -> publish -> store output -> frames_published += 1. On
/// error: send it on `errors` and return (the thread ends; the GUI
/// decides whether to respawn).
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

/// Owns the capture thread. Dropping (or calling `stop`) sets the stop
/// flag and joins.
pub struct WorkerHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WorkerHandle {
    /// Idempotent: safe to call more than once (`Drop` calls it too).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join.take() {
            // A panicked worker thread has nothing further this handle
            // can do about it beyond having already requested `stop`.
            let _ = handle.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.join.as_ref().is_some_and(|handle| !handle.is_finished())
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "macos")]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(target_os = "windows")]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = gemelli_spout::SpoutPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    Err(PublishError::ServerCreate {
        name: server_name.to_string(),
        reason: "Syphon/Spout publishing is not supported on this platform".to_string(),
    })
}

/// Parameters for one capture-thread run. Changing device, fps, or server
/// name needs a fresh `NokhwaSource`/publisher, so the GUI stops the old
/// worker and calls `spawn_worker` again with a new spec rather than
/// mutating a running one.
pub struct WorkerSpec {
    pub device: gemelli_core::capture::DeviceInfo,
    pub requested_fps: Option<u32>,
    pub server_name: String,
}

/// Opens `NokhwaSource` and the publisher on the new thread; open
/// failures are reported the same way as any other capture-loop error —
/// sent on `errors`, thread ends without ever calling `run_capture`.
pub fn spawn_worker(
    spec: WorkerSpec,
    shared: Arc<SharedState>,
    errors: mpsc::Sender<WorkerError>,
) -> WorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);

    let join = std::thread::spawn(move || {
        let mut source =
            match gemelli_core::capture::NokhwaSource::open(&spec.device, spec.requested_fps) {
                Ok(source) => source,
                Err(error) => {
                    let _ = errors.send(WorkerError::Capture(error));
                    return;
                }
            };

        let mut publisher = match open_publisher(&spec.server_name) {
            Ok(publisher) => publisher,
            Err(error) => {
                let _ = errors.send(WorkerError::Publish(error));
                return;
            }
        };

        run_capture(&mut source, publisher.as_mut(), &shared, &thread_stop, &errors);
    });

    WorkerHandle { stop, join: Some(join) }
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

        assert_eq!(shared.latest_raw.lock().unwrap().as_deref(), Some(&frame));
        assert_eq!(shared.latest_output.lock().unwrap().as_deref(), Some(&expected_output));
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
        assert_eq!(shared.latest_output.lock().unwrap().as_deref(), Some(&expected_second));
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

    #[test]
    fn capture_error_is_sent_and_loop_returns() {
        let mut source = FakeSource::new(vec![]); // next_frame() errors immediately
        let shared = SharedState::new(TransformConfig::default());
        let stop = AtomicBool::new(false);
        let mut publisher = CollectingPublisher::new(|_| {});
        let (tx, rx) = mpsc::channel::<WorkerError>();

        run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

        let error = rx.try_recv().expect("an error must have been sent");
        assert!(matches!(error, WorkerError::Capture(CaptureError::FrameRead { .. })));
        assert_eq!(publisher.published.len(), 0);
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn publish_error_is_sent_and_output_is_not_overwritten() {
        let frame = Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap();
        let mut source = FakeSource::new(vec![frame.clone()]);
        let shared = SharedState::new(TransformConfig::default());
        let stop = AtomicBool::new(false);
        let mut publisher = FailingPublisher;
        let (tx, rx) = mpsc::channel::<WorkerError>();

        run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

        let error = rx.try_recv().expect("an error must have been sent");
        assert!(matches!(error, WorkerError::Publish(PublishError::Publish { .. })));
        // Raw is stored before publish is attempted; output is only
        // stored *after* a successful publish — proves the step order
        // documented on run_capture (store raw -> apply -> publish ->
        // store output).
        assert_eq!(shared.latest_raw.lock().unwrap().as_deref(), Some(&frame));
        assert_eq!(*shared.latest_output.lock().unwrap(), None);
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
mod handle_tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Duration;

    use gemelli_core::capture::{CaptureError, CaptureSource};
    use gemelli_core::frame::Frame;
    use gemelli_core::publish::{PublishError, TexturePublisher};
    use gemelli_core::transform::TransformConfig;

    use super::{SharedState, WorkerError, WorkerHandle, run_capture};

    /// Always returns the same 1x1 frame — lets a test run a real
    /// `run_capture` thread that only stops when told to, with no bound
    /// on frame count.
    struct InfiniteSource {
        frame: Frame,
    }

    impl CaptureSource for InfiniteSource {
        fn next_frame(&mut self) -> Result<Frame, CaptureError> {
            Ok(self.frame.clone())
        }
    }

    struct NullPublisher;

    impl TexturePublisher for NullPublisher {
        fn publish(&mut self, _frame: &Frame) -> Result<(), PublishError> {
            Ok(())
        }
    }

    fn spawn_fake_worker(
        shared: Arc<SharedState>,
        errors: mpsc::Sender<WorkerError>,
    ) -> WorkerHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let join = thread::spawn(move || {
            let mut source =
                InfiniteSource { frame: Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap() };
            let mut publisher = NullPublisher;
            run_capture(&mut source, &mut publisher, &shared, &thread_stop, &errors);
        });
        WorkerHandle { stop, join: Some(join) }
    }

    /// Busy-waits for the fake worker to have processed at least one
    /// frame — a deterministic readiness signal instead of a blind sleep.
    fn wait_for_first_frame(shared: &SharedState) {
        while shared.frames_published.load(Ordering::SeqCst) == 0 {
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn is_running_reflects_thread_lifecycle() {
        let shared = Arc::new(SharedState::new(TransformConfig::default()));
        let (tx, _rx) = mpsc::channel();
        let mut handle = spawn_fake_worker(Arc::clone(&shared), tx);

        wait_for_first_frame(&shared);
        assert!(handle.is_running());

        handle.stop(); // blocks until the thread actually joins

        assert!(!handle.is_running());
    }

    #[test]
    fn stop_is_idempotent() {
        let shared = Arc::new(SharedState::new(TransformConfig::default()));
        let (tx, _rx) = mpsc::channel();
        let mut handle = spawn_fake_worker(shared, tx);

        handle.stop();
        handle.stop(); // must not panic or block forever

        assert!(!handle.is_running());
    }

    #[test]
    fn drop_stops_the_worker_thread() {
        let shared = Arc::new(SharedState::new(TransformConfig::default()));
        let (tx, _rx) = mpsc::channel();
        let handle = spawn_fake_worker(Arc::clone(&shared), tx);

        wait_for_first_frame(&shared);
        drop(handle);

        // Drop's stop() joins before returning, so the thread is already
        // dead by this point — no polling needed for this assertion to
        // be non-flaky.
        let count_at_drop = shared.frames_published.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(20));
        assert_eq!(shared.frames_published.load(Ordering::SeqCst), count_at_drop);
    }
}

#[cfg(test)]
mod spawn_worker_tests {
    use std::sync::Arc;
    use std::sync::mpsc;

    use gemelli_core::capture::DeviceInfo;
    use gemelli_core::selector::DeviceId;
    use gemelli_core::transform::TransformConfig;

    use super::{SharedState, WorkerSpec, spawn_worker};

    fn fake_device() -> DeviceInfo {
        DeviceInfo {
            index: 9999,
            name: "nonexistent test device".to_string(),
            id: DeviceId::new("gemelli-test-bogus-id"),
        }
    }

    #[test]
    fn worker_spec_holds_the_given_fields() {
        let spec = WorkerSpec {
            device: fake_device(),
            requested_fps: Some(30),
            server_name: "gemelli".to_string(),
        };

        assert_eq!(spec.device, fake_device());
        assert_eq!(spec.requested_fps, Some(30));
        assert_eq!(spec.server_name, "gemelli");
    }

    #[test]
    #[ignore = "opens a real camera device (a fabricated device id is out of \
                range on every real machine, but nokhwa still touches the OS \
                camera subsystem to discover that, which is flaky/slow in \
                CI); run manually with `cargo test -p gemelli-gui \
                spawn_worker_open_failure -- --ignored`"]
    fn spawn_worker_open_failure() {
        let shared = Arc::new(SharedState::new(TransformConfig::default()));
        let (tx, rx) = mpsc::channel();
        let spec = WorkerSpec {
            device: fake_device(),
            requested_fps: None,
            server_name: "gemelli-test".to_string(),
        };

        let mut handle = spawn_worker(spec, shared, tx);
        let error = rx.recv().expect("open failure must be sent on the errors channel");
        assert!(matches!(error, super::WorkerError::Capture(_)));

        handle.stop();
        assert!(!handle.is_running());
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "requires a real camera and a real macOS GPU/Syphon session; \
                run manually with `cargo test -p gemelli-gui \
                spawn_worker_publishes_real_frames -- --ignored`, then \
                check a Syphon client (e.g. Syphon Recorder) sees \
                \"gemelli-worker-smoke\" publishing"]
    fn spawn_worker_publishes_real_frames() {
        use std::sync::atomic::Ordering;
        use std::thread;
        use std::time::Duration;

        let shared = Arc::new(SharedState::new(TransformConfig::default()));
        let (tx, _rx) = mpsc::channel();
        let spec = WorkerSpec {
            device: DeviceInfo { index: 0, name: "default camera".to_string(), id: None },
            requested_fps: None,
            server_name: "gemelli-worker-smoke".to_string(),
        };

        let mut handle = spawn_worker(spec, Arc::clone(&shared), tx);
        thread::sleep(Duration::from_secs(3));

        assert!(shared.frames_published.load(Ordering::SeqCst) > 0);
        assert!(shared.latest_output.lock().unwrap().is_some());

        handle.stop();
        assert!(!handle.is_running());
    }
}
