//! Shared runtime state managed by Tauri (`.manage(NativeTtsState::default())`).
//!
//! The loaded engine slot, in-flight cancellation set, and model-install guard only
//! exist when the native engine is compiled in; the disabled build keeps a
//! zero-cost placeholder so the managed type is identical across both feature
//! configurations.

use std::sync::Mutex;

#[cfg(feature = "native-tts-core")]
use std::sync::Arc;

#[cfg(feature = "native-tts-core")]
use std::collections::HashSet;

#[cfg(feature = "native-tts-core")]
use super::engine::LoadedTtsEngine;

pub struct NativeTtsState {
    #[cfg(feature = "native-tts-core")]
    pub(crate) engine: Arc<Mutex<Option<LoadedTtsEngine>>>,
    #[cfg(feature = "native-tts-core")]
    pub(crate) cancelled_jobs: Arc<Mutex<HashSet<String>>>,
    #[cfg(feature = "native-tts-core")]
    pub(crate) model_installing: Arc<Mutex<HashSet<String>>>,
    #[cfg(not(feature = "native-tts-core"))]
    _disabled: Mutex<()>,
}

impl NativeTtsState {
    /// True during a model or SILMA runtime install, or while a job holds the engine.
    /// Audiobook saves keep the engine locked for the whole job, so `try_lock` also
    /// detects them, and never waits on one. A poisoned lock reads as busy.
    pub(crate) fn is_busy(&self) -> bool {
        #[cfg(feature = "native-tts-core")]
        {
            self.engine.try_lock().is_err()
                || self
                    .model_installing
                    .lock()
                    .map_or(true, |installing| !installing.is_empty())
        }
        #[cfg(not(feature = "native-tts-core"))]
        false
    }

    /// Drop the loaded engine, which is otherwise reused without rechecking disk, so
    /// it would keep "working" after its model files are gone. Blocks until any
    /// running job releases the lock, so call `is_busy` first.
    pub(crate) fn unload_engine(&self) {
        #[cfg(feature = "native-tts-core")]
        if let Ok(mut engine) = self.engine.lock() {
            *engine = None;
        }
    }
}

impl Default for NativeTtsState {
    fn default() -> Self {
        Self {
            #[cfg(feature = "native-tts-core")]
            engine: Arc::new(Mutex::new(None)),
            #[cfg(feature = "native-tts-core")]
            cancelled_jobs: Arc::new(Mutex::new(HashSet::new())),
            #[cfg(feature = "native-tts-core")]
            model_installing: Arc::new(Mutex::new(HashSet::new())),
            #[cfg(not(feature = "native-tts-core"))]
            _disabled: Mutex::new(()),
        }
    }
}
