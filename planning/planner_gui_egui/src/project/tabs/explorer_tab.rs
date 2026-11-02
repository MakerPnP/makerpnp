use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

use derivative::Derivative;
use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_i18n::{tr, translate_fluent};
use egui_ltreeview::{
    Action, Activate, DirPosition, DragAndDrop, NodeBuilder, TreeView, TreeViewBuilder, TreeViewState,
};
use egui_mobius::types::Value;
use i18n::fluent_argument_helpers::args::build_fluent_args;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use planner_app::{Arg, PcbSide, PhaseReference, ProjectTreeItem, ProjectTreeView};
use tap::Tap;
use tracing::{debug, trace};
use util::path::clip_path;

use crate::i18n::conversions::pcb_side_to_i18n_key;
use crate::project::data::ProjectData;
use crate::project::tabs::ProjectTabContext;
use crate::project::{project_path_from_view_path, view_path_from_project_path};
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ExplorerTabUi {
    project_directory: PathBuf,

    #[derivative(Debug = "ignore")]
    tree_view_state: Value<TreeViewState<usize>>,

    pub component: ComponentState<ExplorerTabUiCommand>,
}

impl ExplorerTabUi {
    pub fn new(project_directory: PathBuf) -> Self {
        Self {
            project_directory,
            tree_view_state: Default::default(),
            component: Default::default(),
        }
    }

    fn show_project_tree(&self, ui: &mut Ui, graph: &Graph<ProjectTreeItem, ()>, node: NodeIndex) {
        let mut tree_view_state = self.tree_view_state.lock().unwrap();

        let (_response, actions) = TreeView::new(ui.make_persistent_id("project_explorer_tree")).show_state(
            ui,
            &mut tree_view_state,
            |builder: &mut egui_ltreeview::TreeViewBuilder<'_, usize>| {
                self.show_project_tree_inner(builder, graph, node);
            },
        );

