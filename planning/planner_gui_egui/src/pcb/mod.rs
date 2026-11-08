use std::collections::HashMap;
use std::path::PathBuf;

use derivative::Derivative;
use eda_units::eda_units::dimension_unit::{
    DimensionUnitPoint2, DimensionUnitPoint2Ext, DimensionUnitVector2, DimensionUnitVector2Ext,
};
use eda_units::eda_units::unit_system::UnitSystem;
use egui::Ui;
use egui_dock::Split;
use planner_app::{
    DesignIndex, Event, ObjectPath, PanelSizing, PcbOverview, PcbSide, PcbView, PcbViewRequest, PlacementPositionUnit,
};
use regex::Regex;
use slotmap::new_key_type;
use tabs::configuration_tab::{
    ConfigurationTab, ConfigurationTabUiAction, ConfigurationTabUiCommand, ConfigurationTabUiContext, ConfigurationUi,
};
use tabs::explorer_tab::{ExplorerTab, ExplorerTabUiCommand, ExplorerTabUiContext, ExplorerUi};
use tabs::gerber_viewer_tab::{
    GerberViewerTab, GerberViewerTabUi, GerberViewerTabUiAction, GerberViewerTabUiCommand, GerberViewerTabUiContext,
};
use tracing::{debug, info, trace};

use crate::pcb::core_helper::PcbCoreHelper;
use crate::pcb::tabs::explorer_tab::ExplorerTabUiAction;
use crate::pcb::tabs::panel_tab::{PanelTab, PanelTabUi, PanelTabUiAction, PanelTabUiCommand, PanelTabUiContext};
use crate::pcb::tabs::{PcbTabAction, PcbTabContext, PcbTabUiCommand, PcbTabs};
use crate::planner_app_core::{PlannerCoreService, PlannerError};
use crate::sync::Enqueue;
use crate::sync::Value;
use crate::task::Task;
use crate::ui_commands::PcbDesignComponent;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_components::gerber_viewer_ui::{GerberViewerMode, GerberViewerUiInstanceArgs};
use crate::ui_util::NavigationPath;

pub mod tabs;

new_key_type! {
    /// A key for a pcb
    pub struct PcbKey;
}

#[derive(Debug)]
pub enum PcbAction {
    Task(PcbKey, Task<PcbAction>),
    SetModifiedState(bool),
    UiCommand(PcbUiCommand),
    RequestRepaint,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Pcb {
    #[derivative(Debug = "ignore")]
    planner_core_service: PlannerCoreService,

    pcb_ui_state: Value<PcbUiState>,

    path: PathBuf,
    modified: bool,
    /// the core's generation of the modified state last applied, see [`PlannerAction::SetModifiedState`].
    modified_generation: u64,

    pcb_overview: Option<PcbOverview>,
    panel_sizing: Option<PanelSizing>,

    pcb_tabs: Value<PcbTabs>,

    pub component: ComponentState<(PcbKey, PcbUiCommand)>,
}

impl Pcb {
    pub fn from_path(path: PathBuf, key: PcbKey, pcb_tabs: Value<PcbTabs>) -> (Self, Vec<PcbUiCommand>) {
        Self::new_inner(path, key, None, pcb_tabs, PcbUiCommand::Load)
    }

    pub fn new(
        path: PathBuf,
        key: PcbKey,
        name: String,
        units: u16,
        pcb_tabs: Value<PcbTabs>,
    ) -> (Self, Vec<PcbUiCommand>) {
        Self::new_inner(path.clone(), key, None, pcb_tabs, PcbUiCommand::Create {
            path,
            name,
            units,
        })
    }

