//! Core reactive system components.
//! 
//! This module provides the fundamental building blocks for reactive state management:
//! 
//! - `Value<T>`: A thread-safe container for values that can be monitored for changes
//! - `Derived<T>`: Computed values that automatically update when dependencies change
//! - `SignalRegistry`: Registry that manages reactive values and their dependencies
//! 
//! # Example
//! 
//! ```rust
//! use std::sync::Arc;
//! use std::thread;
//! use std::time::Duration;
//! use makerpnp_reactive::{Value, Derived, SignalRegistry};
//! 
//! // Create a registry to manage reactive values
//! let registry = SignalRegistry::new();
//! 
//! // Create a value and register it
//! let count = Value::new(0i32);
//! let count_for_compute = count.clone();
//! registry.register_signal(Arc::new(count.clone()));
//! 
//! // Create a computed value that depends on count
//! let doubled = Derived::new(&[count.clone()], move || {
//!     let val = *count_for_compute.lock();
//!     val * 2
//! });
//! registry.register_signal(Arc::new(doubled.clone()));
//! 
//! // Update values using set() to ensure proper change notification
//! count.set(5);
//! 
//! // Wait a moment for notifications to propagate
//! thread::sleep(Duration::from_millis(50));
//! 
//! assert_eq!(doubled.get(), 10);
//! ```

mod value;
mod derived;
mod registry;

pub use value::{Value, ValueExt};
pub use derived::Derived;
pub use registry::SignalRegistry;
