//! Progress-reporting boundary.
//!
//! Long operations (e.g. VM bring-up) surface step updates through [`Reporter`] so core stays
//! decoupled from stdout — the CLI prints them, the GUI can render them, tests can record them.

/// Receives human-readable step updates from a long-running operation.
pub trait Reporter {
    fn step(&self, msg: &str);
}

/// Discards progress (tests, non-interactive callers).
#[derive(Debug, Default)]
pub struct SilentReporter;

impl Reporter for SilentReporter {
    fn step(&self, _msg: &str) {}
}
