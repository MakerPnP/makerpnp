//! A thread-safe reactive state management system for MakerPnP.
//! 
//! This crate provides a reactive programming model that enables automatic UI updates
//! when state changes, with built-in thread safety and change detection.
//! 
//! # Key Features
//! 
//! - Thread-safe state sharing between UI and machine control threads
//! - Real-time updates for machine status, camera feeds, and sensor data
//! - Computed values that automatically update (e.g., machine status indicators)
//! - Clean architecture that separates state management from UI code
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
//! // Create values for machine state
//! let x_position = Value::new(0.0f64);
//! let y_position = Value::new(0.0f64);
//! 
//! // Clone values for use in compute closure
//! let x_for_compute = x_position.clone();
//! let y_for_compute = y_position.clone();
//! 
//! // Create a computed value for total distance
//! let distance = Derived::new(&[x_position.clone(), y_position.clone()], move || {
//!     let x = *x_for_compute.lock();
//!     let y = *y_for_compute.lock();
//!     (x * x + y * y).sqrt()
//! });
//! 
//! // Values automatically update
//! x_position.set(3.0);
//! y_position.set(4.0);
//! 
//! // Wait a moment for notifications to propagate
//! thread::sleep(Duration::from_millis(50));
//! 
//! assert_eq!(distance.get(), 5.0); // Pythagorean theorem!
//! ```

pub mod reactive;

pub use reactive::{Value, Derived, SignalRegistry};
