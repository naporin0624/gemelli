use thiserror::Error;

use crate::frame::Frame;

pub trait TexturePublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError>;
}

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("failed to create texture server \"{name}\": {reason}")]
    ServerCreate { name: String, reason: String },
    #[error("failed to publish frame: {reason}")]
    Publish { reason: String },
}

#[cfg(test)]
mod tests {
    use super::{PublishError, TexturePublisher};
    use crate::frame::Frame;

    struct RecordingPublisher {
        received: Vec<Frame>,
    }

    impl TexturePublisher for RecordingPublisher {
        fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
            self.received.push(frame.clone());

            Ok(())
        }
    }

    #[test]
    fn dyn_texture_publisher_records_published_frame() {
        let frame = Frame::new(1, 1, vec![1, 2, 3, 255]).expect("valid frame");
        let mut recording = RecordingPublisher { received: Vec::new() };
        // Object-safety proof, mirroring CaptureSource in capture.rs.
        let publisher: &mut dyn TexturePublisher = &mut recording;

        publisher.publish(&frame).expect("publish succeeds");

        assert_eq!(recording.received, vec![frame]);
    }
}
