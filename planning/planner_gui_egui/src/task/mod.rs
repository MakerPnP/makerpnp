use std::fmt::{Debug, Formatter};

/// Follow-up work for the application to do after handling a command: the commands or actions to handle next, in order.
///
/// Every task is finished as soon as it's made, there's nothing to wait for. The application handles what it produces
/// on the thread that draws the frames, after the commands already sent (`GARCH-11`).
pub struct Task<T>(Vec<T>);

impl<T> Task<T> {
    /// Creates a [`Task`] that does nothing.
    pub fn none() -> Self {
        Self(Vec::new())
    }

    /// Creates a [`Task`] that produces the given value.
    pub fn done(value: T) -> Self {
        Self(vec![value])
    }

    /// Combines the given tasks into one, which produces what each of them does, in order.
    pub fn batch(tasks: impl IntoIterator<Item = Self>) -> Self {
        Self(
            tasks
                .into_iter()
                .flat_map(|task| task.0)
                .collect(),
        )
    }

    /// Maps each value the [`Task`] produces with the given closure.
    pub fn map<O>(self, f: impl FnMut(T) -> O) -> Task<O> {
        Task(self.0.into_iter().map(f).collect())
    }

    /// Makes a new [`Task`] for each value the current one produces, using the given closure.
    pub fn then<O>(self, mut f: impl FnMut(T) -> Task<O>) -> Task<O> {
        Task(
            self.0
                .into_iter()
                .flat_map(|value| f(value).0)
                .collect(),
        )
    }

    /// Produces what this [`Task`] does, followed by what `task` does.
    pub fn chain(mut self, task: Self) -> Self {
        self.0.extend(task.0);
        self
    }
}

/// What the task produces, in order.
pub fn run_to_completion<T>(task: Task<T>) -> Vec<T> {
    task.0
}

impl<T> Debug for Task<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task<{} values>", self.0.len())
    }
}

impl<T, E> Task<Result<T, E>> {
    /// Makes a new [`Task`] for each `Ok` value this one produces, using the given closure. `Err` values are dropped.
    pub fn and_then<A>(self, f: impl Fn(T) -> Task<A>) -> Task<A> {
        self.then(move |result| result.map_or_else(|_err: E| Task::none(), &f))
    }

    pub fn map_err<O>(self, f: impl Fn(E) -> O) -> Task<Result<T, O>> {
        self.map(move |result| result.map_err(&f))
    }

    pub fn or_else(self, f: impl Fn(E) -> Self) -> Self {
        self.then(move |result| match result {
            Ok(value) => Task::done(Ok(value)),
            Err(error) => f(error),
        })
    }

    pub fn inspect_err(self, f: impl Fn(&E)) -> Task<Result<T, E>> {
        self.map(move |result| result.inspect_err(|error| f(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_produces_its_values_in_order() {
        let task = Task::batch([Task::done(1), Task::none(), Task::done(2).chain(Task::done(3))])
            .then(|value| Task::batch([Task::done(value), Task::done(value * 10)]))
            .map(|value| value + 1);

        assert_eq!(run_to_completion(task), [2, 11, 3, 21, 4, 31]);
    }

    #[test]
    fn chaining_a_task_that_does_nothing_keeps_the_first() {
        assert_eq!(run_to_completion(Task::done(1).chain(Task::none())), [1]);
    }
}
