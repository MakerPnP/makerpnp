use std::sync::{Mutex, MutexGuard};

#[allow(dead_code)]
static LOCK: Mutex<bool> = Mutex::new(false);

/// Use a mutex to prevent multiple test threads interacting with the same static state.
/// This can happen when tests use the same mock context.  Without this mechanism tests will
/// interact with each other causing unexpected results and test failures.
#[allow(dead_code)]
pub fn aquire() -> MutexGuard<'static, bool> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
