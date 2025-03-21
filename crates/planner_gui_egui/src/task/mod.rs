use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;

use futures::FutureExt;
use futures::StreamExt;
use futures::stream::BoxStream;
use futures::{Stream, future, stream};

use crate::runtime::boxed_stream;

pub struct Task<T>(Option<BoxStream<'static, T>>);

impl<T> Task<T> {
    /// Creates a [`Task`] that does nothing.
    pub fn none() -> Self {
        Self(None)
    }

    /// Creates a new [`Task`] that instantly produces the given value.
    pub fn done(value: T) -> Self
    where
        T: Send + 'static,
    {
        Self::future(future::ready(value))
    }

    /// Creates a [`Task`] that runs the given [`Future`] to completion and maps its
    /// output with the given closure.
    pub fn perform<O>(future: impl Future<Output = O> + Send + 'static, f: impl Fn(O) -> T + Send + 'static) -> Self
    where
        T: Send + 'static,
        O: Send + 'static,
    {
        Self::future(future.map(f))
    }

    /// Creates a [`Task`] that runs the given [`Stream`] to completion and maps each
    /// item with the given closure.
    pub fn run<A>(stream: impl Stream<Item = A> + Send + 'static, f: impl Fn(A) -> T + Send + 'static) -> Self
    where
        T: 'static,
    {
        Self::stream(stream.map(f))
    }

    /// Combines the given tasks and produces a single [`Task`] that will run all of them
    /// in parallel.
    pub fn batch(tasks: impl IntoIterator<Item = Self>) -> Self
    where
        T: 'static,
    {
        Self(Some(boxed_stream(stream::select_all(
            tasks
                .into_iter()
                .filter_map(|task| task.0),
        ))))
    }

    /// Maps the output of a [`Task`] with the given closure.
    pub fn map<O>(self, mut f: impl FnMut(T) -> O + Send + 'static) -> Task<O>
    where
        T: Send + 'static,
        O: Send + 'static,
    {
        self.then(move |output| Task::done(f(output)))
    }

    /// Performs a new [`Task`] for every output of the current [`Task`] using the
    /// given closure.
    ///
    /// This is the monadic interface of [`Task`]—analogous to [`Future`] and
    /// [`Stream`].
    pub fn then<O>(self, mut f: impl FnMut(T) -> Task<O> + Send + 'static) -> Task<O>
    where
        T: Send + 'static,
        O: Send + 'static,
    {
        Task(match self.0 {
            None => None,
            Some(stream) => Some(boxed_stream(stream.flat_map(move |output| {
                let result = f(output)
                    .0
                    .unwrap_or_else(|| boxed_stream(stream::empty()));
                result
            }))),
        })
    }

    /// Chains a new [`Task`] to be performed once the current one finishes completely.
    pub fn chain(self, task: Self) -> Self
    where
        T: 'static,
    {
        match self.0 {
            None => task,
            Some(first) => match task.0 {
                None => Task::none(),
                Some(second) => Task(Some(boxed_stream(first.chain(second)))),
            },
        }
    }

    /// Creates a new [`Task`] that runs the given [`Future`] and produces
    /// its output.
    pub fn future(future: impl Future<Output = T> + Send + 'static) -> Self
    where
        T: 'static,
    {
        Self::stream(stream::once(future))
    }

    /// Creates a new [`Task`] that runs the given [`Stream`] and produces
    /// each of its items.
    pub fn stream(stream: impl Stream<Item = T> + Send + 'static) -> Self
    where
        T: 'static,
    {
        Self(Some(boxed_stream(stream)))
    }
}

pub fn into_stream<T>(task: Task<T>) -> Option<BoxStream<'static, T>> {
    task.0
}

pub fn into_future<T: 'static>(task: Task<T>) -> Option<Pin<Box<dyn Future<Output = T> + Send + 'static>>> {
    task.0.map(|stream| {
        let future = stream.into_future().map(|(item, _)| item.unwrap());
        Box::pin(future) as Pin<Box<dyn Future<Output = T> + Send + 'static>>
    })
}

impl<T> Debug for Task<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Task<...>")
    }
}

impl<T, E> Task<Result<T, E>> {
    /// Executes a new [`Task`] after this one, only when it succeeds with an `Ok` value.
    /// If the task returns an `Err` then the `Err` value is consumed.
    ///
    /// The success value is provided to the closure to create the subsequent [`Task`].
    pub fn and_then<A>(self, f: impl Fn(T) -> Task<A> + Send + 'static) -> Task<A>
    where
        T: Send + 'static,
        E: Send + 'static,
        A: Send + 'static,
    {
        self.then(move |result| result.map_or_else(|_err: E| Task::none(), &f))
    }

    pub fn map_err<O>(self, f: impl Fn(E) -> O + Send + 'static) -> Task<Result<T, O>>
    where
        T: Send + 'static,
        E: Send + 'static,
        O: Send + 'static,
    {
        self.map(move |result| {
            let foo = match result {
                Ok(value) => Ok(value),
                Err(error) => Err(f(error)),
            };

            foo
        })
    }

    pub fn or_else(self, f: impl Fn(E) -> Self + Send + 'static) -> Self
    where
        T: Send + 'static,
        E: Send + 'static,
    {
        self.then(move |result| match result {
            Ok(value) => Task::done(Ok(value)),
            Err(error) => f(error),
        })
    }

    pub fn inspect_err(self, f: impl Fn(&E) + Send + 'static) -> Task<Result<T, E>>
    where
        T: Send + 'static,
        E: Send + 'static,
    {
        let task = self.then(move |result| {
            let result = result.inspect_err(|error| f(error));
            Task::done(result)
        });

        task
    }
}