    fn new_inner(
        path: PathBuf,
        key: PcbKey,
        name: Option<String>,
        pcb_tabs: Value<PcbTabs>,
        initial_command: PcbUiCommand,
    ) -> (Self, Vec<PcbUiCommand>) {
        debug!("Creating pcb instance from path. path: {:?}", path);

        let component: ComponentState<(PcbKey, PcbUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let pcb_ui_state = Value::new(PcbUiState::new(key, name, component_sender.clone()));

        let core_service = PlannerCoreService::new();

        let mut instance = Self {
            planner_core_service: core_service,
            pcb_ui_state,
            path,
            modified: false,
            modified_generation: 0,
            pcb_overview: None,
            panel_sizing: None,
            component,
            pcb_tabs,
        };

        let mut commands = instance.configure_tabs(key);
        commands.insert(0, initial_command);

        (instance, commands)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn configure_tabs(&mut self, key: PcbKey) -> Vec<PcbUiCommand> {
        let component_sender = self.component.sender.clone();
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();

        pcb_tabs
            .component
            .configure_mapper(component_sender, move |command| {
                (key, PcbUiCommand::TabCommand(command))
            });

        pcb_tabs.filter_map(|(_key, tab)| {
            let command = match tab {
                PcbTabKind::Explorer(_tab) => PcbUiCommand::ShowExplorer,
                PcbTabKind::Configuration(_) => PcbUiCommand::ShowConfiguration,
                PcbTabKind::Panel(_tab) => PcbUiCommand::ShowPanel,
                PcbTabKind::GerberViewer(tab) => PcbUiCommand::ShowGerberViewer(tab.args.clone()),
            };

            Some(command)
        })
    }

    pub fn show_explorer(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Explorer(_)));
        if result.is_err() {
            pcb_tabs.add_tab(PcbTabKind::Explorer(ExplorerTab::default()));
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Overview {
                path,
            },
        )))
    }

    pub fn show_configuration(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Configuration(_)));
        if result.is_err() {
            pcb_tabs.add_tab_to_leaf_or_split(
                PcbTabKind::Configuration(ConfigurationTab::default()),
                0.25,
                Split::Right,
            );
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Overview {
                path,
            },
        )))
    }

    pub fn show_panel(&mut self, path: PathBuf) -> Task<PcbAction> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        let result = pcb_tabs.show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::Panel(_)));
        if result.is_err() {
            pcb_tabs.add_tab_to_leaf_or_split(PcbTabKind::Panel(PanelTab::default()), 0.25, Split::Right);
        }

        Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
            PcbViewRequest::Panel {
                path,
            },
        )))
    }

    fn ensure_gerber_viewer(&self, key: PcbKey, args: GerberViewerUiInstanceArgs) -> bool {
        let mut state = self.pcb_ui_state.lock().unwrap();
        let mut created = false;
        let _gerber_viewer_ui_state = state
            .gerber_viewer_tab_uis
            .entry(args.clone())
            .or_insert_with(|| {
                created = true;
                debug!("ensuring gerber viewer tab ui. args: {:?}", args);
                let mut gerber_viewer_ui = GerberViewerTabUi::new(args.clone());
                gerber_viewer_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            (key, PcbUiCommand::GerberViewerTabUiCommand {
                                args: args.clone(),
                                command,
                            })
                        }
                    });

                gerber_viewer_ui
            });

        created
    }

    pub fn show_gerber_viewer(&mut self, key: PcbKey, args: GerberViewerUiInstanceArgs) -> Option<Task<PcbAction>> {
        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        pcb_tabs
            .show_tab(|candidate_tab| matches!(candidate_tab, PcbTabKind::GerberViewer(tab) if tab.args.eq(&args)))
            .inspect_err(|_| {
                pcb_tabs.add_tab_to_leaf_or_split(
                    PcbTabKind::GerberViewer(GerberViewerTab::new(args.clone())),
                    0.25,
                    Split::Right,
                );
            })
            .ok();

        match self.ensure_gerber_viewer(key, args) {
            false => None,
            true => {
                let task1 = Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                    PcbViewRequest::Overview {
                        path: self.path.clone(),
                    },
                )));
                let task2 = Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                    PcbViewRequest::Panel {
                        path: self.path.clone(),
                    },
                )));

                let tasks = vec![task1, task2];
                Some(Task::batch(tasks))
            }
        }
    }

    fn navigate(&mut self, key: &PcbKey, navigation_path: NavigationPath) -> Option<PcbAction> {
        // if the path starts with `/pcb/` then show/hide UI elements based on the path,
        debug!("Navigating. navigation_path: '{}'", navigation_path);

        #[must_use]
        fn handle_root(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/".into()) {
                // Show the configuration, in lieu of anything else.
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowConfiguration)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_configuration(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/configuration".into()) {
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowConfiguration)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_panel(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            if navigation_path.eq(&"/pcb/panel".into()) {
                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowPanel)),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            let design_pattern = Regex::new(r"^/pcb/pcb(/(?<side>(top|bottom)))?$").unwrap();
            if let Some(captures) = design_pattern.captures(&navigation_path) {
                let pcb_side: Option<PcbSide> = captures
                    .name("side")
                    .map(|value| value.as_str().parse::<PcbSide>().ok())
                    .flatten();

                let args = GerberViewerUiInstanceArgs {
                    mode: GerberViewerMode::Panel,
                    pcb_side,
                };

                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowGerberViewer(args))),
                ))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb_design(key: &PcbKey, navigation_path: &NavigationPath) -> Option<PcbAction> {
            let design_pattern = Regex::new(r"^/pcb/designs/(?<design>\d*){1}(/(?<side>(top|bottom)))?$").unwrap();
            if let Some(captures) = design_pattern.captures(&navigation_path) {
                let design_index: DesignIndex = captures
                    .name("design")
                    .unwrap()
                    .as_str()
                    .parse::<DesignIndex>()
                    .unwrap();

                let pcb_side: Option<PcbSide> = captures
                    .name("side")
                    .map(|value| value.as_str().parse::<PcbSide>().ok())
                    .flatten();

                let args = GerberViewerUiInstanceArgs {
                    mode: GerberViewerMode::Design(design_index),
                    pcb_side,
                };

                Some(PcbAction::Task(
                    *key,
                    Task::done(PcbAction::UiCommand(PcbUiCommand::ShowGerberViewer(args))),
                ))
            } else {
                None
            }
        }

        let handlers = [
            handle_root,
            handle_configuration,
            handle_panel,
            handle_pcb,
            handle_pcb_design,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(key, &navigation_path))
    }

    fn handle_panel_tab_ui_action(
        &mut self,
        key: PcbKey,
        panel_tab_ui_action: Option<PanelTabUiAction>,
    ) -> Option<PcbAction> {
        match panel_tab_ui_action {
            None => None,
            Some(PanelTabUiAction::None) => None,
            Some(PanelTabUiAction::ApplyPanelSizing(panel_sizing)) => self
                .planner_core_service
                .update(Event::ApplyPanelSizing {
                    path: self.path.clone(),
                    panel_sizing,
                })
                .when_ok(key, |_| {
                    Some(PcbUiCommand::PanelTabUiCommand(PanelTabUiCommand::PanelSizingSaved))
                }),
            Some(PanelTabUiAction::ApplyAssemblyOrientation(assembly_orientation)) => self
                .planner_core_service
                .update(Event::ApplyAssemblyOrientation {
                    path: self.path.clone(),
                    assembly_orientation,
                })
                .when_ok(key, |_| {
                    Some(PcbUiCommand::PanelTabUiCommand(
                        PanelTabUiCommand::AssemblyOrientationSaved,
                    ))
                }),
            Some(PanelTabUiAction::Task(task)) => {
                let task = task.map(|command| PcbAction::UiCommand(PcbUiCommand::PanelTabUiCommand(command)));

                Some(PcbAction::Task(key, task))
            }
            Some(PanelTabUiAction::UiCommand(command)) => {
                let context = &mut PanelTabUiContext::default();
                let panel_tab_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .panel_tab_ui
                    .update(command, context);

                let action = self.handle_panel_tab_ui_action(key, panel_tab_ui_action);

                action
            }
            Some(PanelTabUiAction::RefreshPcb) => Some(PcbAction::Task(
                key,
                Task::done(PcbAction::UiCommand(PcbUiCommand::RefreshPcb)),
            )),
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct PcbUiState {
    name: Option<String>,

    key: PcbKey,
    explorer_tab_ui: ExplorerUi,
    configuration_tab_ui: ConfigurationUi,
    gerber_viewer_tab_uis: HashMap<GerberViewerUiInstanceArgs, GerberViewerTabUi>,
    panel_tab_ui: PanelTabUi,
}

impl PcbUiState {
    pub fn new(key: PcbKey, name: Option<String>, sender: Enqueue<(PcbKey, PcbUiCommand)>) -> Self {
        let mut instance = Self {
            name,
            key,
            explorer_tab_ui: ExplorerUi::new(),
            configuration_tab_ui: ConfigurationUi::new(),
            gerber_viewer_tab_uis: HashMap::new(),
            panel_tab_ui: PanelTabUi::new(),
        };

        instance
            .explorer_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                (key, PcbUiCommand::ExplorerTabUiCommand(command))
            });

        instance
            .configuration_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                (key, PcbUiCommand::ConfigurationTabUiCommand(command))
            });

        instance
            .panel_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                (key, PcbUiCommand::PanelTabUiCommand(command))
            });

        instance
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum PcbTabKind {
    Explorer(ExplorerTab),
    Configuration(ConfigurationTab),
    GerberViewer(GerberViewerTab),
    Panel(PanelTab),
}

