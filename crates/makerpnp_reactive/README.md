# MakerPnP Reactive

A thread-safe reactive state management system for MakerPnP.

## Features

- Thread-safe state sharing between UI and machine control threads
- Real-time updates for machine status, camera feeds, and sensor data
- Computed values that automatically update when dependencies change
- Clean architecture that separates state management from UI code

## Usage

```rust
use makerpnp_reactive::{Value, Derived};

// Create reactive values
let count = Value::new(0);
let doubled = Derived::new(&[count.clone()], move || {
    let val = *count.lock();
    val * 2
});

// Update values
count.set(5);
```

## Important Notes

### Asynchronous Change Notifications

The reactive system uses channels and background threads to propagate change notifications in a thread-safe manner. This means that updates to derived values happen asynchronously. In most UI applications, this is not noticeable due to the natural timing of UI events.

However, if you need to test for an updated value immediately after a change, you may need to add a small delay to allow for notification propagation:

```rust
use std::thread;
use std::time::Duration;

count.set(5);
thread::sleep(Duration::from_millis(50)); // Allow time for notification
assert_eq!(doubled.get(), 10);
```

This is primarily a testing concern and doesn't affect normal application usage where you're responding to UI events or using the reactive system to drive UI updates.
