use std::fmt::{Debug, Formatter};

use tracing::trace;

use crate::sync::Enqueue;

pub struct ComponentState<UiCommand> {
    /// Commands sent before the mapper is configured are kept, and passed on once it is.
    pub sender: Enqueue<UiCommand>,
}

impl<UiCommand> Debug for ComponentState<UiCommand> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentState")
            .finish()
    }
}

impl<UiCommand: Send + Clone + 'static> Default for ComponentState<UiCommand> {
    fn default() -> Self {
        Self {
            sender: Enqueue::pending(),
        }
    }
}

impl<UiCommand: Send + Clone + Debug + 'static> ComponentState<UiCommand> {
    /// Passes each command sent to this component to `sender`, as it's sent, wrapped by `wrapper`.
    pub fn configure_mapper<F, WrappedUiCommand>(&mut self, sender: Enqueue<WrappedUiCommand>, mut wrapper: F)
    where
        F: FnMut(UiCommand) -> WrappedUiCommand + Send + 'static,
        WrappedUiCommand: Send + 'static,
    {
        self.sender.connect(move |command| {
            sender
                .send(wrapper(command))
                .expect("sent");
        });
    }

    pub fn send(&self, command: UiCommand) {
        self.sender.send(command).expect("sent");
    }
}

impl<UiCommand> Drop for ComponentState<UiCommand> {
    fn drop(&mut self) {
        trace!("dropping component state");
    }
}

pub trait UiComponent {
    type UiContext<'context>;
    type UiCommand;
    type UiAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, context: &mut Self::UiContext<'context>);

    #[profiling::function]
    fn update<'context>(
        &mut self,
        _command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        None
    }
}
