use vizia::prelude::*;
use crate::tabbed_document_container::TabbedDocument;
use crate::tabs::document::DocumentTab;
use crate::tabs::home::HomeTab;

pub mod document;
pub mod home;


#[derive(Clone, Data)]
pub enum TabKind {
    Home(HomeTab),
    Document(DocumentTab),
}

impl TabKind {
    pub fn name(&self) -> String {
        match self {
            TabKind::Home(_) => { "Home".to_string() }
            TabKind::Document(document_tab) => { document_tab.document.name.clone() }
        }
    }
}

impl TabbedDocument for TabKind {

    fn build_tab(&self) -> TabPair {
        match self {
            TabKind::Home(tab) => tab.build_tab(self.name()),
            TabKind::Document(tab) => tab.build_tab(),
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        match self {
            TabKind::Home(tab) => tab.event(cx, event),
            TabKind::Document(tab) => tab.event(cx, event)
        }
    }
}
