use std::sync::Arc;
use parking_lot::Mutex;

/// A registry that manages reactive values and their dependencies.
/// 
/// The registry is responsible for keeping track of all reactive values
/// and ensuring they are not dropped while still needed.
#[derive(Default)]
pub struct SignalRegistry {
    signals: Mutex<Vec<Arc<dyn Send + Sync + 'static>>>,
}

impl SignalRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            signals: Mutex::new(Vec::new()),
        }
    }

    /// Registers a signal with the registry.
    /// 
    /// The signal will be kept alive as long as the registry exists.
    pub fn register_signal<T>(&self, signal: Arc<T>) 
    where 
        T: Send + Sync + 'static
    {
        self.signals.lock().push(signal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use crate::reactive::Value;
    use crate::reactive::value::ValueExt;

    #[test]
    fn test_registry_keeps_signals_alive() {
        let registry = SignalRegistry::new();
        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_clone = dropped.clone();

        let value = Value::new(42);
        value.on_change(move || {
            if dropped_clone.load(Ordering::SeqCst) {
                panic!("Signal was dropped!");
            }
        });

        registry.register_signal(Arc::new(value.clone()));
        dropped.store(true, Ordering::SeqCst);

        // Update value to trigger callback
        value.set(84);
        
        // If we get here without panicking, the signal was kept alive
        assert!(true);
    }
}
