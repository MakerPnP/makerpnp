//! Types for sharing state and sending commands.
//!
//! Commands are handled one at a time, in the order they were sent, on the thread that draws the frames (`GARCH-11`).
//! Sending a command passes it straight through each component's mapper to the application's [`Queue`], which the
//! application handles at the start of each frame. No threads are involved.

use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// A sender of commands. Sending never blocks.
///
/// Until it has somewhere to send to, see [`Enqueue::connect`], commands are kept, and they are sent in order once it
/// has. Clones send to the same place.
pub struct Enqueue<T>(Arc<Mutex<Forward<T>>>);

enum Forward<T> {
    Pending(Vec<T>),
    To(Box<dyn FnMut(T) + Send>),
}

/// The receiver of an [`Enqueue`] no longer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

impl<T> Enqueue<T> {
    /// A sender which keeps what it's sent until it's connected.
    pub fn pending() -> Self {
        Self(Arc::new(Mutex::new(Forward::Pending(Vec::new()))))
    }

    pub fn send(&self, item: T) -> Result<(), Disconnected> {
        let mut forward = self
            .0
            .lock()
            .map_err(|_| Disconnected)?;
        match &mut *forward {
            Forward::Pending(items) => items.push(item),
            Forward::To(forward) => forward(item),
        }
        Ok(())
    }

    /// Sends everything this sender is sent to `forward`, starting with anything sent so far.  Replaces any previous
    /// connection.
    pub fn connect(&self, mut forward: impl FnMut(T) + Send + 'static) {
        let mut state = self.0.lock().unwrap();
        if let Forward::Pending(items) = &mut *state {
            for item in items.drain(..) {
                forward(item);
            }
        }
        *state = Forward::To(Box::new(forward));
    }
}

impl<T> Clone for Enqueue<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Debug for Enqueue<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Enqueue")
    }
}

/// The commands sent to an [`Enqueue`], in the order they were sent, for the application to handle.
pub struct Queue<T>(Arc<Mutex<VecDeque<T>>>);

impl<T> Queue<T> {
    pub fn pop(&self) -> Option<T> {
        self.0.lock().unwrap().pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }
}

/// A sender, and the queue of what it's sent.
pub fn queue<T: Send + 'static>() -> (Enqueue<T>, Queue<T>) {
    let items = Arc::new(Mutex::new(VecDeque::new()));
    let sender = Enqueue::pending();
    sender.connect({
        let items = items.clone();
        move |item| {
            items.lock().unwrap().push_back(item);
        }
    });

    (sender, Queue(items))
}

/// Shared state, clones refer to the same value.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Value<T>(Arc<Mutex<T>>);

impl<T> Value<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }

    pub fn lock(&self) -> Result<ValueGuard<'_, T>, PoisonError<MutexGuard<'_, T>>> {
        self.0.lock().map(ValueGuard)
    }
}

impl<T: Default> Default for Value<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Clone for Value<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Debug> Debug for Value<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Value")
            .field(&self.0)
            .finish()
    }
}

pub struct ValueGuard<'a, T>(MutexGuard<'a, T>);

impl<T> Deref for ValueGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ValueGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_of_a_value_share_the_same_state() {
        let value = Value::new(1);
        let clone = value.clone();

        *clone.lock().unwrap() = 2;

        assert_eq!(*value.lock().unwrap(), 2);
    }

    #[test]
    fn commands_are_queued_in_the_order_they_were_sent_through_any_sender() {
        let (sender, queue) = queue::<String>();

        let first = Enqueue::<u32>::pending();
        let second = Enqueue::<u32>::pending();
        first.connect({
            let sender = sender.clone();
            move |item| {
                sender
                    .send(format!("first {item}"))
                    .unwrap()
            }
        });
        second.connect({
            let sender = sender.clone();
            move |item| {
                sender
                    .send(format!("second {item}"))
                    .unwrap()
            }
        });

        first.send(1).unwrap();
        second.send(2).unwrap();
        first.send(3).unwrap();

        let queued = std::iter::from_fn(|| queue.pop()).collect::<Vec<_>>();
        assert_eq!(queued, ["first 1", "second 2", "first 3"]);
    }

    #[test]
    fn commands_sent_before_connecting_are_sent_once_connected() {
        let (sender, queue) = queue::<u32>();
        let pending = Enqueue::<u32>::pending();
        let clone = pending.clone();

        pending.send(1).unwrap();
        clone.send(2).unwrap();
        assert!(queue.is_empty());

        pending.connect(move |item| sender.send(item * 10).unwrap());
        clone.send(3).unwrap();

        let queued = std::iter::from_fn(|| queue.pop()).collect::<Vec<_>>();
        assert_eq!(queued, [10, 20, 30]);
    }
}