#[derive(Debug, Clone)]
pub enum PcbUiCommand {
    None,

    // FIXME don't care about projects, don't care about /all/ pcbs, care about *this* PCB.
    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
        generation: u64,
    },

    Create {
        path: PathBuf,
        name: String,
        units: u16,
    },
    Created {
        path: PathBuf,
    },
    Load,
    Loaded,
    Save,
    Saved,

    //
    // errors
    //
    Error(PlannerError),

    //
    // views
    //
    PcbView(PcbView),
    RequestPcbView(PcbViewRequest),

    //
    // tabs
    //
    TabCommand(PcbTabUiCommand),

    ShowExplorer,
    ExplorerTabUiCommand(ExplorerTabUiCommand),

    ShowConfiguration,
    ConfigurationTabUiCommand(ConfigurationTabUiCommand),

    ShowGerberViewer(GerberViewerUiInstanceArgs),
    GerberViewerTabUiCommand {
        args: GerberViewerUiInstanceArgs,
        command: GerberViewerTabUiCommand,
    },

    ShowPanel,
    PanelTabUiCommand(PanelTabUiCommand),
    LocateComponent {
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
    /// A design's placement, located on each of the PCB's viewers of the design (`UNITASG-6`).
    LocateDesignComponent(PcbDesignComponent),
    RefreshPcb,
}

