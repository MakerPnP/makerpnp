use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use derivative::Derivative;
use egui::Ui;
use egui_dock::Split;
use egui_i18n::tr;
use egui_mobius::types::{Enqueue, Value, ValueGuard};
use planner_app::{
    AddOrRemoveAction, Event, FileReference, LibraryConfig, LoadOutSource, ObjectPath, PcbSide, PcbUnitIndex, PcbView,
    PcbViewRequest, PhaseOverview, PhaseReference, PlacementOperation, PlacementPositionUnit, PlacementState,
    PlacementStatus, ProcessReference, ProjectOverview, ProjectView, ProjectViewRequest, Reference, SetOrClearAction,
};
use regex::Regex;
use slotmap::new_key_type;
use tabs::explorer_tab::{ExplorerTab, ExplorerTabUi, ExplorerTabUiAction, ExplorerTabUiCommand, ExplorerTabUiContext};
use tabs::load_out_tab::{LoadOutTab, LoadOutTabUi, LoadOutTabUiAction, LoadOutTabUiCommand, LoadOutTabUiContext};
use tabs::overview_tab::{OverviewTab, OverviewTabUi, OverviewTabUiAction, OverviewTabUiCommand, OverviewTabUiContext};
use tabs::parts_tab::{PartsTab, PartsTabUi, PartsTabUiAction, PartsTabUiCommand, PartsTabUiContext};
use tabs::pcb_tab::{PcbTab, PcbTabUi, PcbTabUiAction, PcbTabUiCommand, PcbTabUiContext};
use tabs::phase_tab::{PhaseTab, PhaseTabUi, PhaseTabUiAction, PhaseTabUiCommand, PhaseTabUiContext};
use tabs::placements_tab::{
    PlacementsTab, PlacementsTabUi, PlacementsTabUiAction, PlacementsTabUiCommand, PlacementsTabUiContext,
};
use tabs::unit_assignments_tab::{
    UnitAssignmentsTab, UnitAssignmentsTabUi, UnitAssignmentsTabUiAction, UnitAssignmentsTabUiCommand,
    UnitAssignmentsTabUiContext, UpdateUnitAssignmentsArgs,
};
use tracing::{debug, error, info, trace};

