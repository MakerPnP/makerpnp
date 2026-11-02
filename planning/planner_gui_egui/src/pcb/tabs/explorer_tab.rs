use std::collections::BTreeMap;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::tr;
use egui_ltreeview::{Action, NodeBuilder, TreeView, TreeViewState};
use planner_app::{PcbOverview, PcbUnitIndex};
use tracing::debug;

use crate::pcb::tabs::PcbTabContext;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ExplorerUi {
    pcb_overview: Option<PcbOverview>,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,
    navigation_paths: Value<BTreeMap<usize, NavigationPath>>,

    pub component: ComponentState<ExplorerTabUiCommand>,
}

impl ExplorerUi {
    pub fn new() -> Self {
        Self {
            pcb_overview: None,
            tree_view_state: Default::default(),
            navigation_paths: Default::default(),
            component: Default::default(),
        }
    }

    pub fn update_pcb_overview(&mut self, pcb_overview: PcbOverview) {
        self.pcb_overview.replace(pcb_overview);
    }

    // FUTURE consider extracting the tree and navigation path creation into the planner_app, similar to `Event::RequestProjectTreeView`
    fn show_pcb_tree(&self, ui: &mut Ui) {
        let Some(pcb_overview) = self.pcb_overview.as_ref() else {
            return;
        };

        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        let mut navigation_paths = self.navigation_paths.lock().unwrap();

        let (_response, actions) = TreeView::new(ui.make_persistent_id("pcb_explorer_tree"))
            .allow_multi_selection(false)
            .show_state(
            ui,
            &mut tree_view_state,
            |tree_builder: &mut egui_ltreeview::TreeViewBuilder<'_, usize>| {

                let mut node_id = 0;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-root", {name: &pcb_overview.name})).selectable(false));
                        }),
                );
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/".to_string()));

                //
                // Configuration
                //

                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-explorer-node-configuration"));
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/configuration".to_string()));

                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-explorer-node-panel"));
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/panel".to_string()));

                //
                // Views
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-pcb-view")).selectable(false));
                        }),
                );
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/pcb".to_string()));

                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-side-top"));
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/pcb/top".to_string()));
                node_id += 1;
                tree_builder.leaf(node_id, tr!("pcb-side-bottom"));
                navigation_paths.insert(node_id, NavigationPath::new("/pcb/pcb/bottom".to_string()));

                // views
                tree_builder.close_dir();

                //
                // designs
                //

                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-designs")).selectable(false));
                        }),
                );

                for (design_index, design) in pcb_overview.designs.iter().enumerate() {
                    node_id += 1;
                    tree_builder.node(
                        NodeBuilder::dir(node_id)
                            .activatable(true)
                            .label_ui(|ui| {
                                ui.add(egui::Label::new(design.to_string()).selectable(false));
                            }),
                    );
                    navigation_paths.insert(node_id, NavigationPath::new(format!("/pcb/designs/{}", design_index).to_string()));

                    node_id += 1;
                    tree_builder.leaf(node_id, tr!("pcb-side-top"));
                    navigation_paths.insert(node_id, NavigationPath::new(format!("/pcb/designs/{}/top", design_index).to_string()));

                    node_id += 1;
                    tree_builder.leaf(node_id, tr!("pcb-side-bottom"));
                    navigation_paths.insert(node_id, NavigationPath::new(format!("/pcb/designs/{}/bottom", design_index).to_string()));

                    tree_builder.close_dir();
                }

                // designs
                tree_builder.close_dir();

                //
                // units
                //
                node_id += 1;
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(tr!("pcb-explorer-node-units")).selectable(false));
                        }),
                );

                for index in 0_u16..pcb_overview.units {
                    node_id += 1;

                    let pcb_number = index + 1;
                    let label = pcb_overview.unit_map
                        .get(&(index as PcbUnitIndex))
                        .map(|design_index| {
                            let name = &pcb_overview.designs[*design_index];
                            tr!("pcb-explorer-node-units-assignment-assigned", { pcb_number: pcb_number, design_name: name.to_string()})
                        })
                        .unwrap_or_else(|| tr!("pcb-explorer-node-units-assignment-unassigned", {pcb_number: pcb_number}).to_string());

                    tree_builder.leaf(node_id, label.to_string());
                }
                // units
                tree_builder.close_dir();

                // root
                tree_builder.close_dir();

            },
        );

        for action in actions {
            match action {
                Action::SetSelected(selection) => {
                    debug!("action, set-selected. selection: {:?}", selection);

                    // TODO handle selection
                }
                Action::Activate(activation) => {
                    debug!(
                        "action, activate. selection: {:?}, modifiers: {:?}",
                        activation.selected, activation.modifiers
                    );

                    for node_id in activation.selected {
                        if let Some(navigation_path) = navigation_paths.get(&node_id) {
                            self.component
                                .send(ExplorerTabUiCommand::Navigate(navigation_path.clone()));
                        }

                        // HACK: tree-view-dir-activate-expand-hack
                        tree_view_state.expand_node(&node_id);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn select_path(&mut self, navigation_path: &NavigationPath) {
        if let Some((node_id, _navigation_path)) = self
            .navigation_paths
            .lock()
            .unwrap()
            .iter()
            .find(|(_k, v)| (*v).eq(navigation_path))
        {
            let mut tree_view_state = self.tree_view_state.lock().unwrap();

            tree_view_state.set_selected(vec![*node_id]);
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExplorerTabUiCommand {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone)]
pub enum ExplorerTabUiAction {
    Navigate(NavigationPath),
}

#[derive(Debug, Clone, Default)]
pub struct ExplorerTabUiContext {}

impl UiComponent for ExplorerUi {
    type UiContext<'context> = ExplorerTabUiContext;
    type UiCommand = ExplorerTabUiCommand;
    type UiAction = ExplorerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if self.pcb_overview.is_some() {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.show_pcb_tree(ui);
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ExplorerTabUiCommand::Navigate(path) => {
                self.select_path(&path);

                Some(ExplorerTabUiAction::Navigate(path))
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExplorerTab {}

impl Tab for ExplorerTab {
    type Context = PcbTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("pcb-explorer-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.explorer_tab_ui, ui, &mut ExplorerTabUiContext::default());
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
