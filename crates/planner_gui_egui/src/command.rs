use std::future::Future;

/// A command that can be executed to produce a value of type `Output`.
pub trait Command {
    /// The type of value produced by executing this command.
    type Output;

    /// Executes this command and returns a future that will resolve to its output.
    fn execute(self) -> impl Future<Output = Self::Output> + Send;
}

/// Extension trait for commands that provides additional functionality.
pub trait CommandExt: Command {
    /// Maps the output of this command using the given function.
    fn map<F, T>(self, f: F) -> impl Command<Output = T> + Send
    where
        F: FnOnce(Self::Output) -> T + Send + 'static,
        Self: Sized + Send + 'static,
    {
        MapCommand { cmd: self, f }
    }
}

impl<T: Command> CommandExt for T {}

struct MapCommand<Cmd, F> {
    cmd: Cmd,
    f: F,
}

impl<Cmd, F, T> Command for MapCommand<Cmd, F>
where
    Cmd: Command + Send + 'static,
    F: FnOnce(Cmd::Output) -> T + Send + 'static,
{
    type Output = T;

    fn execute(self) -> impl Future<Output = Self::Output> + Send {
        async move {
            let output = self.cmd.execute().await;
            (self.f)(output)
        }
    }
}