use crate::file_picker::Picker;
use crate::planner_app_core::{PlannerCoreService, PlannerError};
use crate::project::core_helper::ProjectCoreHelper;
use crate::project::dialogs::add_phase::{AddPhaseModal, AddPhaseModalAction, AddPhaseModalUiCommand};
use crate::project::dialogs::package_sources::{
    PackageSourcesModal, PackageSourcesModalAction, PackageSourcesModalUiCommand,
};
use crate::project::tabs::issues_tab::{
    IssuesTab, IssuesTabUi, IssuesTabUiAction, IssuesTabUiCommand, IssuesTabUiContext,
};
use crate::project::tabs::parts_tab::PartsTabUiApplyAction;
use crate::project::tabs::placements_tab::PlacementsTabUiApplyAction;
use crate::project::tabs::process_tab::{
    ProcessTab, ProcessTabUi, ProcessTabUiAction, ProcessTabUiCommand, ProcessTabUiContext,
};
use crate::project::tabs::{ProjectTabAction, ProjectTabContext, ProjectTabUiCommand, ProjectTabs};
use crate::project::toolbar::{ProjectToolbar, ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

//
// other modules
//
mod dialogs;
mod process;
mod tables;
pub mod tabs;
mod toolbar;

new_key_type! {
    /// A key for a project
    pub struct ProjectKey;
}

#[derive(Debug)]
pub enum ProjectAction {
    Task(ProjectKey, Task<ProjectAction>),
    SetModifiedState(bool),
    UiCommand(ProjectUiCommand),
    ShowPcb(PathBuf),
    RequestRepaint,
    LocateComponent {
        pcb_file: PathBuf,
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Project {
    #[derivative(Debug = "ignore")]
    planner_core_service: PlannerCoreService,
    path: PathBuf,
    project_ui_state: Value<ProjectUiState>,

    modified: bool,
    pcbs_modified: bool,

    /// list of errors to show
    errors: Vec<(chrono::DateTime<chrono::Utc>, String)>,

    /// initially empty until the OverviewView has been received and processed.
    processes: Vec<ProcessReference>,

    /// initially empty until the OverviewView has been received and processed.
    library_config: Option<LibraryConfig>,

    /// initially empty until the OverviewView has been received and processed.
    pcbs: Vec<PathBuf>,

    /// initially empty, requires fetching the PhaseOverviewView for each phase before it can be used.
    phases: Vec<PhaseOverview>,

    project_tabs: Value<ProjectTabs>,

    toolbar: ProjectToolbar,

    pub component: ComponentState<(ProjectKey, ProjectUiCommand)>,

    add_phase_modal: Option<AddPhaseModal>,
    package_sources_modal: Option<PackageSourcesModal>,

    file_picker: Value<Picker>,
}

// FUTURE consider moving this into the planner app core itself, so it can be re-used by all apps using the same core.
pub mod tree_item {
    use std::sync::LazyLock;

    use regex::Regex;

    pub const PHASES: &str = r"^/project/phases$";
    pub const PHASE: &str = r"^/project/phases/(?<phase>[^/]*){1}$";
    pub const PHASE_LOADOUT: &str = r"^/project/phases/(?<phase>[^/]*){1}/loadout$";

    pub const PCB: &str = r"^/project/pcbs/(?<pcb>[0-9]?){1}$";
    pub const PCB_UNIT: &str = r"^/project/pcbs/(?<pcb>[0-9]?){1}/units(?:.*)?$";
    pub const PROCESS: &str = r"^/project/processes/(?<process>[^/]*){1}$";

    pub struct RegularExpressions {
        pub phases: Regex,
        pub phase: Regex,
        pub phase_loadout: Regex,
        pub pcb: Regex,
        pub pcb_unit: Regex,
        pub process: Regex,
    }

    impl Default for RegularExpressions {
        fn default() -> Self {
            Self {
                phases: Regex::new(PHASES).unwrap(),
                phase: Regex::new(PHASE).unwrap(),
                phase_loadout: Regex::new(PHASE_LOADOUT).unwrap(),
                pcb: Regex::new(PCB).unwrap(),
                pcb_unit: Regex::new(PCB_UNIT).unwrap(),
                process: Regex::new(PROCESS).unwrap(),
            }
        }
    }

    pub static REGULAR_EXPRESSIONS: LazyLock<RegularExpressions> = LazyLock::new(|| RegularExpressions::default());
}

impl Project {
    pub fn from_path(
        path: PathBuf,
        key: ProjectKey,
        project_tabs: Value<ProjectTabs>,
    ) -> (Self, Vec<ProjectUiCommand>) {
        Self::new_inner(path, key, None, project_tabs, ProjectUiCommand::Load)
    }

    pub fn new(
        name: String,
        path: PathBuf,
        key: ProjectKey,
        project_tabs: Value<ProjectTabs>,
    ) -> (Self, Vec<ProjectUiCommand>) {
        Self::new_inner(path, key, Some(name), project_tabs, ProjectUiCommand::Create)
    }

    fn new_inner(
        path: PathBuf,
        key: ProjectKey,
        name: Option<String>,
        project_tabs: Value<ProjectTabs>,
        initial_command: ProjectUiCommand,
    ) -> (Self, Vec<ProjectUiCommand>) {
        debug!("Creating project instance from path. path: {}", &path.display());

        let component: ComponentState<(ProjectKey, ProjectUiCommand)> = ComponentState::default();
        let component_sender = component.sender.clone();

        let mut toolbar = ProjectToolbar::default();
        toolbar
            .component
            .configure_mapper(component_sender.clone(), move |command| {
                trace!("project toolbar mapper. command: {:?}", command);
                (key, ProjectUiCommand::ToolbarCommand(command))
            });

        let project_directory = path.parent().unwrap().to_path_buf();
        let project_ui_state = Value::new(ProjectUiState::new(
            key,
            project_directory,
            name,
            component_sender.clone(),
        ));

        let core_service = PlannerCoreService::new();
        let mut instance = Self {
            path,
            planner_core_service: core_service,
            project_ui_state,
            modified: false,
            pcbs_modified: false,
            pcbs: Default::default(),
            errors: Default::default(),
            processes: Default::default(),
            library_config: None,
            phases: Default::default(),
            project_tabs,
            toolbar,
            component,
            add_phase_modal: None,
            package_sources_modal: None,
            file_picker: Default::default(),
        };

        let mut commands = instance.configure_tabs(key);
        commands.insert(0, initial_command);

        (instance, commands)
    }

    pub fn tabs(&self) -> Value<ProjectTabs> {
        self.project_tabs.clone()
    }

    #[must_use]
    pub fn configure_tabs(&mut self, key: ProjectKey) -> Vec<ProjectUiCommand> {
        let component_sender = self.component.sender.clone();

        debug!("Configuring tabs component for project for tab. key: {:?}", key);
        let mut project_tabs = self.project_tabs.lock().unwrap();
        project_tabs
            .component
            .configure_mapper(component_sender.clone(), move |command| {
                trace!("project inner-tab mapper. command: {:?}", command);
                (key, ProjectUiCommand::TabCommand(command))
            });

        //
        // when the app is restored, tabs will be present, but the `project_ui_state` won't be contain the correct
        // state, so issue commands to restore the state
        //

        project_tabs.filter_map(|(_key, tab)| {
            let command = match tab {
                ProjectTabKind::Explorer(_tab) => ProjectUiCommand::ShowExplorer,
                ProjectTabKind::Issues(_tab) => ProjectUiCommand::ShowIssues,
                ProjectTabKind::LoadOut(tab) => ProjectUiCommand::ShowPhaseLoadout {
                    phase: tab.phase.clone(),
                },
                ProjectTabKind::Overview(_tab) => ProjectUiCommand::ShowOverview,
                ProjectTabKind::Parts(_tab) => ProjectUiCommand::ShowParts,
                ProjectTabKind::Pcb(tab) => ProjectUiCommand::ShowPcb(tab.pcb_index),
                ProjectTabKind::Phase(tab) => ProjectUiCommand::ShowPhase(tab.phase.clone()),
                ProjectTabKind::Placements(_tab) => ProjectUiCommand::ShowPlacements,
                ProjectTabKind::Process(tab) => ProjectUiCommand::ShowProcess(tab.process.clone()),
                ProjectTabKind::UnitAssignments(tab) => ProjectUiCommand::ShowPcbUnitAssignments(tab.pcb_index),
            };

            Some(command)
        })
    }

    pub fn show_explorer(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Explorer(_)));
        if result.is_err() {
            project_tabs.add_tab(ProjectTabKind::Explorer(ExplorerTab::default()));
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
            ProjectViewRequest::ProjectTree,
        )))
    }

    pub fn show_issues(&mut self) -> Vec<Task<ProjectAction>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Issues(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Issues(IssuesTab::default()), 0.25, Split::Right);
        }

        let tasks = vec![Task::done(ProjectAction::UiCommand(
            ProjectUiCommand::RequestProjectView(ProjectViewRequest::ProjectReport),
        ))];

        tasks
    }

    pub fn show_overview(&mut self) -> Vec<Task<ProjectAction>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Overview(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Overview(OverviewTab::default()), 0.25, Split::Right);
        }

        let tasks = vec![
            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                ProjectViewRequest::Overview,
            ))),
            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                ProjectViewRequest::Phases,
            ))),
        ];

        tasks
    }

    pub fn show_parts(&mut self) -> Task<ProjectAction> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Parts(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Parts(PartsTab::default()), 0.25, Split::Right);
        }

        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
            ProjectViewRequest::Parts,
        )))
    }

    pub fn show_placements(&mut self) -> Vec<Task<ProjectAction>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Placements(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(
                ProjectTabKind::Placements(PlacementsTab::default()),
                0.25,
                Split::Right,
            );
        }

        let tasks = vec![
            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                ProjectViewRequest::Phases,
            ))),
            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                ProjectViewRequest::Placements,
            ))),
        ];

        tasks
    }

    pub fn show_phase(&mut self, key: ProjectKey, phase: PhaseReference) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PhaseTab::new(phase.clone());

        project_tabs
            .show_tab(
                |candidate_tab_kind| matches!(candidate_tab_kind, ProjectTabKind::Phase(candidate_tab) if candidate_tab.eq(&tab)),
            )
            .inspect(|tab_key| {
                debug!("showing existing phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .inspect_err(|_| {
                let tab_key = project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Phase(tab), 0.25, Split::Right);
                debug!("adding phase tab. phase: {:?}, tab_key: {:?}", phase, tab_key);
            })
            .ok();

        match self.ensure_phase(key, &phase) {
            false => None,
            true => Some(vec![
                Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::PhaseOverview {
                        phase: phase.clone(),
                    },
                ))),
                Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::PhasePlacements {
                        phase: phase.clone(),
                    },
                ))),
            ]),
        }
    }

    pub fn show_process(&mut self, key: ProjectKey, process: ProcessReference) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = ProcessTab::new(process.clone());

        project_tabs
            .show_tab(
                |candidate_tab_kind| matches!(candidate_tab_kind, ProjectTabKind::Process(candidate_tab) if candidate_tab.eq(&tab)),
            )
            .inspect(|tab_key| {
                debug!("showing existing process tab. process: {:?}, tab_key: {:?}", process, tab_key);
            })
            .inspect_err(|_| {
                let tab_key = project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Process(tab), 0.25, Split::Right);
                debug!("adding process tab. process: {:?}, tab_key: {:?}", process, tab_key);
            })
            .ok();

        match self.ensure_process(key, &process) {
            false => None,
            true => Some(vec![Task::done(ProjectAction::UiCommand(
                ProjectUiCommand::RequestProjectView(ProjectViewRequest::ProcessDefinition {
                    process: process.clone(),
                }),
            ))]),
        }
    }

    pub fn show_loadout(
        &self,
        key: ProjectKey,
        phase: Reference,
        load_out_source: &LoadOutSource,
    ) -> Option<Task<ProjectAction>> {
        let project_directory = self.path.parent().unwrap();

        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = LoadOutTab::new(project_directory.into(), phase.clone(), load_out_source.clone());

        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::LoadOut(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key| {
                debug!("showing existing load-out tab. load_out_source: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .inspect_err(|_| {
                let tab_key = project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::LoadOut(tab), 0.25, Split::Right);
                debug!("adding load-out tab. load_out_source: {:?}, tab_key: {:?}", load_out_source, tab_key);
            })
            .ok();

        match self.ensure_load_out(key, phase.clone(), load_out_source) {
            false => None,
            true => Some(Task::done(ProjectAction::UiCommand(
                ProjectUiCommand::RequestProjectView(ProjectViewRequest::PhaseLoadOut {
                    phase,
                }),
            ))),
        }
    }

    pub fn show_pcb(&mut self, key: ProjectKey, pcb_index: u16) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = PcbTab::new(pcb_index);

        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::Pcb(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key|{
                debug!("showing existing pcb tab. pcb: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .inspect_err(|_|{
                let tab_key = project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Pcb(tab), 0.25, Split::Right);
                debug!("adding pcb tab. pcb_index: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .ok();

        match self.ensure_pcb(key, pcb_index) {
            false => None,
            true => Some(vec![Task::done(ProjectAction::UiCommand(
                ProjectUiCommand::RequestProjectView(ProjectViewRequest::PcbOverview {
                    pcb: pcb_index.clone(),
                }),
            ))]),
        }
    }

    pub fn show_unit_assignments(&mut self, key: ProjectKey, pcb_index: u16) -> Option<Vec<Task<ProjectAction>>> {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let tab = UnitAssignmentsTab::new(pcb_index);

        project_tabs
            .show_tab(|candidate_tab_kind| {
                matches!(candidate_tab_kind, ProjectTabKind::UnitAssignments(candidate_tab) if candidate_tab.eq(&tab))
            })
            .inspect(|tab_key|{
                debug!("showing existing unit assignments tab. pcb: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .inspect_err(|_|{
                let tab_key = project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::UnitAssignments(tab), 0.25, Split::Right);
                debug!("adding unit assignments tab. pcb_index: {:?}, tab_key: {:?}", pcb_index, tab_key);
            })
            .ok();

        match self.ensure_unit_assignments(key, pcb_index) {
            false => None,
            true => Some(vec![
                Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::PcbOverview {
                        pcb: pcb_index,
                    },
                ))),
                Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::PcbUnitAssignments {
                        pcb: pcb_index,
                    },
                ))),
            ]),
        }
    }

    fn ensure_phase(&self, key: ProjectKey, phase: &Reference) -> bool {
        let phase = phase.clone();
        let mut state = self.project_ui_state.lock().unwrap();
        let mut created = false;
        let _phase_state = state
            .phases_tab_uis
            .entry(phase.clone())
            .or_insert_with(|| {
                created = true;
                debug!("ensuring phase ui. phase: {:?}", phase);
                let mut phase_ui = PhaseTabUi::new();
                phase_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("phase ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::PhaseTabUiCommand {
                                phase: phase.clone(),
                                command,
                            })
                        }
                    });

                phase_ui
            });

        created
    }

    fn ensure_process(&self, key: ProjectKey, process: &Reference) -> bool {
        let process = process.clone();
        let mut state = self.project_ui_state.lock().unwrap();

        self.ensure_process_inner(key, process.clone(), &mut state)
    }

    fn ensure_process_inner(
        &self,
        key: ProjectKey,
        process: Reference,
        state: &mut ValueGuard<ProjectUiState>,
    ) -> bool {
        let mut created = false;
        let _process_state = state
            .process_tab_uis
            .entry(process.clone())
            .or_insert_with(|| {
                created = true;
                debug!("ensuring process ui. process: {:?}", process);
                let mut process_ui = ProcessTabUi::new();
                process_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("process ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::ProcessTabUiCommand {
                                process: process.clone(),
                                command,
                            })
                        }
                    });

                process_ui
            });

        created
    }

    fn ensure_load_out(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) -> bool {
        let load_out_source = load_out_source.clone();
        let mut state = self.project_ui_state.lock().unwrap();
        let mut created = false;

        let _load_out_ui = state
            .load_out_tab_uis
            .entry(load_out_source.clone())
            .or_insert_with(|| {
                created = true;
                debug!("ensuring load out ui. source: {:?}", load_out_source);
                let mut load_out_ui = LoadOutTabUi::new(phase);
                load_out_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("load out ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::LoadOutTabUiCommand {
                                load_out_source: load_out_source.clone(),
                                command,
                            })
                        }
                    });

                load_out_ui
            });

        created
    }

    fn ensure_pcb(&self, key: ProjectKey, pcb_index: u16) -> bool {
        let mut state = self.project_ui_state.lock().unwrap();
        let mut created = false;

        let _pcb_ui = state
            .pcb_tab_uis
            .entry(pcb_index as usize)
            .or_insert_with(|| {
                created = true;
                debug!("ensuring pcb ui. pcb_index: {:?}", pcb_index);
                let mut pcb_ui = PcbTabUi::new(self.path.clone());
                pcb_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("pcb ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::PcbTabUiCommand {
                                pcb_index,
                                command,
                            })
                        }
                    });

                pcb_ui
            });

        created
    }

    fn ensure_unit_assignments(&self, key: ProjectKey, pcb_index: u16) -> bool {
        let mut state = self.project_ui_state.lock().unwrap();

        let mut created = false;

        let _unit_assignments_ui = state
            .unit_assignment_tab_uis
            .entry(pcb_index as usize)
            .or_insert_with(|| {
                created = true;
                debug!("ensuring unit assignments ui. pcb_index: {:?}", pcb_index);
                let mut unit_assignments_ui = UnitAssignmentsTabUi::new(self.path.clone(), pcb_index as u16);
                unit_assignments_ui
                    .component
                    .configure_mapper(self.component.sender.clone(), {
                        move |command| {
                            trace!("pcb ui mapper. command: {:?}", command);
                            (key, ProjectUiCommand::UnitAssignmentsTabUiCommand {
                                pcb_index,
                                command,
                            })
                        }
                    });

                unit_assignments_ui
            });

        created
    }

    fn navigate(&mut self, key: ProjectKey, path: NavigationPath) -> Option<ProjectAction> {
        // if the path starts with `/project/` then show/hide UI elements based on the path,
        info!("project::navigate. path: {}", path);

        #[must_use]
        fn handle_root(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if path.eq(&"/project/".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowOverview));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_issues(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if path.eq(&"/project/issues".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowIssues));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_placements(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if path.eq(&"/project/placements".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPlacements));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_parts(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if path.eq(&"/project/parts".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowParts));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phases(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if path.eq(&"/project/phases".into()) {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RefreshPhases));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phase(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if let Some(captures) = tree_item::REGULAR_EXPRESSIONS
                .phase
                .captures(&path)
            {
                let phase_reference: String = captures
                    .name("phase")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("phase_reference: {}", phase_reference);

                let reference = Reference::from_raw(phase_reference);
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPhase(reference)));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_process(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if let Some(captures) = tree_item::REGULAR_EXPRESSIONS
                .process
                .captures(&path)
            {
                let process_reference: String = captures
                    .name("process")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("process_reference: {}", process_reference);

                let reference = Reference::from_raw(process_reference);
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowProcess(reference)));
                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_phase_loadout(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if let Some(captures) = tree_item::REGULAR_EXPRESSIONS
                .phase_loadout
                .captures(&path)
            {
                let phase_reference: String = captures
                    .name("phase")
                    .unwrap()
                    .as_str()
                    .to_string();
                debug!("phase_reference: {}", phase_reference);

                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPhaseLoadout {
                    phase: Reference(phase_reference),
                }));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_pcb(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if let Some(captures) = tree_item::REGULAR_EXPRESSIONS
                .pcb
                .captures(&path)
            {
                let pcb_index = captures
                    .name("pcb")
                    .unwrap()
                    .as_str()
                    .parse::<u16>()
                    .unwrap();
                debug!("pcb: {}", pcb_index);

                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcb(pcb_index)));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        #[must_use]
        fn handle_unit_assignments(key: &ProjectKey, path: &NavigationPath) -> Option<ProjectAction> {
            if let Some(captures) = tree_item::REGULAR_EXPRESSIONS
                .pcb_unit
                .captures(&path)
            {
                let pcb_index = captures
                    .name("pcb")
                    .unwrap()
                    .as_str()
                    .parse::<u16>()
                    .unwrap();
                debug!("pcb: {}", pcb_index);

                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcbUnitAssignments(
                    pcb_index,
                )));

                Some(ProjectAction::Task(*key, task))
            } else {
                None
            }
        }

        let handlers = [
            handle_root,
            handle_issues,
            handle_parts,
            handle_pcb,
            handle_phase,
            handle_phase_loadout,
            handle_phases,
            handle_placements,
            handle_process,
            handle_unit_assignments,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(&key, &path))
    }

    pub fn update_processes(&mut self, project_overview: &ProjectOverview) {
        self.processes = project_overview.processes.clone();
    }

    pub fn update_pcbs(&mut self, project_overview: &ProjectOverview) {
        self.pcbs = project_overview
            .pcbs
            .iter()
            .map(|pcb| pcb.pcb_file.build_path(&self.path))
            .collect();
    }

    pub fn update_library_config(&mut self, library_config: &LibraryConfig) {
        self.library_config = Some(library_config.clone())
    }

    #[must_use]
    fn update_placement(
        planner_core_service: &mut PlannerCoreService,
        key: ProjectKey,
        object_path: ObjectPath,
        new_placement: PlacementState,
        old_placement: PlacementState,
    ) -> (Vec<Task<ProjectAction>>, Vec<UpdatePlacementAction>) {
        let mut tasks = vec![];

        fn handle_phase(
            planner_core_service: &mut PlannerCoreService,
            _key: &ProjectKey,
            object_path: &ObjectPath,
            new_placement: &PlacementState,
            old_placement: &PlacementState,
        ) -> Option<(Vec<UpdatePlacementAction>, Result<Vec<ProjectAction>, ProjectAction>)> {
            if !new_placement
                .phase
                .eq(&old_placement.phase)
            {
                // it's possible that assigning/clearing a placement could make the phase complete

                let (phase, operation, mut update_placement_actions) =
                    match (&new_placement.phase, &old_placement.phase) {
                        (Some(new_phase), None) => (new_phase, SetOrClearAction::Set, vec![
                            UpdatePlacementAction::RefreshPhasePlacements {
                                phase: new_phase.clone(),
                            },
                            UpdatePlacementAction::RefreshPhaseOverview {
                                phase: new_phase.clone(),
                            },
                        ]),
                        (Some(new_phase), Some(old_phase)) => (new_phase, SetOrClearAction::Set, vec![
                            UpdatePlacementAction::RefreshPhasePlacements {
                                phase: new_phase.clone(),
                            },
                            UpdatePlacementAction::RefreshPhasePlacements {
                                phase: old_phase.clone(),
                            },
                            UpdatePlacementAction::RefreshPhaseOverview {
                                phase: new_phase.clone(),
                            },
                            UpdatePlacementAction::RefreshPhaseOverview {
                                phase: old_phase.clone(),
                            },
                        ]),
                        (None, Some(old_phase)) => (old_phase, SetOrClearAction::Clear, vec![
                            UpdatePlacementAction::RefreshPhasePlacements {
                                phase: old_phase.clone(),
                            },
                            UpdatePlacementAction::RefreshPhaseOverview {
                                phase: old_phase.clone(),
                            },
                        ]),
                        _ => unreachable!(),
                    };

                update_placement_actions.push(UpdatePlacementAction::RefreshPhases);

                Some((
                    update_placement_actions,
                    planner_core_service
                        .update(Event::AssignPlacementsToPhase {
                            phase: phase.clone(),
                            operation,
                            placements: exact_match(&object_path.to_string()),
                        })
                        .into_actions(),
                ))
            } else {
                None
            }
        }

        fn handle_placed(
            planner_core_service: &mut PlannerCoreService,
            _key: &ProjectKey,
            object_path: &ObjectPath,
            new_placement: &PlacementState,
            old_placement: &PlacementState,
        ) -> Option<(Vec<UpdatePlacementAction>, Result<Vec<ProjectAction>, ProjectAction>)> {
            if new_placement.phase.is_none() {
                error!(
                    "Attempt to place a placement that has not been assigned to a phase. placement: {:?}",
                    new_placement
                );
                return None;
            }

            if new_placement.operation_status != old_placement.operation_status {
                let operation = match new_placement.operation_status {
                    PlacementStatus::Placed => PlacementOperation::Place,
                    PlacementStatus::Skipped => PlacementOperation::Skip,
                    PlacementStatus::Pending => PlacementOperation::Reset,
                };

                let phase = new_placement.phase.as_ref().unwrap();

                Some((
                    vec![
                        UpdatePlacementAction::RefreshPhaseOverview {
                            phase: phase.clone(),
                        },
                        UpdatePlacementAction::RefreshPhasePlacements {
                            phase: phase.clone(),
                        },
                        UpdatePlacementAction::RefreshPhases,
                    ],
                    planner_core_service
                        .update(Event::RecordPlacementsOperation {
                            object_path_patterns: vec![exact_match(&object_path.to_string())],
                            operation,
                        })
                        .into_actions(),
                ))
            } else {
                None
            }
        }

        #[derive(Debug)]
        enum Operation {
            AddOrRemovePhase,
            SetOrResetPlaced,
        }

        // FUTURE find a solution to keep the operation with the handler, instead of two separate arrays.
        //        a tuple was tried, but results in a compile error: "expected fn item, found a different fn item"
        let operations = [Operation::AddOrRemovePhase, Operation::SetOrResetPlaced];

        let action_handlers = [handle_phase, handle_placed];

        let mut update_placement_actions = vec![];

        for (operation, handler) in operations
            .into_iter()
            .zip(action_handlers.into_iter())
        {
            trace!("update placement, trying handler for operation: {:?}", operation);
            if let Some((additional_update_placement_actions, core_result)) =
                handler(planner_core_service, &key, &object_path, &new_placement, &old_placement)
            {
                debug!("update placement, applicable handler found. operation: {:?}", operation);
                match core_result {
                    Ok(actions) => {
                        debug!("actions: {:?}", actions);
                        let effect_tasks: Vec<Task<ProjectAction>> = actions
                            .into_iter()
                            .map(Task::done)
                            .collect();
                        tasks.extend(effect_tasks);
                        update_placement_actions.extend(additional_update_placement_actions);
                    }
                    Err(service_error) => {
                        tasks.push(Task::done(service_error));
                        break;
                    }
                }
            }
        }

        // We need to refresh the placements, it's possible some of the phases applied to placements may not have
        // been accepted, but the UI will show they were accepted, e.g. when pasting a 'Top' phase onto a
        // placement with a side of 'Bottom'.
        //
        // FUTURE  Ideally, prevent pasting invalid values into the cells in the first place.
        //         This is a limitation of the design of egui_data_tables and the architecture of this app and
        //         would require a large refactoring.
        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
            ProjectViewRequest::Placements,
        )));
        tasks.push(final_task);

        update_placement_actions.dedup();

        (tasks, update_placement_actions)
    }

    fn handle_update_placement_actions(
        tasks: &mut Vec<Task<ProjectAction>>,
        actions: Vec<UpdatePlacementAction>,
        request_issues_refresh: &mut bool,
    ) {
        // Updating placements can create/clear issues.
        *request_issues_refresh = true;

        for action in actions {
            if let Some(task) = match action {
                UpdatePlacementAction::RefreshPhases => Some(Task::done(ProjectAction::UiCommand(
                    ProjectUiCommand::RequestProjectView(ProjectViewRequest::Phases),
                ))),
                UpdatePlacementAction::RefreshPhaseOverview {
                    phase,
                } => Some(Task::done(ProjectAction::UiCommand(
                    ProjectUiCommand::RequestProjectView(ProjectViewRequest::PhaseOverview {
                        phase,
                    }),
                ))),
                UpdatePlacementAction::RefreshPhasePlacements {
                    phase,
                } => Some(Task::done(ProjectAction::UiCommand(
                    ProjectUiCommand::RequestProjectView(ProjectViewRequest::PhasePlacements {
                        phase,
                    }),
                ))),
            } {
                tasks.push(task);
            }
        }
    }

    fn locate_component(
        &self,
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    ) -> Option<ProjectAction> {
        let (pcb_number, unit_number) = object_path
            .pcb_instance_and_unit()
            .unwrap();

        let (pcb_index, _unit_index) = (pcb_number - 1, unit_number - 1);

        self.pcbs
            .get(pcb_index as usize)
            .map(|pcb_file| ProjectAction::LocateComponent {
                pcb_file: pcb_file.clone(),
                object_path,
                pcb_side,
                design_position,
                unit_position,
            })
    }

    fn show_add_phase_modal(&mut self, key: ProjectKey) {
        let mut modal = AddPhaseModal::new(self.path.clone(), self.processes.clone());
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("add phase modal mapper. command: {:?}", command);
                (key, ProjectUiCommand::AddPhaseModalCommand(command))
            });

        self.add_phase_modal = Some(modal);
    }

    fn show_package_sources_modal(&mut self, key: ProjectKey) {
        if let Some(library_config) = &self.library_config {
            let mut modal = PackageSourcesModal::new(
                self.path.clone(),
                library_config.package_source.clone(),
                library_config
                    .package_mappings_source
                    .clone(),
            );
            modal
                .component
                .configure_mapper(self.component.sender.clone(), move |command| {
                    trace!("package sources modal mapper. command: {:?}", command);
                    (key, ProjectUiCommand::PackageSourcesModalCommand(command))
                });

            self.package_sources_modal = Some(modal);
        }
    }

    fn pick_pcb_file(&mut self) {
        let mut picker = self.file_picker.lock().unwrap();
        picker.pick_file();
    }
}

