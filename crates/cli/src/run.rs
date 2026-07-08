//! Full implementation lands in Task 13. This stub exists only so `select.rs`
//! (Task 12) has a `CliError` to compile against.

// No production code constructs this yet (main.rs wiring happens in Task 13);
// only select.rs's choose_device references it so far. Remove once main.rs consumes it.
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("device selection cancelled")]
    SelectionCancelled,
}
