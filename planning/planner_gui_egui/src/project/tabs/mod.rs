use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, Split, Style};

use crate::project::{ProjectTabKind, ProjectUiState};
use crate::sync::Value;
use crate::tabs::{AppTabViewer, Tab, TabKey, Tabs};
use crate::tabs_impl;
use crate::ui_component::{ComponentState, UiComponent};

//
// tabs
//
pub mod explorer_tab;
pub mod issues_tab;
pub mod load_out_tab;
pub mod overview_tab;
pub mod parts_tab;
pub mod pcb_tab;
pub mod phase_tab;
pub mod placements_tab;
pub mod process_tab;
pub mod unit_assignments_tab;

#[derive(Derivative, serde::Deserialize, serde::Serialize)]
#[derivative(Debug)]
pub struct ProjectTabs {
    tabs: Value<Tabs<ProjectTabKind, ProjectTabContext>>,
    tree: Value<DockState<TabKey>>,

    #[serde(skip)]
    pub component: ComponentState<ProjectTabUiCommand>,
}

impl Default for ProjectTabs {
    fn default() -> Self {
        Self {
            tabs: Value::new(Tabs::new()),
            tree: Value::new(DockState::new(vec![])),
            component: ComponentState::default(),
        }
    }
}

// Not to be confused with the other one...
#[derive(Debug)]
pub struct ProjectTabContext {
    pub state: Value<ProjectUiState>,
}

impl ProjectTabs {
    tabs_impl!(ProjectTabKind, ProjectTabContext);
}

#[derive(Debug, Clone)]
pub enum ProjectTabUiCommand {
    None,
}

pub enum ProjectTabAction {
    None,
}

impl UiComponent for ProjectTabs {
    type UiContext<'context> = ProjectTabContext;
    type UiCommand = ProjectTabUiCommand;
    type UiAction = ProjectTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ctx = ui.ctx();

        let mut tab_viewer = AppTabViewer {
            tabs: self.tabs.clone(),
            context,
        };

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
            .id(ui.id().with("project-tabs"))
            .style(Style::from_egui(ctx.global_style().as_ref()))
            .show_inside(ui, &mut tab_viewer);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ProjectTabUiCommand::None => Some(ProjectTabAction::None),
        }
    }
}

impl Tab for ProjectTabKind {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        match self {
            ProjectTabKind::Explorer(tab) => tab.label(),
            ProjectTabKind::Issues(tab) => tab.label(),
            ProjectTabKind::LoadOut(tab) => tab.label(),
            ProjectTabKind::Overview(tab) => tab.label(),
            ProjectTabKind::Parts(tab) => tab.label(),
            ProjectTabKind::Pcb(tab) => tab.label(),
            ProjectTabKind::Phase(tab) => tab.label(),
            ProjectTabKind::Placements(tab) => tab.label(),
            ProjectTabKind::Process(tab) => tab.label(),
            ProjectTabKind::UnitAssignments(tab) => tab.label(),
        }
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            ProjectTabKind::Explorer(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Issues(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::LoadOut(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Overview(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Parts(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Pcb(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Phase(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Placements(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::Process(tab) => tab.ui(ui, tab_key, context),
            ProjectTabKind::UnitAssignments(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close<'a>(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        match self {
            ProjectTabKind::Explorer(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Issues(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::LoadOut(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Overview(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Parts(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Pcb(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Phase(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Placements(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::Process(tab) => tab.on_close(tab_key, context),
            ProjectTabKind::UnitAssignments(tab) => tab.on_close(tab_key, context),
        }
    }
}
