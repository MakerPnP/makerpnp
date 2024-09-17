use vizia::context::{Context, EventContext};
use vizia::events::Event;
use vizia::prelude::*;
use crate::popups::new_project::NewProjectPopup;

pub mod new_project;

#[derive(Clone, Debug, Data)]
pub enum PopupWindow {
    NewProject (NewProjectPopup)
}

impl PopupWindow {
    pub fn build<'a, L: Lens<Target = Option<PopupWindow>>>(&self, cx: &'a mut Context, lens: L) -> Handle<'a, Window> {
        match self {
            PopupWindow::NewProject(popup) => popup.build(cx, lens),
        }
    }

    pub fn on_event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        match self {
            PopupWindow::NewProject(popup) => popup.on_event(ecx, event),
        }
    }
}

#[derive(Clone, Debug, Default, Data, Lens)]
pub struct PopupWindowState {
    pub enabled: bool,
    pub kind: Option<PopupWindow>,
}
