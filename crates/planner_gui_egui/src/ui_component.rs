use std::fmt::{Debug, Formatter};

use egui_mobius::slot::Slot;
use egui_mobius::types::Enqueue;
use tracing::debug;

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
        let (signal, slot) = egui_mobius::factory::create_signal_slot::<UiCommand>();

        Self {
            sender: signal.sender.clone(),
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
                debug!("command: {:?}", command);
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

pub trait UiComponent {
    type UiContext<'context>;
    type UiCommand;
    type UiAction;

    fn ui<'context>(&self, ui: &mut egui::Ui, context: &mut Self::UiContext<'context>);

    fn update<'context>(
        &mut self,
        _command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        None
    }
}
