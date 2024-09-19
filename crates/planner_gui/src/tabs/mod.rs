use uuid::Uuid;
use vizia::prelude::*;
use crate::project::Project;
use crate::tabbed_document_container::TabbedDocument;
use crate::tabs::home::HomeTab;
use crate::tabs::project::ProjectTab;

pub mod project;
pub mod home;


#[derive(Clone, Data)]
pub enum TabKind {
    Home(HomeTab),
    Project(ProjectTab),
}

impl TabKind {
    pub fn name(&self) -> String {
        match self {
            TabKind::Home(_) => {
                // TODO l10n
                "Home".to_string()
            }
            TabKind::Project(tab) => {
                match tab.project {
                    None => {
                        // TODO l10n
                        "New project".to_string()
                    }
                    Some(ref project) => project.name.clone()
                }
            }
        }
    }
}

impl TabbedDocument for TabKind {

    fn build_tab(&self) -> TabPair {
        match self {
            TabKind::Home(tab) => tab.build_tab(self.name()),
            TabKind::Project(tab) => tab.build_tab(self.name()),
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        match self {
            TabKind::Home(tab) => tab.event(cx, event),
            TabKind::Project(tab) => tab.event(cx, event),
        }
    }

    fn id(&self) -> &String {
        match self {
            TabKind::Home(tab) => tab.id(),
            TabKind::Project(tab) => tab.id(),
        }
    }
}
