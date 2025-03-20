use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{channel, Sender};

use parking_lot::Mutex as PLMutex;

/// A thread-safe container for values that can be monitored for changes.
#[derive(Clone)]
pub struct Value<T> {
    pub(crate) inner: Arc<Mutex<T>>,
    notifiers: Arc<PLMutex<Vec<Sender<()>>>>,
}

impl<T> Value<T> {
    /// Gets a lock on the inner value.
    pub fn lock(&self) -> std::sync::MutexGuard<T> {
        self.inner.lock().unwrap()
    }
}

impl<T: Clone + Send + 'static> Value<T> {
    /// Creates a new Value with the given initial value.
    pub fn new(initial: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(initial)),
            notifiers: Arc::new(PLMutex::new(Vec::new())),
        }
    }

    /// Gets the current value.
    pub fn get(&self) -> T {
        self.inner.lock().unwrap().clone()
    }

    /// Sets a new value.
    pub fn set(&self, value: T) {
        let mut guard = self.inner.lock().unwrap();
        *guard = value;
        drop(guard); // Release lock before notifications

        // Notify all listeners
        let notifiers = self.notifiers.lock();
        for notifier in notifiers.iter() {
            let _ = notifier.send(()); // Ignore errors from closed channels
        }
        drop(notifiers); // Explicitly release lock
    }
}

/// Extension trait for monitoring value changes.
pub trait ValueExt<T: Clone + Send + Sync + 'static> {
    /// Registers a callback to be called when the value changes.
    ///
    /// The callback is invoked in a dedicated background thread that polls for changes
    /// every 100ms. Returns an Arc to the callback function.
    fn on_change<F>(&self, callback: F) -> Arc<F>
    where
        F: Fn() + Send + Sync + 'static;
}

impl<T: Clone + Send + Sync + PartialEq + 'static> ValueExt<T> for Value<T> {
    fn on_change<F>(&self, callback: F) -> Arc<F>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let cb = Arc::new(callback);
        let cb_clone = cb.clone();
        
        // Create a channel for change notifications
        let (tx, rx) = channel();
        
        // Add the sender to our notifiers
        self.notifiers.lock().push(tx);
        
        // Spawn a background thread to wait for notifications
        thread::spawn(move || {
            while rx.recv().is_ok() {
                cb_clone();
            }
        });

        cb
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn test_value_get_set() {
        let value = Value::new(42);
        assert_eq!(value.get(), 42);

        value.set(84);
        assert_eq!(value.get(), 84);
    }

    #[test]
    fn test_value_change_notification() {
        let value = Value::new(0);
        let changed = Arc::new(AtomicBool::new(false));
        let changed_clone = changed.clone();

        value.on_change(move || {
            changed_clone.store(true, Ordering::SeqCst);
        });

        value.set(1);
        thread::sleep(Duration::from_millis(50));
        assert!(changed.load(Ordering::SeqCst));
    }
}