#[derive(Debug, PartialEq)]
enum UpdatePlacementAction {
    RefreshPhases,
    RefreshPhaseOverview { phase: PhaseReference },
    RefreshPhasePlacements { phase: PhaseReference },
}

pub struct ProjectContext {
    pub key: ProjectKey,
}

impl UiComponent for Project {
    type UiContext<'context> = ProjectContext;
    type UiCommand = (ProjectKey, ProjectUiCommand);
    type UiAction = ProjectAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let ProjectContext {
            key,
        } = context;

        egui::Panel::top(ui.id().with("top_panel")).show(ui, |ui| {
            ui.label(tr!("project-detail-path", { path: self.path.display().to_string() }));

            let state = self.project_ui_state.lock().unwrap();
            if let Some(name) = &state.name {
                ui.label(tr!("project-detail-name", { name: name }));
            } else {
                ui.spinner();
            }

            self.toolbar.ui(ui, &mut ());
        });

        //
        // Tabs
        //

        let mut tab_context = ProjectTabContext {
            state: self.project_ui_state.clone(),
        };

        let mut project_tabs = self.project_tabs.lock().unwrap();
        project_tabs.cleanup_tabs(&mut tab_context);
        project_tabs.ui(ui, &mut tab_context);

        if !self.errors.is_empty() {
            dialogs::errors::show_errors_modal(ui, *key, &self.path, &self.errors, &self.component);
        }

