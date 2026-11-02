//! Types for sharing state and sending commands between threads.

use std::fmt::{Debug, Formatter};
use std::ops::{Deref, DerefMut};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

/// A sender of commands, sending never blocks since the channel is unbounded.
pub type Enqueue<T> = Sender<T>;

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

/// The receiving end of a channel, handles what it receives on a dedicated thread.
pub struct Slot<T> {
    receiver: Arc<Mutex<Receiver<T>>>,
}

impl<T: Send + 'static> Slot<T> {
    /// Starts a thread which calls the handler for each item received, until all the senders have been dropped.
    pub fn start<F>(&mut self, mut handler: F)
    where
        F: FnMut(T) + Send + 'static,
    {
        let receiver = self.receiver.clone();

        thread::spawn(move || {
            let receiver = receiver.lock().unwrap();
            for item in receiver.iter() {
                handler(item);
            }
        });
    }
}

pub fn channel<T>() -> (Enqueue<T>, Slot<T>) {
    let (sender, receiver) = mpsc::channel();

    (sender, Slot {
        receiver: Arc::new(Mutex::new(receiver)),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn clones_of_a_value_share_the_same_state() {
        let value = Value::new(1);
        let clone = value.clone();

        *clone.lock().unwrap() = 2;

        assert_eq!(*value.lock().unwrap(), 2);
    }

    #[test]
    fn a_slot_handles_items_in_the_order_they_were_sent() {
        let (sender, mut slot) = channel::<u32>();
        let (result_sender, result_receiver) = mpsc::channel();

        slot.start(move |item| result_sender.send(item).unwrap());

        for item in 0..100 {
            sender.send(item).unwrap();
        }
        drop(sender);

        // the iterator ends when the slot's thread drops the handler, which proves the thread stops too.
        assert_eq!(
            result_receiver
                .iter()
                .collect::<Vec<_>>(),
            (0..100).collect::<Vec<_>>()
        );
    }
}
