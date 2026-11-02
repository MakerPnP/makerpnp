use std::fmt::{Debug, Formatter};

use tracing::trace;

use crate::sync::Enqueue;
use crate::sync::Slot;

pub struct ComponentState<UiCommand> {
    pub sender: Enqueue<UiCommand>,
    #[allow(dead_code)]
    slot: Slot<UiCommand>,
}

impl<UiCommand> Debug for ComponentState<UiCommand> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentState")
            .finish()
    }
}

impl<UiCommand: Send + Clone + 'static> Default for ComponentState<UiCommand> {
    fn default() -> Self {
        let (sender, slot) = crate::sync::channel::<UiCommand>();

        Self {
            sender,
            slot,
        }
    }
}

impl<UiCommand: Send + Clone + Debug + 'static> ComponentState<UiCommand> {
    pub fn configure_mapper<F, WrappedUiCommand>(&mut self, sender: Enqueue<WrappedUiCommand>, mut wrapper: F)
    where
        F: FnMut(UiCommand) -> WrappedUiCommand + Send + 'static,
        WrappedUiCommand: Send + 'static,
    {
        self.slot.start({
            move |command| {
                //trace!("command: {:?}", command);
                sender
                    .send(wrapper(command))
                    .expect("sent");
            }
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