        //
        // Modals
        //
        if let Some(dialog) = &self.add_phase_modal {
            dialog.ui(ui, &mut ());
        }
        if let Some(dialog) = &self.package_sources_modal {
            dialog.ui(ui, &mut ());
        }

        //
        // File Picker
        //
        {
            // FUTURE consider having the caller of the picker specify a function to call to build the ProjectUiCommand
            //        similar to how it's done in `ui_app.rs`, this keeps the command creation code close to the code
            //        that needs to pick a file
            let mut picker = self.file_picker.lock().unwrap();
            match picker.picked() {
                Ok(picked_file) => {
                    self.component
                        .send((*key, ProjectUiCommand::PcbFilePicked(picked_file)));
                }
                Err(_) => {}
            }
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        let (key, command) = command;

        let mut request_issues_refresh = false;

        let action = match command {
            ProjectUiCommand::None => None,
            ProjectUiCommand::Create => {
                let state = self.project_ui_state.lock().unwrap();
                self.planner_core_service
                    .update(Event::CreateProject {
                        name: state.name.clone().unwrap(),
                        path: self.path.clone(),
                        packages: None,
                        package_mappings: None,
                    })
                    .when_ok(key, |_| Some(ProjectUiCommand::Created))
            }
            ProjectUiCommand::Created => {
                let show_explorer_task = self.show_explorer();
                let show_overview_tasks = self.show_overview();
                let show_issues_tasks = self.show_issues();
                let mut tasks = vec![show_explorer_task];
                tasks.extend(show_overview_tasks);
                tasks.extend(show_issues_tasks);

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::Load => {
                debug!("Loading project from path. path: {}", self.path.display());

                self.planner_core_service
                    .update(Event::Load {
                        path: self.path.clone(),
                    })
                    .when_ok(key, |_tasks| Some(ProjectUiCommand::Loaded))
            }
            ProjectUiCommand::Loaded => {
                let show_explorer_task = self.show_explorer();
                let show_overview_tasks = self.show_overview();
                let show_issues_tasks = self.show_issues();
                let mut tasks = vec![show_explorer_task];
                tasks.extend(show_overview_tasks);
                tasks.extend(show_issues_tasks);

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::Save => {
                debug!("Saving project. path: {}", self.path.display());
                self.planner_core_service
                    .update(Event::Save)
                    .when_ok(key, |_| Some(ProjectUiCommand::Saved))
            }
            ProjectUiCommand::Saved => {
                debug!("Saved project.");
                None
            }
            ProjectUiCommand::ProjectRefreshed => {
                debug!("Project refreshed.");

                // FUTURE The current approach is to know exactly what needs to happen, however, the child elements/tabs
                //        themselves should be responsible for taking appropriate actions, this requires ui components
                //        to subscribe to refresh events or something and there is no mechanism for that yet.

                let mut requests = vec![
                    // Update anything that uses data from views
                    ProjectViewRequest::Overview,
                    // Update the tree, since phases may have been deleted, etc.
                    ProjectViewRequest::ProjectTree,
                    // refresh phases
                    ProjectViewRequest::Phases,
                    // refresh placements
                    ProjectViewRequest::Placements,
                ];

                let phase_requests = self
                    .phases
                    .iter()
                    .flat_map(|phase| {
                        vec![
                            ProjectViewRequest::PhaseOverview {
                                phase: phase.phase_reference.clone(),
                            },
                            ProjectViewRequest::PhasePlacements {
                                phase: phase.phase_reference.clone(),
                            },
                        ]
                    })
                    .collect::<Vec<_>>();
                requests.extend(phase_requests);

                let tasks = requests
                    .into_iter()
                    .map(|request| Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(request))))
                    .collect::<Vec<_>>();

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => {
                self.modified = project_modified;
                self.pcbs_modified = pcbs_modified;
                // TODO remove the logical or here when AddPcbs has been reworked.
                Some(ProjectAction::SetModifiedState(project_modified || pcbs_modified))
            }

            //
            // errors
            //
            ProjectUiCommand::Error(error) => {
                match error {
                    PlannerError::CoreError(message) => {
                        self.errors.push(message);
                    }
                    PlannerError::Other(message) => {
                        self.errors.push(message);
                    }
                }
                None
            }
            ProjectUiCommand::ClearErrors => {
                self.errors.clear();
                None
            }

            //
            // project views
            //
            ProjectUiCommand::RequestProjectView(view_request) => {
                debug!("request project view: {:?}", view_request);
                let event = match view_request {
                    ProjectViewRequest::Overview => Event::RequestOverviewView {},
                    ProjectViewRequest::Parts => Event::RequestPartStatesView {},
                    ProjectViewRequest::Placements => Event::RequestPlacementsView {},
                    ProjectViewRequest::Phases => Event::RequestPhasesView {},
                    ProjectViewRequest::ProjectTree => Event::RequestProjectTreeView {},
                    ProjectViewRequest::PhaseOverview {
                        phase,
                    } => Event::RequestPhaseOverviewView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PhaseLoadOut {
                        phase,
                    } => Event::RequestPhaseLoadOutView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PhasePlacements {
                        phase,
                    } => Event::RequestPhasePlacementsView {
                        phase_reference: phase,
                    },
                    ProjectViewRequest::PcbOverview {
                        pcb,
                    } => Event::RequestProjectPcbOverviewView {
                        pcb,
                    },
                    ProjectViewRequest::PcbUnitAssignments {
                        pcb,
                    } => Event::RequestPcbUnitAssignmentsView {
                        pcb,
                    },
                    ProjectViewRequest::ProcessDefinition {
                        process,
                    } => Event::RequestProcessDefinitionView {
                        process_reference: process,
                    },
                    ProjectViewRequest::ProjectReport => Event::RequestProjectReportView {},
                };

                self.planner_core_service
                    .update(event)
                    .when_ok(key, |_| None)
            }
            ProjectUiCommand::ProjectView(view) => {
                match view {
                    ProjectView::Overview(project_overview) => {
                        trace!("project overview: {:?}", project_overview);
                        self.update_processes(&project_overview);
                        self.update_pcbs(&project_overview);
                        self.update_library_config(&project_overview.library_config);

                        let mut state = self.project_ui_state.lock().unwrap();
                        state.name = Some(project_overview.name.clone());
                        state
                            .overview_ui
                            .update_overview(project_overview);
                    }
                    ProjectView::ProjectTree(project_tree) => {
                        trace!("project tree: {:?}", project_tree);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .explorer_tab_ui
                            .update_tree(project_tree);
                    }
                    ProjectView::PcbOverview(project_pcb_overview) => {
                        trace!("project_pcb_overview: {:?}", project_pcb_overview);

                        // FUTURE use the name to update the tab label of the pcb overview and unit_assignments tabs?
                        //        would need multiple actions...
                        //        A reactive 'Reactive<Option<PcbOverview>>' that's given to both tabs
                        //        would be perfect here to avoid needing any actions.
                        let _pcb_file = project_pcb_overview.pcb_file.clone();

                        let mut state = self.project_ui_state.lock().unwrap();

                        if let Some(pcb_ui) = state
                            .pcb_tab_uis
                            .get_mut(&(project_pcb_overview.index as usize))
                        {
                            pcb_ui.update_project_pcb_overview(project_pcb_overview.clone());
                        }

                        if let Some(unit_assignments_ui) = state
                            .unit_assignment_tab_uis
                            .get_mut(&(project_pcb_overview.index as usize))
                        {
                            unit_assignments_ui.update_project_pcb_overview(project_pcb_overview.clone());
                        }
                    }
                    ProjectView::PcbUnitAssignments(pcb_unit_assignments) => {
                        trace!("pcb_unit_assignments: {:?}", pcb_unit_assignments);

                        let mut state = self.project_ui_state.lock().unwrap();

                        if let Some(unit_assignments_ui) = state
                            .unit_assignment_tab_uis
                            .get_mut(&(pcb_unit_assignments.index as usize))
                        {
                            unit_assignments_ui.update_unit_assignments(pcb_unit_assignments);
                        }
                    }
                    ProjectView::Placements(placements) => {
                        trace!("placements: {:?}", placements);
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .placements_ui
                            .update_placements(placements, self.phases.clone())
                    }
                    ProjectView::Phases(phases) => {
                        trace!("phases: {:?}", phases);
                        self.phases = phases.phases;
                        let mut state = self.project_ui_state.lock().unwrap();
                        state
                            .placements_ui
                            .update_phases(self.phases.clone());

                        state
                            .overview_ui
                            .update_phases(self.phases.clone());
                    }
                    ProjectView::PhaseOverview(phase_overview) => {
                        trace!("phase overview: {:?}", phase_overview);
                        let phase = phase_overview.phase_reference.clone();

                        self.ensure_phase(key, &phase);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_ui = state
                            .phases_tab_uis
                            .get_mut(&phase)
                            .unwrap();

                        phase_ui.update_overview(phase_overview);
                    }
                    ProjectView::PhasePlacements(phase_placements) => {
                        trace!("phase placements: {:?}", phase_placements);
                        let phase = phase_placements.phase_reference.clone();

                        self.ensure_phase(key, &phase);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let phase_ui = state
                            .phases_tab_uis
                            .get_mut(&phase)
                            .unwrap();

                        phase_ui.update_placements(phase_placements, self.phases.clone());
                    }
                    ProjectView::ProcessDefinition(process_definition) => {
                        trace!("process definition: {:?}", process_definition);
                        let process = process_definition.reference.clone();

                        self.ensure_process(key, &process);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let process_ui = state
                            .process_tab_uis
                            .get_mut(&process)
                            .unwrap();

                        process_ui.update_definition(process_definition);
                    }
                    ProjectView::Parts(part_states) => {
                        trace!("parts: {:?}", part_states);
                        let mut state = self.project_ui_state.lock().unwrap();

                        state
                            .parts_tab_ui
                            .update_part_states(part_states, self.processes.clone())
                    }
                    ProjectView::PhaseLoadOut(load_out) => {
                        trace!("load_out: {:?}", load_out);
                        let load_out_source = load_out.source.clone();

                        self.ensure_load_out(key, load_out.phase_reference.clone(), &load_out_source);

                        let mut state = self.project_ui_state.lock().unwrap();
                        let load_out_ui = state
                            .load_out_tab_uis
                            .get_mut(&load_out_source)
                            .unwrap();

                        load_out_ui.update_load_out(load_out);
                    }
                    ProjectView::ProjectReport(report) => {
                        info!("report:\n{:?}", report);

                        request_issues_refresh = false;

                        let mut state = self.project_ui_state.lock().unwrap();

                        state.issues_ui.update_report(report)
                    }
                }
                None
            }

            //
            // pcb views
            //
            ProjectUiCommand::RequestPcbView(view_request) => {
                let event = match view_request {
                    PcbViewRequest::Overview {
                        path,
                    } => Some(Event::RequestPcbOverviewView {
                        path,
                    }),
                    PcbViewRequest::Panel {
                        ..
                    } => {
                        // TODO add/use a suitable core event
                        None
                    }
                };
                // TODO remove the `if let`
                if let Some(event) = event {
                    self.planner_core_service
                        .update(event)
                        .when_ok(key, |_| None)
                } else {
                    None
                }
            }
            ProjectUiCommand::PcbView(view) => match view {
                PcbView::PcbOverview(pcb_overview) => {
                    let mut state = self.project_ui_state.lock().unwrap();

                    for (_index, pcb_ui) in state.pcb_tab_uis.iter_mut() {
                        pcb_ui.update_pcb_overview(&pcb_overview);
                    }

                    for (_index, unit_assignments_ui) in state.unit_assignment_tab_uis.iter_mut() {
                        unit_assignments_ui.update_pcb_overview(&pcb_overview);
                    }

                    None
                }
                PcbView::PanelSizing(_panel_sizing) => {
                    // nothing requests this view
                    None
                }
            },

            //
            // toolbar
            //
            ProjectUiCommand::ToolbarCommand(toolbar_command) => {
                let action = self
                    .toolbar
                    .update(toolbar_command, &mut ());
                match action {
                    Some(ProjectToolbarAction::ShowProjectExplorer) => {
                        let task = self.show_explorer();
                        Some(ProjectAction::Task(key, task))
                    }
                    Some(ProjectToolbarAction::GenerateArtifacts) => self
                        .planner_core_service
                        .update(Event::GenerateArtifacts)
                        .when_ok(key, |_| None),
                    Some(ProjectToolbarAction::Refresh) => {
                        let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RefreshPcbs)).chain(
                            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RefreshFromDesignVariants)),
                        );
                        Some(ProjectAction::Task(key, task))
                    }
                    Some(ProjectToolbarAction::RemoveUnusedPlacements) => self
                        .planner_core_service
                        .update(Event::RemoveUsedPlacements {
                            phase: None,
                        })
                        .when_ok(key, |_| Some(ProjectUiCommand::ProjectRefreshed)),
                    Some(ProjectToolbarAction::PickPcbFile) => {
                        self.pick_pcb_file();
                        None
                    }
                    Some(ProjectToolbarAction::ShowAddPhaseDialog) => {
                        self.show_add_phase_modal(key);
                        None
                    }
                    Some(ProjectToolbarAction::ShowPackageSourcesDialog) => {
                        self.show_package_sources_modal(key);
                        None
                    }
                    Some(ProjectToolbarAction::ResetOperations) => self
                        .planner_core_service
                        .update(Event::ResetOperations {})
                        .when_ok(key, |_| Some(ProjectUiCommand::ProjectRefreshed)),
                    None => None,
                }
            }

            //
            // pcbs
            //
            ProjectUiCommand::PcbRemoved => Some(ProjectAction::Task(
                key,
                Task::done(ProjectAction::UiCommand(ProjectUiCommand::RefreshFromDesignVariants)),
            )),
            ProjectUiCommand::PcbFilePicked(pcb_path) => {
                // FUTURE consider storing a relative file if the pcb_path is in a subdirectory of the project path.
                let pcb_file = FileReference::Absolute(pcb_path);

                let mut tasks = vec![];

                // Adding PCBs can clear issues.
                request_issues_refresh = true;

                match self
                    .planner_core_service
                    .update(Event::AddPcb {
                        pcb_file,
                    })
                    .into_actions()
                {
                    Ok(actions) => {
                        let event_tasks = actions
                            .into_iter()
                            .map(Task::done)
                            .collect::<Vec<Task<ProjectAction>>>();

                        tasks.extend(event_tasks);

                        tasks.push(Task::done(ProjectAction::UiCommand(
                            ProjectUiCommand::RequestProjectView(ProjectViewRequest::ProjectTree),
                        )));
                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Err(error_action) => Some(error_action),
                }
            }
            ProjectUiCommand::AddPhaseModalCommand(command) => {
                if let Some(modal) = &mut self.add_phase_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(AddPhaseModalAction::Submit(args)) => {
                            self.add_phase_modal.take();

                            match self
                                .planner_core_service
                                .update(Event::CreatePhase {
                                    process: args.process,
                                    reference: args.reference,
                                    load_out: args.load_out,
                                    pcb_side: args.pcb_side,
                                })
                                .into_actions()
                            {
                                Ok(actions) => {
                                    // Adding phases can clear issues.
                                    request_issues_refresh = true;

                                    let mut tasks = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect::<Vec<Task<ProjectAction>>>();

                                    let additional_tasks = vec![
                                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                            ProjectViewRequest::ProjectTree,
                                        ))),
                                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                            ProjectViewRequest::Phases,
                                        ))),
                                    ];
                                    tasks.extend(additional_tasks);

                                    Some(ProjectAction::Task(key, Task::batch(tasks)))
                                }
                                Err(error_action) => Some(error_action),
                            }
                        }
                        Some(AddPhaseModalAction::CloseDialog) => {
                            self.add_phase_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }
            ProjectUiCommand::PackageSourcesModalCommand(command) => {
                if let Some(modal) = &mut self.package_sources_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(PackageSourcesModalAction::Submit(args)) => {
                            self.package_sources_modal.take();

                            debug!(
                                "packages: {:?}, package_mappings: {:?}",
                                args.packages_source, args.package_mappings_source
                            );

                            self.planner_core_service
                                .update(Event::ApplyPackageSources {
                                    packages_source: args.packages_source,
                                    package_mappings_source: args.package_mappings_source,
                                })
                                .when_ok(key, |_| {
                                    // we raise this event, since changing package souces
                                    // can have an effect of the sort order of placements if they are sorted by
                                    // something that uses package mappings and packages.
                                    Some(ProjectUiCommand::ProjectRefreshed)
                                })
                        }
                        Some(PackageSourcesModalAction::CloseDialog) => {
                            self.package_sources_modal.take();
                            None
                        }
                    }
                } else {
                    None
                }
            }

            //
            // tabs
            //
            ProjectUiCommand::TabCommand(tab_command) => {
                let mut project_tabs = self.project_tabs.lock().unwrap();

                let mut tab_context = ProjectTabContext {
                    state: self.project_ui_state.clone(),
                };

                let action = project_tabs.update(tab_command, &mut tab_context);
                match action {
                    None => {}
                    Some(ProjectTabAction::None) => {
                        debug!("ProjectTabAction::None");
                    }
                }
                None
            }
            ProjectUiCommand::ShowPhaseLoadout {
                phase,
            } => {
                let tasks = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::Phases,
                    ))),
                    // FIXME there is no current way to know whether to continue,  or not, as there is no mechanism
                    //       to know if `ProjectViewRequest::Phases` completed and all the effects have ALSO completed.
                    //       currently the ProjectUiCommand::ContinueShowPhaseLoadout is called before the handler for
                    //       ProjectUiCommand::ProjectView is called.
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::ContinueShowPhaseLoadout {
                        phase,
                    })),
                ];

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ContinueShowPhaseLoadout {
                phase,
            } => {
                let phase_overview = self
                    .phases
                    .iter()
                    .find(|phase_overview| {
                        phase_overview
                            .phase_reference
                            .eq(&phase)
                    });

                if let Some(phase_overview) = phase_overview {
                    let task = self.show_loadout(
                        key,
                        phase_overview.phase_reference.clone(),
                        &phase_overview.load_out_source,
                    );

                    task.map(|task| ProjectAction::Task(key, task))
                } else {
                    // FIXME see ProjectUiCommand::ShowPhaseLoadout handler

                    debug!(
                        "FIXME - there continuing to show a phase when the phase overview has not available. phase: {:?}",
                        phase
                    );

                    // FIXME retrying indefinitely by sending the command again, in the hopes it has worked by the time this
                    //       command handler is executed again.
                    Some(ProjectAction::Task(
                        key,
                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::ContinueShowPhaseLoadout {
                            phase,
                        })),
                    ))
                }
            }
            ProjectUiCommand::ShowPcbUnitAssignments(pcb_index) => {
                let tasks = self.show_unit_assignments(key, pcb_index);
                tasks.map(|tasks| ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ShowExplorer => {
                let task = self.show_explorer();
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::ShowIssues => {
                let tasks = self.show_issues();
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ShowOverview => {
                let tasks = self.show_overview();
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ShowParts => {
                let task = self.show_parts();
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::ShowPlacements => {
                let tasks = self.show_placements();
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::RefreshPhases => {
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::Phases,
                )));
                Some(ProjectAction::Task(key, task))
            }
            ProjectUiCommand::ShowPhase(phase) => {
                let tasks = self.show_phase(key.clone(), phase.clone());

                tasks.map(|tasks| ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ShowProcess(process) => {
                let tasks = self.show_process(key.clone(), process.clone());

                tasks.map(|tasks| ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ShowPcb(pcb_index) => {
                let tasks = self.show_pcb(key.clone(), pcb_index);

                tasks.map(|tasks| ProjectAction::Task(key, Task::batch(tasks)))
            }

            ProjectUiCommand::ExplorerTabUiCommand(command) => {
                let context = &mut ExplorerTabUiContext::default();
                let explorer_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .explorer_tab_ui
                    .update(command, context);
                match explorer_ui_action {
                    None => None,
                    Some(ExplorerTabUiAction::Navigate(path)) => self.navigate(key, path),
                    Some(ExplorerTabUiAction::SetPhaseOrdering(phases)) => {
                        match self
                            .planner_core_service
                            .update(Event::SetPhaseOrdering {
                                phases,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let mut tasks = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect::<Vec<Task<ProjectAction>>>();

                                let additional_tasks = vec![
                                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                        ProjectViewRequest::ProjectTree,
                                    ))),
                                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                        ProjectViewRequest::Phases,
                                    ))),
                                ];
                                tasks.extend(additional_tasks);

                                Some(ProjectAction::Task(key, Task::batch(tasks)))
                            }
                            Err(error_action) => Some(error_action),
                        }
                    }
                }
            }
            ProjectUiCommand::IssuesTabUiCommand(command) => {
                let context = &mut IssuesTabUiContext::default();
                let issues_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .issues_ui
                    .update(command, context);
                match issues_ui_action {
                    None => None,
                    Some(IssuesTabUiAction::None) => None,
                    Some(IssuesTabUiAction::RefreshIssues) => Some(ProjectAction::Task(
                        key,
                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                            ProjectViewRequest::ProjectReport,
                        ))),
                    )),
                }
            }
            ProjectUiCommand::OverviewTabUiCommand(command) => {
                let context = &mut OverviewTabUiContext::default();
                let overview_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .overview_ui
                    .update(command, context);
                match overview_ui_action {
                    None => None,
                    Some(OverviewTabUiAction::None) => None,
                    Some(OverviewTabUiAction::AddPhase) => {
                        self.show_add_phase_modal(key);
                        None
                    }
                    Some(OverviewTabUiAction::AddPcb) => {
                        self.pick_pcb_file();
                        None
                    }
                    Some(OverviewTabUiAction::DeletePhase(reference)) => {
                        // Deleting phases can create issues (no phases, unassigned placements)
                        request_issues_refresh = true;

                        self.planner_core_service
                            .update(Event::DeletePhase {
                                reference: reference.clone(),
                            })
                            .when_ok(key, |_| Some(ProjectUiCommand::PhaseDeleted(reference)))
                    }
                    Some(OverviewTabUiAction::RemovePcb(pcb_unit_index)) => {
                        // Deleting pcbs can create issues (no pcbs, etc)
                        request_issues_refresh = true;
                        self.planner_core_service
                            .update(Event::RemovePcb {
                                index: pcb_unit_index,
                            })
                            .when_ok(key, |_| Some(ProjectUiCommand::PcbRemoved))
                    }
                }
            }
            ProjectUiCommand::PartsTabUiCommand(command) => {
                let context = &mut PartsTabUiContext::default();
                let parts_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .parts_tab_ui
                    .update(command, context);
                match parts_ui_action {
                    Some(PartsTabUiAction::None) => None,
                    None => None,
                    Some(PartsTabUiAction::UpdateProcessesForPart {
                        part,
                        processes,
                    }) => {
                        let mut tasks = vec![];
                        for (process, enabled) in processes {
                            let operation = match enabled {
                                true => AddOrRemoveAction::Add,
                                false => AddOrRemoveAction::Remove,
                            };

                            match self
                                .planner_core_service
                                .update(Event::AssignProcessToParts {
                                    process,
                                    operation,
                                    manufacturer: exact_match(part.manufacturer.as_str()),
                                    mpn: exact_match(part.mpn.as_str()),
                                })
                                .into_actions()
                            {
                                Ok(actions) => {
                                    debug!("actions: {:?}", actions);
                                    let effect_tasks: Vec<Task<ProjectAction>> = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect();
                                    tasks.extend(effect_tasks);
                                }
                                Err(service_error) => {
                                    tasks.push(Task::done(service_error));
                                    break;
                                }
                            }
                        }

                        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                            ProjectViewRequest::Parts,
                        )));
                        tasks.push(final_task);

                        let action = ProjectAction::Task(key, Task::batch(tasks));

                        Some(action)
                    }
                    Some(PartsTabUiAction::ApplyPartsAction(parts, apply_action)) => {
                        let mut tasks = vec![];

                        debug!("apply_action: {:?}", apply_action);

                        for part in parts {
                            debug!("part: {:?}", part);

                            match &apply_action {
                                PartsTabUiApplyAction::AddProcess(process)
                                | PartsTabUiApplyAction::RemoveProcess(process) => {
                                    let operation = match &apply_action {
                                        PartsTabUiApplyAction::AddProcess(_) => AddOrRemoveAction::Add,
                                        PartsTabUiApplyAction::RemoveProcess(_) => AddOrRemoveAction::Remove,
                                    };

                                    match self
                                        .planner_core_service
                                        .update(Event::AssignProcessToParts {
                                            process: process.clone(),
                                            operation,
                                            manufacturer: exact_match(part.manufacturer.as_str()),
                                            mpn: exact_match(part.mpn.as_str()),
                                        })
                                        .into_actions()
                                    {
                                        Ok(actions) => {
                                            debug!("actions: {:?}", actions);
                                            let effect_tasks: Vec<Task<ProjectAction>> = actions
                                                .into_iter()
                                                .map(Task::done)
                                                .collect();
                                            tasks.extend(effect_tasks);
                                        }
                                        Err(service_error) => {
                                            tasks.push(Task::done(service_error));
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let final_task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                            ProjectViewRequest::Parts,
                        )));
                        tasks.push(final_task);

                        let action = ProjectAction::Task(key, Task::batch(tasks));

                        Some(action)
                    }
                    Some(PartsTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                }
            }
            ProjectUiCommand::PhaseTabUiCommand {
                phase,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let phase_ui = state
                    .phases_tab_uis
                    .get_mut(&phase)
                    .unwrap();

                let context = &mut PhaseTabUiContext::default();
                let phase_ui_action = phase_ui.update(command, context);

                match phase_ui_action {
                    None => None,
                    Some(PhaseTabUiAction::None) => None,
                    Some(PhaseTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PhaseTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let (mut tasks, actions) = Self::update_placement(
                            &mut self.planner_core_service,
                            key,
                            object_path,
                            new_placement,
                            old_placement,
                        );
                        Self::handle_update_placement_actions(&mut tasks, actions, &mut request_issues_refresh);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PhaseTabUiAction::AddPartsToLoadout {
                        phase,
                        manufacturer_pattern,
                        mpn_pattern,
                    }) => {
                        // Adding parts to loadouts can clear issues
                        request_issues_refresh = true;

                        self.planner_core_service
                            .update(Event::AddPartsToLoadout {
                                phase,
                                manufacturer: manufacturer_pattern,
                                mpn: mpn_pattern,
                            })
                            .when_ok(key, |_| None)
                    }
                    Some(PhaseTabUiAction::SetPlacementOrderings(args)) => self
                        .planner_core_service
                        .update(Event::SetPlacementOrdering {
                            phase: phase.clone(),
                            placement_orderings: args.orderings,
                        })
                        .when_ok(key, |_| Some(ProjectUiCommand::RefreshPhase(phase))),
                    Some(PhaseTabUiAction::TaskAction {
                        phase,
                        operation,
                        task,
                        action,
                    }) => self
                        .planner_core_service
                        .update(Event::RecordPhaseOperation {
                            phase: phase.clone(),
                            operation,
                            task,
                            action,
                        })
                        .when_ok(key, |_| Some(ProjectUiCommand::RefreshPhase(phase))),
                    Some(PhaseTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => self.locate_component(object_path, pcb_side, design_position, unit_position),
                    Some(PhaseTabUiAction::Refresh {
                        phase,
                    }) => {
                        let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RefreshPhase(phase)));
                        Some(ProjectAction::Task(key, task))
                    }
                }
            }
            ProjectUiCommand::ProcessTabUiCommand {
                process,
                command,
            } => {
                trace!("process: {:?}, command: {:?}", process, command);

                let mut state = self.project_ui_state.lock().unwrap();
                let process_ui = state
                    .process_tab_uis
                    .get_mut(&process)
                    .unwrap();

                let context = &mut ProcessTabUiContext::default();
                let process_ui_action = process_ui.update(command, context);

                match process_ui_action {
                    None => None,
                    Some(ProcessTabUiAction::None) => None,

                    Some(ProcessTabUiAction::Delete(process_reference)) => {
                        let mut tasks = vec![];
                        match self
                            .planner_core_service
                            .update(Event::DeleteProcess {
                                process_reference: process_reference.clone(),
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let effect_tasks: Vec<Task<ProjectAction>> = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect();
                                tasks.extend(effect_tasks);

                                tasks.push(Task::done(ProjectAction::UiCommand(
                                    ProjectUiCommand::RequestProjectView(ProjectViewRequest::ProjectTree),
                                )));
                                tasks.push(Task::done(ProjectAction::UiCommand(
                                    ProjectUiCommand::RequestProjectView(ProjectViewRequest::Overview),
                                )));

                                let mut tabs = self.project_tabs.lock().unwrap();
                                tabs.retain(|_key, kind|{
                                    !matches!(kind, ProjectTabKind::Process(tab) if tab.process.eq(&process_reference))
                                });
                                state.process_tab_uis.remove(&process);
                            }
                            Err(service_error) => {
                                tasks.push(Task::done(service_error));
                            }
                        }
                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    //
                    // form
                    //
                    Some(ProcessTabUiAction::Reset {
                        process_reference,
                    }) => self
                        .planner_core_service
                        .update(Event::RequestProcessDefinitionView {
                            process_reference,
                        })
                        .when_ok(key, |_| None),
                    Some(ProcessTabUiAction::Apply(args)) => {
                        // the process reference may have changed, use the updated reference
                        let updated_process_reference = args
                            .process_definition
                            .reference
                            .clone();

                        let process_reference_changed = !updated_process_reference.eq(&args.process_reference);

                        if process_reference_changed {
                            if state
                                .process_tab_uis
                                .contains_key(&updated_process_reference)
                            {
                                let now = chrono::DateTime::from(SystemTime::now());
                                return Some(ProjectAction::UiCommand(ProjectUiCommand::Error(PlannerError::Other(
                                    (now, tr!("process-error-name-already-in-use")),
                                ))));
                            }

                            debug!(
                                "process reference changed. old: {}, new: {}",
                                args.process_reference, updated_process_reference
                            );
                            // update the reference in the map
                            let (old_reference, _process_ui) = state
                                .process_tab_uis
                                .remove_entry(&args.process_reference)
                                .unwrap();
                            // if we just re-insert the ui with an updated reference, the ui's mapper will still have the old reference, so we need to update the mapper too, easier just to re-create the ui instance
                            //state.process_tab_uis.insert(updated_process_reference.clone(), process_ui);
                            self.ensure_process_inner(key, updated_process_reference.clone(), &mut state);

                            // update the tab's reference
                            let tabs = self.project_tabs.lock().unwrap();
                            let updated = tabs.filter_map_mut(|(_key, tab)| match tab {
                                ProjectTabKind::Process(tab) if tab.process.eq(&old_reference) => {
                                    tab.process = updated_process_reference.clone();
                                    Some(true)
                                }
                                _ => None,
                            });
                            assert!(matches!(updated.first(), Some(true)));
                        }

                        let mut tasks = vec![];

                        match self
                            .planner_core_service
                            .update(Event::ApplyProcessDefinition {
                                process_reference: args.process_reference,
                                process_definition: args.process_definition,
                            })
                            .into_actions()
                        {
                            Ok(actions) => {
                                let effect_tasks: Vec<Task<ProjectAction>> = actions
                                    .into_iter()
                                    .map(Task::done)
                                    .collect();
                                tasks.extend(effect_tasks);

                                tasks.push(Task::done(ProjectAction::UiCommand(ProjectUiCommand::ProcessChanged {
                                    process: updated_process_reference.clone(),
                                })));
                            }
                            Err(service_error) => {
                                tasks.push(Task::done(service_error));
                            }
                        }
                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                }
            }
            ProjectUiCommand::LoadOutTabUiCommand {
                load_out_source,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let load_out_ui = state
                    .load_out_tab_uis
                    .get_mut(&load_out_source)
                    .unwrap();

                let context = &mut LoadOutTabUiContext::default();
                let phase_ui_action = load_out_ui.update(command, context);

                match phase_ui_action {
                    Some(LoadOutTabUiAction::None) => None,
                    Some(LoadOutTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    None => None,
                    Some(LoadOutTabUiAction::UpdateFeederForPart {
                        phase,
                        part,
                        feeder,
                    }) => {
                        // Changing loadouts can create/clear issues.
                        request_issues_refresh = true;

                        debug!(
                            "update feeder. phase: {:?}, part: {:?}, feeder: {:?}",
                            phase, part, feeder
                        );
                        self.planner_core_service
                            .update(Event::AssignFeederToLoadOutItem {
                                phase,
                                feeder_reference: feeder,
                                manufacturer: exact_match(&part.manufacturer),
                                mpn: exact_match(&part.mpn),
                            })
                            .when_ok(key, |_| None)
                    }
                }
            }
            ProjectUiCommand::PlacementsTabUiCommand(command) => {
                let context = &mut PlacementsTabUiContext::default();
                let placements_ui_action = self
                    .project_ui_state
                    .lock()
                    .unwrap()
                    .placements_ui
                    .update(command, context);
                match placements_ui_action {
                    Some(PlacementsTabUiAction::None) => None,
                    Some(PlacementsTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PlacementsTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let (mut tasks, actions) = Self::update_placement(
                            &mut self.planner_core_service,
                            key,
                            object_path,
                            new_placement,
                            old_placement,
                        );
                        Self::handle_update_placement_actions(&mut tasks, actions, &mut request_issues_refresh);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PlacementsTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => self.locate_component(object_path, pcb_side, design_position, unit_position),
                    None => None,
                    Some(PlacementsTabUiAction::ApplyPlacementsAction(selection, action)) => {
                        let mut tasks = Vec::with_capacity(selection.len());
                        let mut actions = Vec::with_capacity(selection.len());

                        for item in selection {
                            let new_phase = match &action {
                                PlacementsTabUiApplyAction::RemovePhase(_phase) => None,
                                PlacementsTabUiApplyAction::ApplyPhase(phase) => Some(phase.clone()),
                            };

                            if item.state.phase.eq(&new_phase) {
                                continue;
                            }

                            let mut new_placement = item.state.clone();
                            new_placement.phase = new_phase;

                            let old_placement = item.state.clone();

                            let (item_tasks, item_actions) = Self::update_placement(
                                &mut self.planner_core_service,
                                key,
                                item.path,
                                new_placement,
                                old_placement,
                            );

                            tasks.extend(item_tasks);
                            actions.extend(item_actions);
                        }

                        actions.dedup_by(|a, b| a == b);

                        Self::handle_update_placement_actions(&mut tasks, actions, &mut request_issues_refresh);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                }
            }
            ProjectUiCommand::PcbTabUiCommand {
                pcb_index,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let pcb_ui = state
                    .pcb_tab_uis
                    .get_mut(&(pcb_index as usize))
                    .unwrap();

                let context = &mut PcbTabUiContext::default();
                let pcb_ui_action = pcb_ui.update(command, context);
                match pcb_ui_action {
                    None => None,
                    Some(PcbTabUiAction::None) => None,
                    Some(PcbTabUiAction::ShowUnitAssignments(pcb_index)) => Some(ProjectAction::Task(
                        key,
                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::ShowPcbUnitAssignments(
                            pcb_index,
                        ))),
                    )),
                    Some(PcbTabUiAction::RequestPcbOverview(path)) => Some(ProjectAction::Task(
                        key,
                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestPcbView(
                            PcbViewRequest::Overview {
                                path,
                            },
                        ))),
                    )),
                    Some(PcbTabUiAction::ShowPcb(pcb_path)) => Some(ProjectAction::ShowPcb(pcb_path)),
                }
            }
            ProjectUiCommand::UnitAssignmentsTabUiCommand {
                pcb_index,
                command,
            } => {
                let mut state = self.project_ui_state.lock().unwrap();
                let unit_assignment_ui = state
                    .unit_assignment_tab_uis
                    .get_mut(&(pcb_index as usize))
                    .unwrap();

                let context = &mut UnitAssignmentsTabUiContext::default();
                let unit_assignment_ui_action = unit_assignment_ui.update(command, context);
                match unit_assignment_ui_action {
                    None => None,
                    Some(UnitAssignmentsTabUiAction::None) => None,
                    Some(UnitAssignmentsTabUiAction::UpdateUnitAssignments(UpdateUnitAssignmentsArgs {
                        pcb_index,
                        variant_map,
                    })) => {
                        // Changing assignments can create/clear issues.
                        request_issues_refresh = true;

                        let mut events = vec![];

                        for (pcb_unit_index, variant_name) in variant_map.iter().enumerate() {
                            let mut object_path = ObjectPath::default();
                            object_path.set_pcb_instance(pcb_index + 1);
                            object_path.set_pcb_unit(pcb_unit_index as u16 + 1);

                            events.push(Event::AssignVariantToUnit {
                                variant: variant_name.clone(),
                                unit: object_path,
                            });
                        }

                        let mut tasks = vec![];
                        for event in events {
                            match self
                                .planner_core_service
                                .update(event)
                                .into_actions()
                            {
                                Ok(actions) => {
                                    let effect_tasks: Vec<Task<ProjectAction>> = actions
                                        .into_iter()
                                        .map(Task::done)
                                        .collect();
                                    tasks.extend(effect_tasks);
                                }
                                Err(service_error) => {
                                    tasks.push(Task::done(service_error));
                                    break;
                                }
                            }
                        }

                        let additional_tasks = vec![
                            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                ProjectViewRequest::ProjectTree,
                            ))),
                            Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                                ProjectViewRequest::Overview,
                            ))),
                        ];
                        tasks.extend(additional_tasks);

                        let action = ProjectAction::Task(key, Task::batch(tasks));

                        Some(action)
                    }
                    Some(UnitAssignmentsTabUiAction::RequestPcbOverview(path)) => Some(ProjectAction::Task(
                        key,
                        Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestPcbView(
                            PcbViewRequest::Overview {
                                path,
                            },
                        ))),
                    )),
                }
            }

            //
            // phases
            //
            ProjectUiCommand::RefreshPhase(phase) => {
                let tasks = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::Phases,
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::PhaseOverview {
                            phase: phase.clone(),
                        },
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::PhasePlacements {
                            phase: phase.clone(),
                        },
                    ))),
                ];
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::PhaseDeleted(phase) => {
                self.phases
                    .retain(|it| !it.phase_reference.eq(&phase));

                // close any phase placements tabs using this phase reference
                let mut tabs = self.project_tabs.lock().unwrap();
                tabs.retain(|_key, kind| !matches!(kind, ProjectTabKind::Phase(tab) if tab.phase.eq(&phase)));
                let mut ui_state = self.project_ui_state.lock().unwrap();
                ui_state.phases_tab_uis.remove(&phase);

                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::ProjectRefreshed));
                Some(ProjectAction::Task(key, task))
            }

            //
            // other
            //
            ProjectUiCommand::RefreshFromDesignVariants => {
                info!("Refreshing from design variants.");
                self.planner_core_service
                    .update(Event::RefreshFromDesignVariants)
                    .when_ok(key, |_| Some(ProjectUiCommand::ProjectRefreshed))
            }
            ProjectUiCommand::RefreshPcbs => {
                info!("Refreshing PCBs.");
                self.planner_core_service
                    .update(Event::RefreshPcbs)
                    .when_ok(key, |_| Some(ProjectUiCommand::PcbsRefreshed))
            }
            ProjectUiCommand::PcbsRefreshed => {
                info!("PCBs refreshed.");

                let mut tasks = vec![];
                for (index, _path) in self.pcbs.iter().enumerate() {
                    let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::PcbOverview {
                            pcb: index as u16,
                        },
                    )));
                    tasks.push(task);
                }

                // an updated PCB can effect designs and variants.
                let task = Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                    ProjectViewRequest::ProjectTree,
                )));
                tasks.push(task);

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            ProjectUiCommand::ProcessChanged {
                process,
            } => {
                info!("Process changed. process: {}", process);

                let tasks = vec![
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::ProjectTree,
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::Overview,
                    ))),
                    Task::done(ProjectAction::UiCommand(ProjectUiCommand::RequestProjectView(
                        ProjectViewRequest::ProcessDefinition {
                            process: process.clone(),
                        },
                    ))),
                ];

                let state = self.project_ui_state.lock().unwrap();
                // if a phase tab that uses the process is open, it needs to be refreshed now too
                for (_phase, tab_ui) in &state.phases_tab_uis {
                    tab_ui.on_process_changed(&process);
                }
                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
        };

        //
        // Issues refresh
        //
        if request_issues_refresh {
            self.component.send((
                key,
                ProjectUiCommand::RequestProjectView(ProjectViewRequest::ProjectReport),
            ))
        }

        action
    }
}

