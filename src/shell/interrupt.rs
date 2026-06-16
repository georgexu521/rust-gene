//! Interrupt state machine for the split-footer CLI.
//!
//! The first Ctrl+C cancels the current model turn; the second Ctrl+C, if
//! pressed while no turn is running, exits the CLI. This matches the behavior
//! of common terminal coding agents.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct InterruptState {
    /// True when a model turn is currently running.
    pub running: Arc<AtomicBool>,
    /// True if the user already requested an interrupt during this run.
    pub interrupted: Arc<AtomicBool>,
}

impl InterruptState {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_turn(&self) {
        self.running.store(true, Ordering::SeqCst);
        self.interrupted.store(false, Ordering::SeqCst);
    }

    pub fn end_turn(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.interrupted.store(false, Ordering::SeqCst);
    }

    /// Returns true if the turn should be cancelled.
    pub fn request_interrupt(&self) -> bool {
        if self.running.load(Ordering::SeqCst) {
            self.interrupted.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for InterruptState {
    fn default() -> Self {
        Self::new()
    }
}
