use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;

use crate::capture::{CaptureError, CaptureSource};
use crate::publish::{PublishError, TexturePublisher};
use crate::transform::{self, TransformConfig, TransformError};

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Publish(#[from] PublishError),
}

/// Loops: next_frame → apply(config) → publish, until `stop` is true
/// (checked at the top of every iteration). Any step's error aborts the
/// loop and propagates.
pub fn run_pipeline(
    source: &mut dyn CaptureSource,
    config: &TransformConfig,
    publisher: &mut dyn TexturePublisher,
    stop: &AtomicBool,
) -> Result<(), PipelineError> {
    while !stop.load(Ordering::SeqCst) {
        let frame = source.next_frame()?;
        let transformed = transform::apply(&frame, config)?;
        publisher.publish(&transformed)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{PipelineError, run_pipeline};
    use crate::capture::{CaptureError, CaptureSource};
    use crate::frame::Frame;
    use crate::publish::{PublishError, TexturePublisher};
    use crate::transform::{self, Rotation, TransformConfig};

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

    struct CollectingPublisher {
        published: Vec<Frame>,
        stop_after: Option<usize>,
        stop: Arc<AtomicBool>,
    }

    impl CollectingPublisher {
        fn new(stop_after: Option<usize>, stop: Arc<AtomicBool>) -> Self {
            Self { published: Vec::new(), stop_after, stop }
        }
    }

    impl TexturePublisher for CollectingPublisher {
        fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
            self.published.push(frame.clone());
            if self.stop_after == Some(self.published.len()) {
                self.stop.store(true, Ordering::SeqCst);
            }

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
        // 2 wide x 3 tall, every pixel a unique BGRA value, row-major.
        let data = vec![
            10, 20, 30, 255, 40, 50, 60, 255, // row 0
            70, 80, 90, 255, 100, 110, 120, 255, // row 1
            130, 140, 150, 255, 160, 170, 180, 255, // row 2
        ];

        Frame::new(2, 3, data).expect("valid frame")
    }

    #[test]
    fn applies_transform_before_publish() {
        let frame = asymmetric_frame();
        let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
        let expected = transform::apply(&frame, &config).expect("apply succeeds");
        let mut source = FakeSource::new(vec![frame]);
        let stop = Arc::new(AtomicBool::new(false));
        let mut publisher = CollectingPublisher::new(Some(1), Arc::clone(&stop));

        let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

        assert!(result.is_ok());
        assert_eq!(publisher.published.len(), 1);
        assert_eq!(publisher.published[0], expected);
        assert_eq!(publisher.published[0].width(), 3);
        assert_eq!(publisher.published[0].height(), 2);
    }

    #[test]
    fn stop_flag_ends_loop_with_ok() {
        let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).expect("valid frame");
        let frames = vec![pixel.clone(), pixel.clone(), pixel];
        let mut source = FakeSource::new(frames);
        let config = TransformConfig::default();
        let stop = Arc::new(AtomicBool::new(false));
        let mut publisher = CollectingPublisher::new(Some(2), Arc::clone(&stop));

        // 3 frames are available but stop_after=2 must end the loop before the
        // 3rd next_frame() call — if run_pipeline ignored `stop` this would
        // instead exhaust FakeSource and return an Err.
        let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

        assert!(result.is_ok());
        assert_eq!(publisher.published.len(), 2);
    }

    #[test]
    fn capture_error_propagates() {
        let mut source = FakeSource::new(vec![]);
        let config = TransformConfig::default();
        let stop = Arc::new(AtomicBool::new(false));
        let mut publisher = CollectingPublisher::new(None, Arc::clone(&stop));

        let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

        assert!(matches!(result, Err(PipelineError::Capture(CaptureError::FrameRead { .. }))));
    }

    #[test]
    fn publish_error_propagates() {
        let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).expect("valid frame");
        let mut source = FakeSource::new(vec![pixel]);
        let config = TransformConfig::default();
        let stop = Arc::new(AtomicBool::new(false));
        let mut publisher = FailingPublisher;

        let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

        assert!(matches!(result, Err(PipelineError::Publish(PublishError::Publish { .. }))));
    }
}