#[derive(Debug)]
pub struct ProjectUiState {
    /// initially unknown until the project is loaded
    /// always known for newly created projects.
    name: Option<String>,

    explorer_tab_ui: ExplorerTabUi,
    issues_ui: IssuesTabUi,
    load_out_tab_uis: HashMap<LoadOutSource, LoadOutTabUi>,
    overview_ui: OverviewTabUi,
    parts_tab_ui: PartsTabUi,
    pcb_tab_uis: HashMap<usize, PcbTabUi>,
    phases_tab_uis: HashMap<PhaseReference, PhaseTabUi>,
    placements_ui: PlacementsTabUi,
    process_tab_uis: HashMap<ProcessReference, ProcessTabUi>,
    unit_assignment_tab_uis: HashMap<usize, UnitAssignmentsTabUi>,
}

impl ProjectUiState {
    pub fn new(
        key: ProjectKey,
        project_directory: PathBuf,
        name: Option<String>,
        sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    ) -> Self {
        let mut instance = Self {
            name,
            explorer_tab_ui: ExplorerTabUi::new(project_directory),
            issues_ui: IssuesTabUi::new(),
            load_out_tab_uis: HashMap::default(),
            overview_ui: OverviewTabUi::new(),
            parts_tab_ui: PartsTabUi::new(),
            pcb_tab_uis: HashMap::default(),
            phases_tab_uis: HashMap::default(),
            placements_ui: PlacementsTabUi::new(),
            process_tab_uis: HashMap::default(),
            unit_assignment_tab_uis: HashMap::default(),
        };

        instance
            .explorer_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("explorer ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::ExplorerTabUiCommand(command))
            });