/// The placement's position, as the EDA tool exported it, on the design's gerbers: the design's placement offset,
/// usually negative, added, and its gerber offset, usually negative, subtracted, to apply it again.
fn on_design_gerbers(
    panel_sizing: &PanelSizing,
    design_index: usize,
    mut design_position: PlacementPositionUnit,
) -> PlacementPositionUnit {
    let design_sizing = &panel_sizing.design_sizings[design_index];
    trace!("design_sizing: {:?}", design_sizing);

    // FIXME hard-coded use of UnitSystem::Millimeters
    let placement_offset = DimensionUnitPoint2::new_dim_f64(
        design_sizing.placement_offset.x,
        design_sizing.placement_offset.y,
        UnitSystem::Millimeters,
    );
    let gerber_offset = DimensionUnitPoint2::new_dim_f64(
        design_sizing.gerber_offset.x,
        design_sizing.gerber_offset.y,
        UnitSystem::Millimeters,
    );

    let eda_offset = placement_offset - gerber_offset;
    design_position.coords += eda_offset;

    trace!("adjusted design_position: {:?}", design_position);
    design_position
}

pub struct PcbContext {
    pub key: PcbKey,
}

impl UiComponent for Pcb {
    type UiContext<'context> = PcbContext;
    type UiCommand = (PcbKey, PcbUiCommand);
    type UiAction = PcbAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps when using taffy
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        //
        // Tabs
        //
        let mut tab_context = PcbTabContext {
            state: self.pcb_ui_state.clone(),
        };

        let mut pcb_tabs = self.pcb_tabs.lock().unwrap();
        pcb_tabs.cleanup_tabs(&mut tab_context);
        pcb_tabs.ui(ui, &mut tab_context);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("Pcb command. command: {:?}", command);
        let (key, command) = command;

