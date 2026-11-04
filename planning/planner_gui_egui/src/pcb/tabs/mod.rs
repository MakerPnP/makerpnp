use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, NodeIndex, Split, Style};
use tracing::debug;

use crate::pcb::{PcbTabKind, PcbUiState};
use crate::sync::Value;
use crate::tabs::{AppTabViewer, Tab, TabKey, Tabs};
use crate::tabs_impl;
use crate::ui_component::{ComponentState, UiComponent};

pub mod configuration_tab;
pub mod explorer_tab;
pub mod gerber_viewer_tab;

pub mod panel_tab;

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct PcbTabs {
    tabs: Value<Tabs<PcbTabKind, PcbTabContext>>,
    tree: Value<DockState<TabKey>>,

    #[serde(skip)]
    pub component: ComponentState<PcbTabUiCommand>,
}

impl Default for PcbTabs {
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
pub struct PcbTabContext {
    pub state: Value<PcbUiState>,
}

impl PcbTabs {
    tabs_impl!(PcbTabKind, PcbTabContext);
}

#[derive(Debug, Clone)]
pub enum PcbTabUiCommand {
    None,
}

pub enum PcbTabAction {
    None,
}

impl UiComponent for PcbTabs {
    type UiContext<'context> = PcbTabContext;
    type UiCommand = PcbTabUiCommand;
    type UiAction = PcbTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ctx = ui.ctx();

        let mut tab_viewer = AppTabViewer {
            tabs: self.tabs.clone(),
            context,
        };

        let mut tree = self.tree.lock().unwrap();

        DockArea::new(&mut tree)
            .id(ui.id().with("pcb-tabs"))
            .style(Style::from_egui(ctx.global_style().as_ref()))
            .show_inside(ui, &mut tab_viewer);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("PcbTabs command. command: {:?}", command);
        match command {
            PcbTabUiCommand::None => Some(PcbTabAction::None),
        }
    }
}

impl Tab for PcbTabKind {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        match self {
            PcbTabKind::Explorer(tab) => tab.label(),
            PcbTabKind::Configuration(tab) => tab.label(),
            PcbTabKind::Panel(tab) => tab.label(),
            PcbTabKind::GerberViewer(tab) => tab.label(),
        }
    }

    fn ui<'a>(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        match self {
            PcbTabKind::Explorer(tab) => tab.ui(ui, tab_key, context),
            PcbTabKind::Configuration(tab) => tab.ui(ui, tab_key, context),
            PcbTabKind::Panel(tab) => tab.ui(ui, tab_key, context),
            PcbTabKind::GerberViewer(tab) => tab.ui(ui, tab_key, context),
        }
    }

    fn on_close<'a>(&mut self, tab_key: &TabKey, context: &mut Self::Context) -> OnCloseResponse {
        match self {
            PcbTabKind::Explorer(tab) => tab.on_close(tab_key, context),
            PcbTabKind::Configuration(tab) => tab.on_close(tab_key, context),
            PcbTabKind::Panel(tab) => tab.on_close(tab_key, context),
            PcbTabKind::GerberViewer(tab) => tab.on_close(tab_key, context),
        }
    }
}