        instance
            .overview_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("overview ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::OverviewTabUiCommand(command))
            });

        instance
            .issues_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("issues ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::IssuesTabUiCommand(command))
            });

        instance
            .parts_tab_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("parts ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::PartsTabUiCommand(command))
            });

        instance
            .placements_ui
            .component
            .configure_mapper(sender.clone(), move |command| {
                trace!("placements ui mapper. command: {:?}", command);
                (key, ProjectUiCommand::PlacementsTabUiCommand(command))
            });

        instance
    }
}

// these should not contain state
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum ProjectTabKind {
    Explorer(ExplorerTab),
    Issues(IssuesTab),
    LoadOut(LoadOutTab),
    Overview(OverviewTab),
    Parts(PartsTab),
    Pcb(PcbTab),
    Phase(PhaseTab),
    Process(ProcessTab),
    Placements(PlacementsTab),
    UnitAssignments(UnitAssignmentsTab),
}

#[derive(Debug, Clone)]
pub enum ProjectUiCommand {
    None,

    SetModifiedState {
        project_modified: bool,
        pcbs_modified: bool,
    },

    //
    // errors
    //
    Error(PlannerError),
    ClearErrors,

    //
    // projects
    //
    Create,
    Created,
    Load,
    Loaded,
    Save,
    Saved,
    RequestProjectView(ProjectViewRequest),
    ProjectView(ProjectView),
    ProjectRefreshed,

