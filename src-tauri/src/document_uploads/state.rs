//! Shared state for the single in-flight document batch import.

use std::sync::{Arc, Mutex};

#[derive(Default)]
struct BatchState {
    running: bool,
    cancelled: bool,
}

#[derive(Clone, Default)]
pub(crate) struct DocumentUploadState {
    batch: Arc<Mutex<BatchState>>,
}

impl DocumentUploadState {
    /// Reserve the one batch slot and return a guard that releases it on every exit path.
    pub(crate) fn begin_batch(&self) -> Result<DocumentBatchControl, String> {
        let mut batch = self
            .batch
            .lock()
            .map_err(|_| "Document batch state lock poisoned".to_string())?;
        if batch.running {
            return Err("Another document import is already running".into());
        }
        batch.running = true;
        batch.cancelled = false;
        drop(batch);
        Ok(DocumentBatchControl {
            batch: self.batch.clone(),
        })
    }

    /// Ask the active batch to stop before it begins another file.
    pub(crate) fn cancel_batch(&self) -> Result<bool, String> {
        let mut batch = self
            .batch
            .lock()
            .map_err(|_| "Document batch state lock poisoned".to_string())?;
        if batch.running {
            batch.cancelled = true;
        }
        Ok(batch.running)
    }
}

pub(crate) struct DocumentBatchControl {
    batch: Arc<Mutex<BatchState>>,
}

impl DocumentBatchControl {
    /// Read the cooperative flag without taking ownership of the shared state.
    pub(crate) fn is_cancelled(&self) -> Result<bool, String> {
        let batch = self
            .batch
            .lock()
            .map_err(|_| "Document batch state lock poisoned".to_string())?;
        Ok(batch.cancelled)
    }
}

impl Drop for DocumentBatchControl {
    fn drop(&mut self) {
        // RAII releases the slot after success, error, cancellation, or panic.
        if let Ok(mut batch) = self.batch.lock() {
            batch.running = false;
            batch.cancelled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_state_serializes_and_cancels_imports() {
        let state = DocumentUploadState::default();
        let control = state.begin_batch().expect("first batch");

        assert!(state.begin_batch().is_err());
        assert!(state.cancel_batch().expect("cancel active batch"));
        assert!(control.is_cancelled().expect("read cancellation"));

        drop(control);
        assert!(!state.cancel_batch().expect("no active batch"));
        assert!(state.begin_batch().is_ok());
    }
}