        match command {
            PcbUiCommand::None => None,
            PcbUiCommand::Create {
                path,
                name,
                units,
            } => self
                .planner_core_service
                .update(Event::CreatePcb {
                    name,
                    units,
                    path: path.clone(),
                    unit_map: None,
                })
                .when_ok(key, |_| {
                    Some(PcbUiCommand::Created {
                        path,
                    })
                }),
            PcbUiCommand::Created {
                path,
            } => {
                let task1 = self.show_explorer(path.clone());
                let task2 = self.show_configuration(path);
                let tasks = vec![task1, task2];
                Some(PcbAction::Task(key, Task::batch(tasks)))
            }
            PcbUiCommand::Load => {
                let path = self.path.clone();

                self.planner_core_service
                    .update(Event::LoadPcb {
                        path,
                    })
                    .when_ok(key, |_| Some(PcbUiCommand::Loaded))
            }
            PcbUiCommand::Loaded => {
                let task1 = self.show_explorer(self.path.clone());
                let task2 = self.show_configuration(self.path.clone());
                let tasks = vec![task1, task2];
                Some(PcbAction::Task(key, Task::batch(tasks)))
            }
            PcbUiCommand::Save => {
                let path = self.path.clone();

                self.planner_core_service
                    .update(Event::SavePcb {
                        path,
                    })
                    .when_ok(key, |_| Some(PcbUiCommand::Saved))
            }
            PcbUiCommand::Saved => None,
            PcbUiCommand::Error(_error) => {
                // TODO show a dialog for PCB errors
                None
            }
            PcbUiCommand::RefreshPcb => {
                let task1 = Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                    PcbViewRequest::Overview {
                        path: self.path.clone(),
                    },
                )));
                let task2 = Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                    PcbViewRequest::Panel {
                        path: self.path.clone(),
                    },
                )));
                let tasks = vec![task1, task2];

                Some(PcbAction::Task(key, Task::batch(tasks)))
            }
            PcbUiCommand::PcbView(view) => {
                match view {
                    PcbView::PcbOverview(pcb_overview) => {
                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();

                        for gerber_viewer_ui in pcb_ui_state
                            .gerber_viewer_tab_uis
                            .values_mut()
                        {
                            gerber_viewer_ui.update_pcb_overview(pcb_overview.clone());
                        }

                        pcb_ui_state
                            .configuration_tab_ui
                            .update_pcb_overview(pcb_overview.clone());
                        pcb_ui_state
                            .explorer_tab_ui
                            .update_pcb_overview(pcb_overview.clone());
                        pcb_ui_state
                            .panel_tab_ui
                            .update_pcb_overview(pcb_overview.clone());

                        self.pcb_overview = Some(pcb_overview);
                    }
                    PcbView::PanelSizing(panel_sizing) => {
                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();

                        pcb_ui_state
                            .panel_tab_ui
                            .update_panel_sizing(panel_sizing.clone());

                        for gerber_viewer_ui in pcb_ui_state
                            .gerber_viewer_tab_uis
                            .values_mut()
                        {
                            gerber_viewer_ui.update_panel_sizing(panel_sizing.clone());
                        }

                        self.panel_sizing = Some(panel_sizing);
                    }
                }
                None
            }
            PcbUiCommand::SetModifiedState {
                pcbs_modified,
                generation,
                ..
            } => {
                if generation < self.modified_generation {
                    debug!(
                        "ignoring an older modified state. generation: {}, applied: {}",
                        generation, self.modified_generation
                    );
                    return None;
                }
                self.modified_generation = generation;
                // FIXME we want to know if *THIS* pcb is modified, not any pcb.
                self.modified = pcbs_modified;
                Some(PcbAction::SetModifiedState(pcbs_modified))
            }
            PcbUiCommand::RequestPcbView(view_request) => {
                let event = match view_request {
                    PcbViewRequest::Overview {
                        path,
                    } => Some(Event::RequestPcbOverviewView {
                        path,
                    }),
                    PcbViewRequest::Panel {
                        path,
                    } => Some(Event::RequestPcbPanelSizingView {
                        path,
                    }),
                };

                // TODO remove the 'if let'
                if let Some(event) = event {
                    self.planner_core_service
                        .update(event)
                        .when_ok(key, |_| None)
                } else {
                    None
                }
            }

            //
            // tabs
            //
            PcbUiCommand::ShowExplorer => {
                let task = self.show_explorer(self.path.clone());
                Some(PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowConfiguration => {
                let task = self.show_configuration(self.path.clone());
                Some(PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowGerberViewer(args) => {
                let task = self.show_gerber_viewer(key, args);

                task.map(|task| PcbAction::Task(key, task))
            }
            PcbUiCommand::ShowPanel => {
                let task = self.show_panel(self.path.clone());
                Some(PcbAction::Task(key, task))
            }
            PcbUiCommand::TabCommand(tab_command) => {
                let mut pcb_tabs = self.pcb_tabs.lock().unwrap();

                let mut tab_context = PcbTabContext {
                    state: self.pcb_ui_state.clone(),
                };

                let action = pcb_tabs.update(tab_command, &mut tab_context);
                match action {
                    None => {}
                    Some(PcbTabAction::None) => {}
                }
                None
            }
            PcbUiCommand::ExplorerTabUiCommand(command) => {
                let context = &mut ExplorerTabUiContext::default();
                let explorer_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .explorer_tab_ui
                    .update(command, context);
                match explorer_ui_action {
                    Some(ExplorerTabUiAction::Navigate(path)) => self.navigate(&key, path),
                    None => None,
                }
            }
            PcbUiCommand::ConfigurationTabUiCommand(command) => {
                let context = &mut ConfigurationTabUiContext::default();
                let configuration_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .configuration_tab_ui
                    .update(command, context);
                match configuration_ui_action {
                    None => None,
                    Some(ConfigurationTabUiAction::None) => None,

                    //
                    // form
                    //
                    Some(ConfigurationTabUiAction::Reset) => self
                        .planner_core_service
                        .update(Event::RequestPcbOverviewView {
                            path: self.path.clone(),
                        })
                        .when_ok(key, |_| None),
                    Some(ConfigurationTabUiAction::Apply(args)) => {
                        self.panel_sizing.take();
                        self.pcb_overview.take();

                        let mut pcb_ui_state = self.pcb_ui_state.lock().unwrap();
                        pcb_ui_state.panel_tab_ui.reset();
                        pcb_ui_state
                            .configuration_tab_ui
                            .reset();

                        match self
                            .planner_core_service
                            .update(Event::ApplyPcbUnitConfiguration {
                                path: self.path.clone(),
                                units: args.units,
                                gerber_offset: args.gerber_offset,
                                designs: args.designs,
                                unit_map: args.unit_map,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![
                                    Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                                        PcbViewRequest::Overview {
                                            path: self.path.clone(),
                                        },
                                    ))),
                                    Task::done(PcbAction::UiCommand(PcbUiCommand::RequestPcbView(
                                        PcbViewRequest::Panel {
                                            path: self.path.clone(),
                                        },
                                    ))),
                                ];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }

                    //
                    // gerber file management
                    //
                    Some(ConfigurationTabUiAction::UpdateGerberFiles {
                        path,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(Event::AddGerberFiles {
                                path: path.clone(),
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![Task::done(PcbAction::UiCommand(
                                    PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                        path,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(ConfigurationTabUiAction::RemoveGerberFiles {
                        path,
                        design,
                        files,
                    }) => {
                        match self
                            .planner_core_service
                            .update(Event::RemoveGerberFiles {
                                path: path.clone(),
                                design,
                                files,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![Task::done(PcbAction::UiCommand(
                                    PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                        path,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(ConfigurationTabUiAction::RefreshGerberFiles {
                        path,
                        design,
                    }) => {
                        match self
                            .planner_core_service
                            .update(Event::RefreshGerberFiles {
                                path: path.clone(),
                                design,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<_>>();

                                let additional_tasks = vec![Task::done(PcbAction::UiCommand(
                                    PcbUiCommand::RequestPcbView(PcbViewRequest::Overview {
                                        path,
                                    }),
                                ))];
                                tasks.extend(additional_tasks);

                                Some(PcbAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                    Some(ConfigurationTabUiAction::ApplyGerberFileFunctions {
                        path,
                        file_functions,
                    }) => self
                        .planner_core_service
                        .update(Event::ApplyGerberFileFunctions {
                            path,
                            file_functions,
                        })
                        .when_ok(key, |_| Some(PcbUiCommand::RefreshPcb)),
                }
            }
            PcbUiCommand::PanelTabUiCommand(command) => {
                let context = &mut PanelTabUiContext::default();
                let panel_tab_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .panel_tab_ui
                    .update(command, context);

                self.handle_panel_tab_ui_action(key, panel_tab_ui_action)
            }
            PcbUiCommand::GerberViewerTabUiCommand {
                args,
                command,
            } => {
                let context = &mut GerberViewerTabUiContext {
                    args,
                };

                let gerber_viewer_ui_action = self
                    .pcb_ui_state
                    .lock()
                    .unwrap()
                    .gerber_viewer_tab_uis
                    .get_mut(&context.args)
                    .unwrap()
                    .update(command, context);

                match gerber_viewer_ui_action {
                    Some(GerberViewerTabUiAction::None) => None,
                    None => None,
                }
            }
            PcbUiCommand::LocateComponent {
                object_path,
                pcb_side,
                mut design_position,
                mut unit_position,
            } => {
                let tabs = self.pcb_tabs.lock().unwrap();

                trace!(
                    "Locate component. pcb: {:?}, object_path: {:?}, pcb_side: {:?}, design_position: {:?}, unit_position: {:?}",
                    self.path, object_path, pcb_side, design_position, unit_position
                );

                if let (Some(pcb_overview), Some(panel_sizing)) = (&self.pcb_overview, &self.panel_sizing) {
                    let (pcb_number, unit_number) = object_path
                        .pcb_instance_and_unit()
                        .unwrap();
                    let (_pcb_index, unit_index) = (pcb_number - 1, unit_number - 1);

                    if let Some(&design_index) = pcb_overview.unit_map.get(&unit_index) {
                        design_position = on_design_gerbers(panel_sizing, design_index, design_position);

                        // Unit coordinates are correct for PnP machine, but may not match the panel gerbers.
                        // Add the panel gerber offset
                        // FUTURE support panel rotation, currently users should crate PCB/panel definitions that match their panel gerbers.
                        // FIXME hard-coded use of UnitSystem::Millimeters
                        let panel_gerber_offset = DimensionUnitVector2::new_dim_f64(
                            pcb_overview.gerber_offset.x,
                            pcb_overview.gerber_offset.y,
                            UnitSystem::Millimeters,
                        );

                        unit_position.coords += panel_gerber_offset;

                        trace!("adjusted unit_position: {:?}", unit_position);

                        let tab_keys = tabs.filter_map(|(candidate_tab_key, candidate_tab)| match candidate_tab {
                            PcbTabKind::GerberViewer(gerber_viewer_tab) => match gerber_viewer_tab.args.mode {
                                GerberViewerMode::Design(candidate_index) if candidate_index.eq(&design_index) => {
                                    Some(candidate_tab_key.clone())
                                }
                                GerberViewerMode::Panel => Some(candidate_tab_key.clone()),
                                _ => None,
                            },
                            _ => None,
                        });

                        let pcb_ui_state = self.pcb_ui_state.lock().unwrap();

                        for tab_key in tab_keys {
                            tabs.with_tab_mut(&tab_key, |tab| {
                                let PcbTabKind::GerberViewer(tab) = tab else { return };

                                if let Some(gerber_viewer_ui) = pcb_ui_state
                                    .gerber_viewer_tab_uis
                                    .get(&tab.args)
                                {
                                    gerber_viewer_ui
                                        .component
                                        .send(GerberViewerTabUiCommand::LocateComponent {
                                            object_path: object_path.clone(),
                                            pcb_side,
                                            design_position,
                                            unit_position,
                                        })
                                }
                            })
                        }
                    }
                }

                None
            }
            // `UNITASG-6`, on each viewer of the design, not the panel's, as the placement isn't of one unit.
            PcbUiCommand::LocateDesignComponent(PcbDesignComponent {
                pcb_file,
                design_name,
                pcb_side,
                design_position,
            }) => {
                info!(
                    "Locating design component on PCB. pcb: '{}', design: '{}', pcb_side: {:?}, design_position: {:?}",
                    pcb_file.display(),
                    design_name,
                    pcb_side,
                    design_position
                );

                let (Some(pcb_overview), Some(panel_sizing)) = (&self.pcb_overview, &self.panel_sizing) else {
                    return None;
                };
                let Some(design_index) = pcb_overview
                    .designs
                    .iter()
                    .position(|candidate| candidate.to_string() == design_name)
                else {
                    debug!(
                        "Design not found. pcb: '{}', design: '{}'",
                        pcb_file.display(),
                        design_name
                    );
                    return None;
                };
                let design_position = on_design_gerbers(panel_sizing, design_index, design_position);

                let tabs = self.pcb_tabs.lock().unwrap();
                let tab_keys = tabs.filter_map(|(candidate_tab_key, candidate_tab)| match candidate_tab {
                    PcbTabKind::GerberViewer(gerber_viewer_tab) => match gerber_viewer_tab.args.mode {
                        GerberViewerMode::Design(candidate_index) if candidate_index.eq(&design_index) => {
                            Some(candidate_tab_key.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                });

                let pcb_ui_state = self.pcb_ui_state.lock().unwrap();
                for tab_key in tab_keys {
                    tabs.with_tab_mut(&tab_key, |tab| {
                        let PcbTabKind::GerberViewer(tab) = tab else { return };

                        if let Some(gerber_viewer_ui) = pcb_ui_state
                            .gerber_viewer_tab_uis
                            .get(&tab.args)
                        {
                            gerber_viewer_ui
                                .component
                                .send(GerberViewerTabUiCommand::LocateDesignComponent {
                                    pcb_side,
                                    design_position,
                                })
                        }
                    })
                }

                None
            }
        }
    }
}

mod core_helper {
    use tracing::warn;

    use crate::pcb::{PcbAction, PcbKey, PcbUiCommand};
    use crate::planner_app_core::{PlannerAction, PlannerError};
    use crate::task::Task;

    #[must_use]
    fn when_ok_inner<F>(
        result: Result<Vec<PlannerAction>, PlannerError>,
        project_key: PcbKey,
        f: F,
    ) -> Option<PcbAction>
    where
        F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
    {
        match result {
            Ok(actions) => {
                let mut tasks = vec![];
                let effect_tasks: Vec<Task<PcbAction>> = actions
                    .into_iter()
                    .map(|planner_action| {
                        let project_action = into_project_action(planner_action);
                        Task::done(project_action)
                    })
                    .collect();

                tasks.extend(effect_tasks);

                if let Some(command) = f(&mut tasks) {
                    let final_task = Task::done(PcbAction::UiCommand(command));
                    tasks.push(final_task);
                }

                let action = PcbAction::Task(project_key, Task::batch(tasks));

                Some(action)
            }
            Err(error) => Some(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_actions_inner(result: Result<Vec<PlannerAction>, PlannerError>) -> Result<Vec<PcbAction>, PcbAction> {
        match result {
            Ok(actions) => Ok(actions
                .into_iter()
                .map(into_project_action)
                .collect()),
            Err(error) => Err(PcbAction::UiCommand(PcbUiCommand::Error(error))),
        }
    }

    fn into_project_action(action: PlannerAction) -> PcbAction {
        match action {
            PlannerAction::SetModifiedState {
                project_modified,
                pcbs_modified,
                generation,
                ..
            } => PcbAction::UiCommand(PcbUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
                generation,
            }),
            PlannerAction::ProjectView(_project_view) => {
                warn!("pcb received project view action. ignoring.");
                PcbAction::UiCommand(PcbUiCommand::None)
            }
            PlannerAction::PcbView(pcb_view) => PcbAction::UiCommand(PcbUiCommand::PcbView(pcb_view)),
            PlannerAction::ProcessView(_process_view) => {
                warn!("pcb received process view action. ignoring.");
                PcbAction::UiCommand(PcbUiCommand::None)
            }
        }
    }

    pub trait PcbCoreHelper {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction>;
        fn when_ok<F>(self, pcb_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>;
    }

    impl PcbCoreHelper for Result<Vec<PlannerAction>, PlannerError> {
        fn into_actions(self) -> Result<Vec<PcbAction>, PcbAction> {
            into_actions_inner(self)
        }

        fn when_ok<F>(self, pcb_key: PcbKey, f: F) -> Option<PcbAction>
        where
            F: FnOnce(&mut Vec<Task<PcbAction>>) -> Option<PcbUiCommand>,
        {
            when_ok_inner(self, pcb_key, f)
        }
    }
}

pub(crate) fn make_tabs(key: PcbKey) -> Value<PcbTabs> {
    debug!("Initializing pcb tabs for tab. key: {:?}", key);

    Value::new(PcbTabs::default())
}