    //
    // phases
    //
    RefreshPhases,
    RefreshPhase(PhaseReference),
    PhaseDeleted(PhaseReference),

    //
    // pcbs
    //
    RequestPcbView(PcbViewRequest),
    PcbView(PcbView),
    PcbFilePicked(PathBuf),
    PcbRemoved,

    //
    // toolbar
    //
    ToolbarCommand(ProjectToolbarUiCommand),

    //
    // modals
    //
    AddPhaseModalCommand(AddPhaseModalUiCommand),
    PackageSourcesModalCommand(PackageSourcesModalUiCommand),

    //
    // tabs
    //
    TabCommand(ProjectTabUiCommand),

    ShowExplorer,
    ExplorerTabUiCommand(ExplorerTabUiCommand),

    ShowIssues,
    IssuesTabUiCommand(IssuesTabUiCommand),

    ShowOverview,
    OverviewTabUiCommand(OverviewTabUiCommand),

    ShowParts,
    PartsTabUiCommand(PartsTabUiCommand),

    ShowPlacements,
    PlacementsTabUiCommand(PlacementsTabUiCommand),

    ShowPcb(PcbUnitIndex),
    PcbTabUiCommand {
        pcb_index: u16,
        command: PcbTabUiCommand,
    },

    ShowPhase(Reference),
    PhaseTabUiCommand {
        phase: Reference,
        command: PhaseTabUiCommand,
    },

