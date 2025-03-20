use std::sync::{Arc, Mutex};
use crate::reactive::Value;
use crate::reactive::value::ValueExt;

/// A computed value that automatically updates when its dependencies change.
#[derive(Clone)]
pub struct Derived<T> {
    value: Arc<Mutex<T>>,
}

impl<T: Clone + Send + 'static> Derived<T> {
    /// Creates a new derived value that depends on the given values.
    ///
    /// The compute function is called whenever any of the dependencies change.
    /// 
    /// # Arguments
    /// * `deps` - List of values this derived value depends on
    /// * `compute` - Function that computes the derived value from its dependencies
    ///
    /// # Example
    /// ```rust
    /// use makerpnp_reactive::{Value, Derived};
    ///
    /// let count = Value::new(0);
    /// let doubled = Derived::new(&[count.clone()], move || {
    ///     let val = *count.lock();
    ///     val * 2
    /// });
    /// ```
    pub fn new<F, D>(deps: &[Value<D>], compute: F) -> Self
    where
        F: Fn() -> T + Send + Sync + Clone + 'static,
        D: Clone + Send + Sync + PartialEq + 'static,
    {
        // Compute initial value
        let initial = compute();
        let value = Arc::new(Mutex::new(initial));
        let value_clone = value.clone();

        // Set up change handlers for all dependencies
        // We need to ensure the value is updated when ANY dependency changes
        let compute = Arc::new(compute);
        let value = value.clone();
        
        // Create change handlers for all dependencies
        for dep in deps {
            let compute = compute.clone();
            let value = value.clone();
            dep.on_change(move || {
                let new_value = compute();
                *value.lock().unwrap() = new_value;
            });
        }

        Self {
            value: value_clone,
        }
    }

    /// Gets the current value.
    pub fn get(&self) -> T {
        self.value.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::Value;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_derived_updates() {
        let count = Value::new(0);
        let count_for_compute = count.clone();
        let doubled = Derived::new(&[count.clone()], move || {
            *count_for_compute.lock() * 2
        });

        assert_eq!(doubled.get(), 0);

        count.set(5);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(doubled.get(), 10);
    }

    #[test]
    fn test_derived_multiple_deps() {
        let a = Value::new(1);
        let b = Value::new(2);
        let a_for_compute = a.clone();
        let b_for_compute = b.clone();
        let sum = Derived::new(&[a.clone(), b.clone()], move || {
            *a_for_compute.lock() + *b_for_compute.lock()
        });

        assert_eq!(sum.get(), 3);

        a.set(5);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(sum.get(), 7);
    }
}