        for action in actions {
            match action {
                Action::Activate(Activate {
                    selected,
                    modifiers,
                }) => {
                    let _ = modifiers;
                    for &node_id in &selected {
                        let item = &graph[NodeIndex::new(node_id)];
                        let path = project_path_from_view_path(&item.path);

                        self.component
                            .send(ExplorerTabUiCommand::Navigate(path));

                        // HACK: tree-view-dir-activate-expand-hack
                        tree_view_state.expand_node(&node_id);
                    }
                }
                Action::Move(mut dnd) => {
                    // FIXME there's currently a bug in egui-ltreeview 0.5.3 that causes source nodes to be duplicated.
                    dnd.source.dedup();

                    debug!(
                        "move, source: {:?}, target: {:?}, position: {:?}",
                        dnd.source, dnd.target, dnd.position
                    );
                    self.component
                        .send(ExplorerTabUiCommand::Move(dnd));
                }
                _ => {}
            }
        }
    }

    fn show_project_tree_inner(
        &self,
        tree_builder: &mut TreeViewBuilder<usize>,
        graph: &Graph<ProjectTreeItem, ()>,
        node: NodeIndex,
    ) {
        let item = &graph[node];

        fn handle_phase_loadout<'p>(
            default_key: String,
            item: &'p ProjectTreeItem,
            project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            if !item.key.eq("phase-loadout") {
                return Err(());
            }

            //
            // create a clipped load-out-source-path if possible
            //
            let Arg::String(load_out_source) = &item.args["source"] else {
                return Err(());
            };

            let load_out_source_path = PathBuf::from(load_out_source);

            let clipped_load_out_source = clip_path(project_directory.clone(), load_out_source_path, None);

            let mut item = item.clone();
            item.args
                .insert("source".to_string(), Arg::String(clipped_load_out_source));

            let item: Cow<'p, ProjectTreeItem> = Cow::Owned(item);

            Ok((default_key, item))
        }

        fn handle_unit_assignment<'p>(
            _default_key: String,
            item: &'p ProjectTreeItem,
            _project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            if !item.key.eq("unit-assignment") {
                return Err(());
            }

            fn contains_all(values: &[&String], required: &[&str]) -> bool {
                required
                    .iter()
                    .all(|item| values.iter().any(|s| s == item))
            }

            let required_keys = ["variant_name"];

            let keys: Vec<&String> = item.args.keys().collect();
            let key = if contains_all(&keys, &required_keys) {
                "project-explorer-node-unit-assignment-assigned".to_string()
            } else {
                "project-explorer-node-unit-assignment-unassigned".to_string()
            };

            Ok((key, Cow::Borrowed(item)))
        }

        fn handle_phase<'p>(
            default_key: String,
            item: &'p ProjectTreeItem,
            _project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            if !item.key.eq("phase") {
                return Err(());
            }

            let pcb_side = item
                .args
                .get("pcb_side")
                .and_then(|arg| match arg {
                    Arg::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .map(PcbSide::from_str)
                .unwrap_or(Err(()))?;

            let mut item = item.clone();
            item.args.insert(
                "pcb_side".to_string(),
                Arg::String(tr!(pcb_side_to_i18n_key(&pcb_side))),
            );

            let item: Cow<'p, ProjectTreeItem> = Cow::Owned(item);

            Ok((default_key, item))
        }

        fn default_handler<'p>(
            default_key: String,
            item: &'p ProjectTreeItem,
            _project_directory: &'_ PathBuf,
        ) -> Result<(String, Cow<'p, ProjectTreeItem>), ()> {
            Ok((default_key, Cow::Borrowed(item)))
        }

        // some items need additional processing
        let handlers = [
            handle_phase,
            handle_phase_loadout,
            handle_unit_assignment,
            default_handler,
        ];

        let default_key = format!("project-explorer-node-{}", item.key);

        let (key, item) = handlers
            .iter()
            .find_map(|handler| handler(default_key.clone(), item, &self.project_directory).ok())
            .unwrap();

        let args = build_fluent_args(&item.args);

        let label = translate_fluent(&key, &args);

        let node_id = node.index();

        let mut node_created: bool = false;
        for &neighbour in graph
            .neighbors(node)
            .collect::<Vec<_>>()
            .iter()
            .rev()
        {
            if !node_created {
                tree_builder.node(
                    NodeBuilder::dir(node_id)
                        .activatable(true)
                        .label_ui(|ui| {
                            ui.add(egui::Label::new(label.clone()).selectable(false));
                        }),
                );
                node_created = true;
            }
            self.show_project_tree_inner(tree_builder, graph, neighbour);
        }
        if node_created {
            tree_builder.close_dir();
        } else {
            tree_builder.leaf(node_id, &label);
        }
    }

    fn select_path(&mut self, project_tree: &ProjectTreeView, project_path: &NavigationPath) {
        // it's an error to be given a path without a corresponding tree view node that has the same path
        let path = view_path_from_project_path(project_path).unwrap();

        let graph = &project_tree.tree;

        if let Some(node) = graph
            .node_indices()
            .find(|&index| graph[index].path.eq(&path))
        {
            let mut tree_view_state = self.tree_view_state.lock().unwrap();

            let node_index = node.index();

            let mut selection = tree_view_state.selected().clone();
            if !selection.contains(&node_index) {
                selection.push(node_index)
            }

            tree_view_state.set_selected(selection);
        } else {
            unreachable!()
        }
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub enum ExplorerTabUiCommand {
    Navigate(NavigationPath),
    Move(#[derivative(Debug(format_with = "DragAndDropFormatter::fmt"))] DragAndDrop<usize>),
}

struct DragAndDropFormatter {}

impl DragAndDropFormatter {
    fn fmt(value: &DragAndDrop<usize>, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        f.write_fmt(format_args!(
            "DragAndDrop(source: {:?}, target: {:?}, position: {:?})",
            value.source, value.target, value.position
        ))
    }
}

#[derive(Debug, Clone)]
pub enum ExplorerTabUiAction {
    Navigate(NavigationPath),
    SetPhaseOrdering(Vec<PhaseReference>),
}

#[derive(Debug, Clone)]
pub struct ExplorerTabUiContext<'context> {
    pub data: &'context ProjectData,
}

impl UiComponent for ExplorerTabUi {
    type UiContext<'context> = ExplorerTabUiContext<'context>;
    type UiCommand = ExplorerTabUiCommand;
    type UiAction = ExplorerTabUiAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        if let Some(tree) = context.data.project_tree.get() {
            self.show_project_tree(ui, &tree.tree, NodeIndex::new(0));
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
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ExplorerTabUiCommand::Navigate(path) => {
                if let Some(project_tree) = context.data.project_tree.get() {
                    self.select_path(project_tree, &path);
                }

                Some(ExplorerTabUiAction::Navigate(path))
            }
            ExplorerTabUiCommand::Move(dnd) => {
                let Some(project_tree) = context.data.project_tree.get() else {
                    return None;
                };

                let source_nodes = dnd
                    .source
                    .iter()
                    .map(|source_id| {
                        project_tree
                            .tree
                            .node_weight(NodeIndex::new(*source_id))
                            .unwrap()
                    })
                    .collect::<Vec<_>>();

                let destination_node = project_tree
                    .tree
                    .node_weight(NodeIndex::new(dnd.target))
                    .unwrap();

                debug!(
                    "source_nodes: {:?}, destination_node: {:?}",
                    source_nodes, destination_node
                );

                let destination_node_path = project_path_from_view_path(&destination_node.path).to_string();
                debug!("destination_node_path: {:?}", destination_node_path);

                // Handle re-arranging phases
                if crate::project::tree_item::REGULAR_EXPRESSIONS
                    .phases
                    .is_match(&destination_node_path)
                {
                    let source_paths = source_nodes
                        .iter()
                        .map(|source_node| project_path_from_view_path(&source_node.path).to_string())
                        .collect::<Vec<_>>();

                    let moved_phases = source_paths
                        .iter()
                        .filter_map(|path| {
                            trace!("source_path: {:?}", path);
                            crate::project::tree_item::REGULAR_EXPRESSIONS
                                .phase
                                .captures(&path)
                        })
                        .map(|captures| {
                            let phase_reference = captures.name("phase").unwrap().as_str();
                            PhaseReference::from_raw_str(phase_reference)
                        })
                        .collect::<Vec<_>>();
                    trace!("moved_phases: {:?}", moved_phases);

                    let target_neighbour_paths = project_tree
                        .tree
                        .neighbors(NodeIndex::new(dnd.target))
                        .map(|neighbour_index| {
                            let neighbour = project_tree
                                .tree
                                .node_weight(neighbour_index)
                                .unwrap();
                            project_path_from_view_path(&neighbour.path).to_string()
                        })
                        .collect::<Vec<_>>()
                        .tap_mut(|paths| {
                            paths.reverse();
                        });

                    let all_phases = target_neighbour_paths
                        .iter()
                        .filter_map(|path| {
                            trace!("target_neighbour_path: {:?}", path);
                            crate::project::tree_item::REGULAR_EXPRESSIONS
                                .phase
                                .captures(&path)
                        })
                        .map(|captures| {
                            let phase_reference = captures.name("phase").unwrap().as_str();
                            PhaseReference::from_raw_str(phase_reference)
                        })
                        .collect::<Vec<_>>();

                    debug!("current phase ordering: {:?}", all_phases);

                    fn find_position(
                        project_tree: &ProjectTreeView,
                        index: usize,
                        phases: &mut [PhaseReference],
                    ) -> Option<usize> {
                        phases.iter().position(|phase| {
                            let node = project_tree
                                .tree
                                .node_weight(NodeIndex::new(index))
                                .unwrap();
                            let node_path = project_path_from_view_path(&node.path).to_string();
                            let captures = crate::project::tree_item::REGULAR_EXPRESSIONS
                                .phase
                                .captures(&node_path);
                            if let Some(captures) = captures {
                                let node_phase = PhaseReference::from_raw_str(captures.name("phase").unwrap().as_str());
                                return *phase == node_phase;
                            }
                            false
                        })
                    }

                    let phases = match dnd.position {
                        DirPosition::First => {
                            let mut all_phases = all_phases;
                            all_phases.retain(|phase| !moved_phases.contains(phase));
                            let mut phases = moved_phases;
                            phases.extend(all_phases);

                            phases
                        }
                        DirPosition::Last => {
                            let mut phases = all_phases;
                            phases.retain(|phase| !moved_phases.contains(phase));
                            phases.extend(moved_phases);
                            phases
                        }
                        DirPosition::After(index) => {
                            let mut phases = all_phases;
                            phases.retain(|phase| !moved_phases.contains(phase));

                            // Find the position after the specified index node
                            let insert_position = find_position(project_tree, index, &mut phases)
                                .map(|pos| pos + 1)
                                .unwrap_or(phases.len());

                            // Insert the moved phases at the determined position
                            phases.splice(insert_position..insert_position, moved_phases);

                            phases
                        }
                        DirPosition::Before(index) => {
                            let mut phases = all_phases;
                            phases.retain(|phase| !moved_phases.contains(phase));

                            // Find the position before the specified index node
                            let insert_position =
                                find_position(project_tree, index, &mut phases).unwrap_or(phases.len());

                            // Insert the moved phases at the determined position
                            phases.splice(insert_position..insert_position, moved_phases);

                            phases
                        }
                    };

                    debug!("new phase ordering: {:?}", phases);
                    return Some(ExplorerTabUiAction::SetPhaseOrdering(phases.into()));
                }

                None
            }
        }
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExplorerTab {}

impl Tab for ExplorerTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let title = tr!("project-explorer-tab-label");

        egui::widget_text::WidgetText::from(title)
    }

    fn ui<'a>(&mut self, ui: &mut Ui, _tab_key: &TabKey, context: &mut Self::Context) {
        let state = context.state.lock().unwrap();
        UiComponent::ui(&state.explorer_tab_ui, ui, &mut ExplorerTabUiContext {
            data: &state.data,
        });
    }

    fn on_close<'a>(&mut self, _tab_key: &TabKey, _context: &mut Self::Context) -> OnCloseResponse {
        OnCloseResponse::Close
    }
}