    ShowPhaseLoadout {
        phase: Reference,
    },
    ContinueShowPhaseLoadout {
        phase: Reference,
    },
    LoadOutTabUiCommand {
        load_out_source: LoadOutSource,
        command: LoadOutTabUiCommand,
    },

    ShowPcbUnitAssignments(u16),
    UnitAssignmentsTabUiCommand {
        pcb_index: u16,
        command: UnitAssignmentsTabUiCommand,
    },

    ShowProcess(ProcessReference),
    ProcessTabUiCommand {
        process: Reference,
        command: ProcessTabUiCommand,
    },

    RefreshFromDesignVariants,
    RefreshPcbs,
    PcbsRefreshed,
    ProcessChanged {
        process: ProcessReference,
    },
}

fn project_path_from_view_path(view_path: &String) -> NavigationPath {
    let project_path = NavigationPath::new(format!("/project{}", view_path).to_string());
    project_path
}

fn view_path_from_project_path(project_path: &NavigationPath) -> Option<String> {
    let view_path = project_path
        .to_string()
        .split("/project")
        .collect::<Vec<&str>>()
        .get(1)?
        .to_string();
    Some(view_path)
}

fn exact_match(value: &str) -> Regex {
    Regex::new(format!("^{}$", regex::escape(value).as_str()).as_str()).unwrap()
}

mod core_helper {
    use crate::planner_app_core::{PlannerAction, PlannerError};
    use crate::project::{ProjectAction, ProjectKey, ProjectUiCommand};
    use crate::task::Task;

    #[must_use]
    fn when_ok_inner<F>(
        result: Result<Vec<PlannerAction>, PlannerError>,
        project_key: ProjectKey,
        f: F,
    ) -> Option<ProjectAction>
    where
        F: FnOnce(&mut Vec<Task<ProjectAction>>) -> Option<ProjectUiCommand>,
    {
        match result {
            Ok(actions) => {
                let mut tasks = vec![];
                let effect_tasks: Vec<Task<ProjectAction>> = actions
                    .into_iter()
                    .map(|planner_action| {
                        let project_action = into_project_action(planner_action);
                        Task::done(project_action)
                    })
                    .collect();

                tasks.extend(effect_tasks);

                if let Some(command) = f(&mut tasks) {
                    let final_task = Task::done(ProjectAction::UiCommand(command));
                    tasks.push(final_task);
                }

                let action = ProjectAction::Task(project_key, Task::batch(tasks));

                Some(action)
            }
            Err(error) => Some(ProjectAction::UiCommand(ProjectUiCommand::Error(error))),
        }
    }

    fn into_actions_inner(
        result: Result<Vec<PlannerAction>, PlannerError>,
    ) -> Result<Vec<ProjectAction>, ProjectAction> {
        match result {
            Ok(actions) => Ok(actions
                .into_iter()
                .map(into_project_action)
                .collect()),
            Err(error) => Err(ProjectAction::UiCommand(ProjectUiCommand::Error(error))),
        }
    }

    fn into_project_action(action: PlannerAction) -> ProjectAction {
        match action {
            PlannerAction::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => ProjectAction::UiCommand(ProjectUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            }),
            PlannerAction::ProjectView(project_view) => {
                ProjectAction::UiCommand(ProjectUiCommand::ProjectView(project_view))
            }
            PlannerAction::PcbView(pcb_view) => ProjectAction::UiCommand(ProjectUiCommand::PcbView(pcb_view)),
        }
    }

    pub trait ProjectCoreHelper {
        fn into_actions(self) -> Result<Vec<ProjectAction>, ProjectAction>;
        fn when_ok<F>(self, project_key: ProjectKey, f: F) -> Option<ProjectAction>
        where
            F: FnOnce(&mut Vec<Task<ProjectAction>>) -> Option<ProjectUiCommand>;
    }

    impl ProjectCoreHelper for Result<Vec<PlannerAction>, PlannerError> {
        fn into_actions(self) -> Result<Vec<ProjectAction>, ProjectAction> {
            into_actions_inner(self)
        }

        fn when_ok<F>(self, project_key: ProjectKey, f: F) -> Option<ProjectAction>
        where
            F: FnOnce(&mut Vec<Task<ProjectAction>>) -> Option<ProjectUiCommand>,
        {
            when_ok_inner(self, project_key, f)
        }
    }
}

pub(crate) fn make_tabs(key: ProjectKey) -> Value<ProjectTabs> {
    debug!("Initializing project tabs for tab. key: {:?}", key);
    let project_tabs = Value::new(ProjectTabs::default());

    project_tabs
}
