use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use derivative::Derivative;
use egui::Ui;
use egui_dock::Split;
use egui_i18n::tr;
use planner_app::{
    AddOrRemoveAction, Event, FileReference, LoadOutSource, ObjectPath, PcbSide, PcbUnitIndex, PcbView, PhaseReference,
    PlacementOperation, PlacementPositionUnit, PlacementState, PlacementStatus, ProcessReference, ProjectView,
    Reference, SetOrClearAction,
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

use crate::file_picker::{FileDialogs, Picker};
use crate::planner_app_core::{PlannerAction, PlannerCoreService, PlannerError};
use crate::project::core_helper::ProjectCoreHelper;
use crate::project::data::{ProjectData, ViewKey};
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
    ProcessDefinitionArgs, ProcessTab, ProcessTabUi, ProcessTabUiAction, ProcessTabUiCommand, ProcessTabUiContext,
};
use crate::project::tabs::{ProjectTabAction, ProjectTabContext, ProjectTabUiCommand, ProjectTabs};
use crate::project::toolbar::{ProjectToolbar, ProjectToolbarAction, ProjectToolbarUiCommand};
use crate::sync::{Enqueue, Value, ValueGuard};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::ui_util::NavigationPath;

//
// other modules
//
pub mod data;
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

    /// false until the core has a project, views can't be fetched until then.
    ready: bool,

    /// the core's generation when the project's data was last marked as stale
    seen_generation: u64,

    /// list of errors to show
    errors: Vec<(chrono::DateTime<chrono::Utc>, String)>,

    project_tabs: Value<ProjectTabs>,

    toolbar: ProjectToolbar,

    pub component: ComponentState<(ProjectKey, ProjectUiCommand)>,

    add_phase_modal: Option<AddPhaseModal>,
    package_sources_modal: Option<PackageSourcesModal>,

    file_picker: Value<Picker>,
    file_dialogs: Arc<dyn FileDialogs>,
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
        file_dialogs: Arc<dyn FileDialogs>,
    ) -> (Self, Vec<ProjectUiCommand>) {
        Self::new_inner(path, key, None, project_tabs, ProjectUiCommand::Load, file_dialogs)
    }

    pub fn new(
        name: String,
        path: PathBuf,
        key: ProjectKey,
        project_tabs: Value<ProjectTabs>,
        file_dialogs: Arc<dyn FileDialogs>,
    ) -> (Self, Vec<ProjectUiCommand>) {
        Self::new_inner(
            path,
            key,
            Some(name),
            project_tabs,
            ProjectUiCommand::Create,
            file_dialogs,
        )
    }

    fn new_inner(
        path: PathBuf,
        key: ProjectKey,
        name: Option<String>,
        project_tabs: Value<ProjectTabs>,
        initial_command: ProjectUiCommand,
        file_dialogs: Arc<dyn FileDialogs>,
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
        let seen_generation = core_service.generation();
        let mut instance = Self {
            path,
            planner_core_service: core_service,
            project_ui_state,
            modified: false,
            pcbs_modified: false,
            ready: false,
            seen_generation,
            errors: Default::default(),
            project_tabs,
            toolbar,
            component,
            add_phase_modal: None,
            package_sources_modal: None,
            file_picker: Value::new(Picker::new(file_dialogs.clone())),
            file_dialogs,
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

    pub fn show_explorer(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Explorer(_)));
        if result.is_err() {
            project_tabs.add_tab(ProjectTabKind::Explorer(ExplorerTab::default()));
        }
    }

    pub fn show_issues(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Issues(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Issues(IssuesTab::default()), 0.25, Split::Right);
        }
    }

    pub fn show_overview(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Overview(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Overview(OverviewTab::default()), 0.25, Split::Right);
        }
    }

    pub fn show_parts(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Parts(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(ProjectTabKind::Parts(PartsTab::default()), 0.25, Split::Right);
        }
    }

    pub fn show_placements(&mut self) {
        let mut project_tabs = self.project_tabs.lock().unwrap();
        let result = project_tabs.show_tab(|candidate_tab| matches!(candidate_tab, ProjectTabKind::Placements(_)));
        if result.is_err() {
            project_tabs.add_tab_to_leaf_or_split(
                ProjectTabKind::Placements(PlacementsTab::default()),
                0.25,
                Split::Right,
            );
        }
    }

    pub fn show_phase(&mut self, key: ProjectKey, phase: PhaseReference) {
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

        self.ensure_phase(key, &phase);
    }

    pub fn show_process(&mut self, key: ProjectKey, process: ProcessReference) {
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

        self.ensure_process(key, &process);
    }

    pub fn show_loadout(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) {
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

        self.ensure_load_out(key, phase, load_out_source);
    }

    pub fn show_pcb(&mut self, key: ProjectKey, pcb_index: u16) {
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

        self.ensure_pcb(key, pcb_index);
    }

    pub fn show_unit_assignments(&mut self, key: ProjectKey, pcb_index: u16) {
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

        self.ensure_unit_assignments(key, pcb_index);
    }

    fn ensure_phase(&self, key: ProjectKey, phase: &Reference) {
        let phase = phase.clone();
        let mut state = self.project_ui_state.lock().unwrap();
        state
            .phases_tab_uis
            .entry(phase.clone())
            .or_insert_with(|| {
                debug!("ensuring phase ui. phase: {:?}", phase);
                let mut phase_ui = PhaseTabUi::new(phase.clone());
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
    }

    fn ensure_process(&self, key: ProjectKey, process: &Reference) {
        let process = process.clone();
        let mut state = self.project_ui_state.lock().unwrap();

        self.ensure_process_inner(key, process, &mut state);
    }

    fn ensure_process_inner(&self, key: ProjectKey, process: Reference, state: &mut ValueGuard<ProjectUiState>) {
        state
            .process_tab_uis
            .entry(process.clone())
            .or_insert_with(|| {
                debug!("ensuring process ui. process: {:?}", process);
                let mut process_ui = ProcessTabUi::new(process.clone());
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
    }

    fn ensure_load_out(&self, key: ProjectKey, phase: Reference, load_out_source: &LoadOutSource) {
        let load_out_source = load_out_source.clone();
        let mut state = self.project_ui_state.lock().unwrap();

        state
            .load_out_tab_uis
            .entry(load_out_source.clone())
            .or_insert_with(|| {
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
    }

    fn ensure_pcb(&self, key: ProjectKey, pcb_index: u16) {
        let mut state = self.project_ui_state.lock().unwrap();

        state
            .pcb_tab_uis
            .entry(pcb_index as usize)
            .or_insert_with(|| {
                debug!("ensuring pcb ui. pcb_index: {:?}", pcb_index);
                let mut pcb_ui = PcbTabUi::new(self.path.clone(), pcb_index);
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
    }

    fn ensure_unit_assignments(&self, key: ProjectKey, pcb_index: u16) {
        let mut state = self.project_ui_state.lock().unwrap();

        state
            .unit_assignment_tab_uis
            .entry(pcb_index as usize)
            .or_insert_with(|| {
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
    }

    /// Closes the given tabs and removes their state.
    ///
    /// This must be done before the views are next fetched, the tabs would otherwise require views that no longer exist.
    fn close_tabs<F>(&self, should_close: F)
    where
        F: Fn(&ProjectTabKind) -> bool,
    {
        let mut project_tabs = self.project_tabs.lock().unwrap();

        project_tabs.retain(|_key, kind| !should_close(kind));

        // `retain` only removes the tabs from the dock, this removes them from the list of tabs and calls `on_close`.
        project_tabs.cleanup_tabs(&mut ProjectTabContext {
            state: self.project_ui_state.clone(),
        });
    }

    fn close_phase_tabs(&self, phase: &PhaseReference) {
        self.close_tabs(|kind| match kind {
            ProjectTabKind::Phase(tab) => tab.phase.eq(phase),
            ProjectTabKind::LoadOut(tab) => tab.phase.eq(phase),
            _ => false,
        });
    }

    /// When a pcb is removed the pcbs after it get a different index, so any tabs for them would show the wrong pcb.
    fn close_pcb_tabs_from(&self, pcb_index: u16) {
        self.close_tabs(|kind| match kind {
            ProjectTabKind::Pcb(tab) => tab.pcb_index >= pcb_index,
            ProjectTabKind::UnitAssignments(tab) => tab.pcb_index >= pcb_index,
            _ => false,
        });
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
            handle_placements,
            handle_process,
            handle_unit_assignments,
        ];

        handlers
            .iter()
            .find_map(|handler| handler(&key, &path))
    }

    /// The events required to change a placement from its old state to its new state.
    fn placement_events(object_path: &ObjectPath, new: &PlacementState, old: &PlacementState) -> Vec<Event> {
        let mut events = vec![];

        // it's possible that assigning/clearing a placement could make the phase complete
        if !new.phase.eq(&old.phase) {
            let assignment = match (&new.phase, &old.phase) {
                (Some(new_phase), _) => Some((new_phase, SetOrClearAction::Set)),
                (None, Some(old_phase)) => Some((old_phase, SetOrClearAction::Clear)),
                (None, None) => None,
            };

            if let Some((phase, operation)) = assignment {
                events.push(Event::AssignPlacementsToPhase {
                    phase: phase.clone(),
                    operation,
                    placements: exact_match(&object_path.to_string()),
                });
            }
        }

        if new.operation_status != old.operation_status {
            if new.phase.is_none() {
                error!(
                    "Attempt to place a placement that has not been assigned to a phase. placement: {:?}",
                    new
                );
            } else {
                let operation = match new.operation_status {
                    PlacementStatus::Placed => PlacementOperation::Place,
                    PlacementStatus::Skipped => PlacementOperation::Skip,
                    PlacementStatus::Pending => PlacementOperation::Reset,
                };

                events.push(Event::RecordPlacementsOperation {
                    object_path_patterns: vec![exact_match(&object_path.to_string())],
                    operation,
                });
            }
        }

        events
    }

    /// Runs the events in order, stopping at the first error.
    fn run_core_events(&mut self, events: Vec<Event>) -> Vec<Task<ProjectAction>> {
        let mut tasks = vec![];

        for event in events {
            match self
                .planner_core_service
                .update(event)
                .into_actions()
            {
                Ok(actions) => {
                    debug!("actions: {:?}", actions);
                    tasks.extend(actions.into_iter().map(Task::done));
                }
                Err(service_error) => {
                    tasks.push(Task::done(service_error));
                    break;
                }
            }
        }

        tasks
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

        let state = self.project_ui_state.lock().unwrap();

        state
            .data
            .overview
            .get()?
            .pcbs
            .get(pcb_index as usize)
            .map(|pcb| ProjectAction::LocateComponent {
                pcb_file: pcb_file_path(&self.path, &pcb.pcb_file),
                object_path,
                pcb_side,
                design_position,
                unit_position,
            })
    }

    fn show_add_phase_modal(&mut self, key: ProjectKey) {
        let processes = {
            let state = self.project_ui_state.lock().unwrap();
            state.data.processes().to_vec()
        };

        let mut modal = AddPhaseModal::new(self.path.clone(), processes, self.file_dialogs.clone());
        modal
            .component
            .configure_mapper(self.component.sender.clone(), move |command| {
                trace!("add phase modal mapper. command: {:?}", command);
                (key, ProjectUiCommand::AddPhaseModalCommand(command))
            });

        self.add_phase_modal = Some(modal);
    }

    fn show_package_sources_modal(&mut self, key: ProjectKey) {
        let library_config = {
            let state = self.project_ui_state.lock().unwrap();
            state
                .data
                .overview
                .get()
                .map(|overview| overview.library_config.clone())
        };

        if let Some(library_config) = library_config {
            let mut modal = PackageSourcesModal::new(
                self.path.clone(),
                library_config.package_source,
                library_config.package_mappings_source,
                self.file_dialogs.clone(),
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

/// A PCB inside the project's folder is stored relative to it, so the project keeps working if the folder is moved.
fn pcb_file_reference(project_path: &Path, pcb_path: PathBuf) -> FileReference {
    let project_directory = project_path.parent().unwrap();

    match pcb_path.strip_prefix(project_directory) {
        Ok(relative_path) => FileReference::Relative(relative_path.to_path_buf()),
        Err(_) => FileReference::Absolute(pcb_path),
    }
}

/// Relative paths are relative to the project's folder, not to the project's file.
fn pcb_file_path(project_path: &Path, pcb_file: &FileReference) -> PathBuf {
    pcb_file.build_path(
        &project_path
            .parent()
            .unwrap()
            .to_path_buf(),
    )
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
            let name = state
                .data
                .overview
                .get()
                .map(|overview| &overview.name)
                .or(state.name.as_ref());
            if let Some(name) = name {
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

        let action = self.handle_command(key, command);

        if self.refresh_views() {
            self.component
                .send((key, ProjectUiCommand::RepaintRequested));
        }

        action
    }
}

impl Project {
    /// Brings the data required by the open tabs up-to-date with the core, then lets components rebuild derived state.
    ///
    /// Runs after every command, so command handlers only need to change the core's state and do not need to know which
    /// views their change affects.
    ///
    /// Returns true if any views were fetched.
    fn refresh_views(&mut self) -> bool {
        if !self.ready {
            return false;
        }

        let project_tabs = self.project_tabs.lock().unwrap();
        let mut state = self.project_ui_state.lock().unwrap();

        Self::mark_stale_if_core_changed(&self.planner_core_service, &mut self.seen_generation, &mut state.data);

        let mut fetched = false;

        // this loops because some views can only be required after another view has been received, e.g. the path of a
        // pcb overview comes from the view linking the pcb to the project.
        loop {
            let required_by_tabs =
                project_tabs.filter_map(|(_tab_key, tab_kind)| Some(tab_kind.required_views(&state.data)));

            let missing = missing_views(required_by_tabs, &state.data);
            if missing.is_empty() {
                break;
            }

            for key in missing {
                Self::fetch_view(&mut self.planner_core_service, &mut self.errors, &mut state.data, key);
                fetched = true;
            }
        }

        state.sync_components();

        fetched
    }

    fn mark_stale_if_core_changed(
        planner_core_service: &PlannerCoreService,
        seen_generation: &mut u64,
        data: &mut ProjectData,
    ) {
        let generation = planner_core_service.generation();
        if generation != *seen_generation {
            *seen_generation = generation;
            data.mark_all_stale();
        }
    }

    /// After this, the view will either have been received or be recorded as failed, so it won't need fetching again.
    fn fetch_view(
        planner_core_service: &mut PlannerCoreService,
        errors: &mut Vec<(chrono::DateTime<chrono::Utc>, String)>,
        data: &mut ProjectData,
        key: ViewKey,
    ) {
        debug!("fetching view: {:?}", key);

        match planner_core_service.query(key.clone().into_event()) {
            Ok(actions) => {
                for action in actions {
                    match action {
                        PlannerAction::ProjectView(view) => data.apply_project_view(view),
                        PlannerAction::PcbView(view) => data.apply_pcb_view(view),
                        PlannerAction::SetModifiedState {
                            ..
                        } => {
                            // requesting a view doesn't modify anything.
                        }
                    }
                }

                data.record_fetched(&key);

                if data.needs_fetch(&key) {
                    error!("core did not provide the requested view. key: {:?}", key);
                    let _ = data.record_failed(key);
                }
            }
            Err(error) => {
                if data.record_failed(key) {
                    Self::record_error(errors, error);
                }
            }
        }
    }

    fn record_error(errors: &mut Vec<(chrono::DateTime<chrono::Utc>, String)>, error: PlannerError) {
        match error {
            PlannerError::CoreError(message) => errors.push(message),
            PlannerError::Other(message) => errors.push(message),
        }
    }

    fn on_project_opened(&mut self) {
        self.ready = true;

        self.show_explorer();
        self.show_overview();
        self.show_issues();
    }

    fn handle_command(&mut self, key: ProjectKey, command: ProjectUiCommand) -> Option<ProjectAction> {
        match command {
            ProjectUiCommand::None => None,
            ProjectUiCommand::Create => {
                let name = {
                    let state = self.project_ui_state.lock().unwrap();
                    state.name.clone().unwrap()
                };
                self.planner_core_service
                    .update(Event::CreateProject {
                        name,
                        path: self.path.clone(),
                        packages: None,
                        package_mappings: None,
                    })
                    .when_ok(key, |_| Some(ProjectUiCommand::Created))
            }
            ProjectUiCommand::Created => {
                self.on_project_opened();
                None
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
                self.on_project_opened();
                None
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
            ProjectUiCommand::SetModifiedState {
                project_modified,
                pcbs_modified,
            } => {
                self.modified = project_modified;
                self.pcbs_modified = pcbs_modified;
                // TODO remove the logical or here when AddPcbs has been reworked.
                Some(ProjectAction::SetModifiedState(project_modified || pcbs_modified))
            }
            ProjectUiCommand::RepaintRequested => Some(ProjectAction::RequestRepaint),

            //
            // errors
            //
            ProjectUiCommand::Error(error) => {
                Self::record_error(&mut self.errors, error);
                None
            }
            ProjectUiCommand::ClearErrors => {
                self.errors.clear();
                None
            }

            //
            // views
            //
            ProjectUiCommand::ProjectView(view) => {
                trace!("project view: {:?}", view);
                let mut state = self.project_ui_state.lock().unwrap();
                state.data.apply_project_view(view);
                None
            }
            ProjectUiCommand::PcbView(view) => {
                trace!("pcb view: {:?}", view);
                let mut state = self.project_ui_state.lock().unwrap();
                state.data.apply_pcb_view(view);
                None
            }

            //
            // toolbar
            //
            ProjectUiCommand::ToolbarCommand(toolbar_command) => {
                let action = self
                    .toolbar
                    .update(toolbar_command, &mut ());
                match action {
                    Some(ProjectToolbarAction::ShowProjectExplorer) => {
                        self.show_explorer();
                        None
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
                        .when_ok(key, |_| None),
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
                        .when_ok(key, |_| None),
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
                let pcb_file = pcb_file_reference(&self.path, pcb_path);

                self.planner_core_service
                    .update(Event::AddPcb {
                        pcb_file,
                    })
                    .when_ok(key, |_| None)
            }

            //
            // modals
            //
            ProjectUiCommand::AddPhaseModalCommand(command) => {
                if let Some(modal) = &mut self.add_phase_modal {
                    let action = modal.update(command, &mut ());
                    match action {
                        None => None,
                        Some(AddPhaseModalAction::Submit(args)) => {
                            self.add_phase_modal.take();

                            self.planner_core_service
                                .update(Event::CreatePhase {
                                    process: args.process,
                                    reference: args.reference,
                                    load_out: args.load_out,
                                    pcb_side: args.pcb_side,
                                })
                                .when_ok(key, |_| None)
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
                                .when_ok(key, |_| None)
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
                // the load-out source comes from the phases view, which may not have been received when tabs are being
                // restored at startup.
                let load_out_source = {
                    let mut state = self.project_ui_state.lock().unwrap();

                    Self::mark_stale_if_core_changed(
                        &self.planner_core_service,
                        &mut self.seen_generation,
                        &mut state.data,
                    );

                    if state.data.needs_fetch(&ViewKey::Phases) {
                        Self::fetch_view(
                            &mut self.planner_core_service,
                            &mut self.errors,
                            &mut state.data,
                            ViewKey::Phases,
                        );
                    }

                    state
                        .data
                        .phase_list()
                        .iter()
                        .find(|phase_overview| {
                            phase_overview
                                .phase_reference
                                .eq(&phase)
                        })
                        .map(|phase_overview| phase_overview.load_out_source.clone())
                };

                match load_out_source {
                    Some(load_out_source) => self.show_loadout(key, phase, &load_out_source),
                    None => error!("Unable to show load-out for unknown phase. phase: {:?}", phase),
                }
                None
            }
            ProjectUiCommand::ShowPcbUnitAssignments(pcb_index) => {
                self.show_unit_assignments(key, pcb_index);
                None
            }
            ProjectUiCommand::ShowExplorer => {
                self.show_explorer();
                None
            }
            ProjectUiCommand::ShowIssues => {
                self.show_issues();
                None
            }
            ProjectUiCommand::ShowOverview => {
                self.show_overview();
                None
            }
            ProjectUiCommand::ShowParts => {
                self.show_parts();
                None
            }
            ProjectUiCommand::ShowPlacements => {
                self.show_placements();
                None
            }
            ProjectUiCommand::ShowPhase(phase) => {
                self.show_phase(key, phase);
                None
            }
            ProjectUiCommand::ShowProcess(process) => {
                self.show_process(key, process);
                None
            }
            ProjectUiCommand::ShowPcb(pcb_index) => {
                self.show_pcb(key, pcb_index);
                None
            }

            ProjectUiCommand::ExplorerTabUiCommand(command) => {
                let explorer_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .explorer_tab_ui
                        .update(command, &mut ExplorerTabUiContext {
                            data: &state.data,
                        })
                };
                match explorer_ui_action {
                    None => None,
                    Some(ExplorerTabUiAction::Navigate(path)) => self.navigate(key, path),
                    Some(ExplorerTabUiAction::SetPhaseOrdering(phases)) => self
                        .planner_core_service
                        .update(Event::SetPhaseOrdering {
                            phases,
                        })
                        .when_ok(key, |_| None),
                }
            }
            ProjectUiCommand::IssuesTabUiCommand(command) => {
                let mut guard = self.project_ui_state.lock().unwrap();
                let state = &mut *guard;
                let issues_ui_action = state
                    .issues_ui
                    .update(command, &mut IssuesTabUiContext {
                        data: &state.data,
                    });
                match issues_ui_action {
                    None => {}
                    Some(IssuesTabUiAction::None) => {}
                    Some(IssuesTabUiAction::RefreshIssues) => state.data.mark_report_stale(),
                }
                None
            }
            ProjectUiCommand::OverviewTabUiCommand(command) => {
                let overview_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .overview_ui
                        .update(command, &mut OverviewTabUiContext {
                            data: &state.data,
                        })
                };
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
                    Some(OverviewTabUiAction::DeletePhase(reference)) => self
                        .planner_core_service
                        .update(Event::DeletePhase {
                            reference: reference.clone(),
                        })
                        .when_ok(key, |_| {
                            self.close_phase_tabs(&reference);
                            None
                        }),
                    Some(OverviewTabUiAction::RemovePcb(pcb_unit_index)) => self
                        .planner_core_service
                        .update(Event::RemovePcb {
                            index: pcb_unit_index,
                        })
                        .when_ok(key, |_| {
                            self.close_pcb_tabs_from(pcb_unit_index);
                            Some(ProjectUiCommand::PcbRemoved)
                        }),
                }
            }
            ProjectUiCommand::PartsTabUiCommand(command) => {
                let parts_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .parts_tab_ui
                        .update(command, &mut PartsTabUiContext {
                            data: &state.data,
                        })
                };
                match parts_ui_action {
                    Some(PartsTabUiAction::None) => None,
                    None => None,
                    Some(PartsTabUiAction::UpdateProcessesForPart {
                        part,
                        processes,
                    }) => {
                        let events = processes
                            .into_iter()
                            .map(|(process, enabled)| Event::AssignProcessToParts {
                                process,
                                operation: match enabled {
                                    true => AddOrRemoveAction::Add,
                                    false => AddOrRemoveAction::Remove,
                                },
                                manufacturer: exact_match(part.manufacturer.as_str()),
                                mpn: exact_match(part.mpn.as_str()),
                            })
                            .collect();

                        let tasks = self.run_core_events(events);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PartsTabUiAction::ApplyPartsAction(parts, apply_action)) => {
                        debug!("apply_action: {:?}", apply_action);

                        let (process, operation) = match apply_action {
                            PartsTabUiApplyAction::AddProcess(process) => (process, AddOrRemoveAction::Add),
                            PartsTabUiApplyAction::RemoveProcess(process) => (process, AddOrRemoveAction::Remove),
                        };

                        let events = parts
                            .into_iter()
                            .map(|part| Event::AssignProcessToParts {
                                process: process.clone(),
                                operation: operation.clone(),
                                manufacturer: exact_match(part.manufacturer.as_str()),
                                mpn: exact_match(part.mpn.as_str()),
                            })
                            .collect();

                        let tasks = self.run_core_events(events);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PartsTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                }
            }
            ProjectUiCommand::PhaseTabUiCommand {
                phase,
                command,
            } => {
                let phase_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .phases_tab_uis
                        .get_mut(&phase)?
                        .update(command, &mut PhaseTabUiContext {
                            data: &state.data,
                        })
                };

                match phase_ui_action {
                    None => None,
                    Some(PhaseTabUiAction::None) => None,
                    Some(PhaseTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PhaseTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let events = Self::placement_events(&object_path, &new_placement, &old_placement);
                        let tasks = self.run_core_events(events);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                    Some(PhaseTabUiAction::AddPartsToLoadout {
                        phase,
                        manufacturer_pattern,
                        mpn_pattern,
                    }) => self
                        .planner_core_service
                        .update(Event::AddPartsToLoadout {
                            phase,
                            manufacturer: manufacturer_pattern,
                            mpn: mpn_pattern,
                        })
                        .when_ok(key, |_| None),
                    Some(PhaseTabUiAction::SetPlacementOrderings(args)) => self
                        .planner_core_service
                        .update(Event::SetPlacementOrdering {
                            phase,
                            placement_orderings: args.orderings,
                        })
                        .when_ok(key, |_| None),
                    Some(PhaseTabUiAction::TaskAction {
                        phase,
                        operation,
                        task,
                        action,
                    }) => self
                        .planner_core_service
                        .update(Event::RecordPhaseOperation {
                            phase,
                            operation,
                            task,
                            action,
                        })
                        .when_ok(key, |_| None),
                    Some(PhaseTabUiAction::LocatePlacement {
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => self.locate_component(object_path, pcb_side, design_position, unit_position),
                }
            }
            ProjectUiCommand::ProcessTabUiCommand {
                process,
                command,
            } => {
                trace!("process: {:?}, command: {:?}", process, command);

                let process_ui_action = {
                    let mut state = self.project_ui_state.lock().unwrap();
                    state
                        .process_tab_uis
                        .get_mut(&process)?
                        .update(command, &mut ProcessTabUiContext::default())
                };

                match process_ui_action {
                    None => None,
                    Some(ProcessTabUiAction::None) => None,

                    Some(ProcessTabUiAction::Delete(process_reference)) => self
                        .planner_core_service
                        .update(Event::DeleteProcess {
                            process_reference: process_reference.clone(),
                        })
                        .when_ok(key, |_| {
                            self.close_tabs(
                                |kind| matches!(kind, ProjectTabKind::Process(tab) if tab.process.eq(&process_reference)),
                            );
                            None
                        }),
                    //
                    // form
                    //
                    Some(ProcessTabUiAction::Reset {
                        ..
                    }) => {
                        let mut guard = self.project_ui_state.lock().unwrap();
                        let state = &mut *guard;
                        if let Some(process_ui) = state.process_tab_uis.get_mut(&process) {
                            process_ui.reset_form(&state.data);
                        }
                        None
                    }
                    Some(ProcessTabUiAction::Apply(args)) => self.apply_process_definition(key, args),
                }
            }
            ProjectUiCommand::LoadOutTabUiCommand {
                load_out_source,
                command,
            } => {
                let load_out_ui_action = {
                    let mut state = self.project_ui_state.lock().unwrap();
                    state
                        .load_out_tab_uis
                        .get_mut(&load_out_source)?
                        .update(command, &mut LoadOutTabUiContext::default())
                };

                match load_out_ui_action {
                    Some(LoadOutTabUiAction::None) => None,
                    Some(LoadOutTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    None => None,
                    Some(LoadOutTabUiAction::UpdateFeederForPart {
                        phase,
                        part,
                        feeder,
                    }) => {
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
                let placements_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .placements_ui
                        .update(command, &mut PlacementsTabUiContext {
                            data: &state.data,
                        })
                };
                match placements_ui_action {
                    Some(PlacementsTabUiAction::None) => None,
                    Some(PlacementsTabUiAction::RequestRepaint) => Some(ProjectAction::RequestRepaint),
                    Some(PlacementsTabUiAction::UpdatePlacement {
                        object_path,
                        new_placement,
                        old_placement,
                    }) => {
                        let events = Self::placement_events(&object_path, &new_placement, &old_placement);
                        let tasks = self.run_core_events(events);

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

                        for item in selection {
                            let new_phase = match &action {
                                // the tab only sends the placements that are in the phase
                                PlacementsTabUiApplyAction::RemovePhase(_phase) => None,
                                PlacementsTabUiApplyAction::ApplyPhase(phase) => Some(phase.clone()),
                            };

                            if item.state.phase.eq(&new_phase) {
                                continue;
                            }

                            let mut new_placement = item.state.clone();
                            new_placement.phase = new_phase;

                            let events = Self::placement_events(&item.path, &new_placement, &item.state);
                            tasks.extend(self.run_core_events(events));
                        }

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                }
            }
            ProjectUiCommand::PcbTabUiCommand {
                pcb_index,
                command,
            } => {
                let pcb_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .pcb_tab_uis
                        .get_mut(&(pcb_index as usize))?
                        .update(command, &mut PcbTabUiContext {
                            data: &state.data,
                        })
                };
                match pcb_ui_action {
                    None => None,
                    Some(PcbTabUiAction::None) => None,
                    Some(PcbTabUiAction::ShowUnitAssignments(pcb_index)) => {
                        self.show_unit_assignments(key, pcb_index);
                        None
                    }
                    Some(PcbTabUiAction::ShowPcb(pcb_path)) => Some(ProjectAction::ShowPcb(pcb_path)),
                }
            }
            ProjectUiCommand::UnitAssignmentsTabUiCommand {
                pcb_index,
                command,
            } => {
                let unit_assignment_ui_action = {
                    let mut guard = self.project_ui_state.lock().unwrap();
                    let state = &mut *guard;
                    state
                        .unit_assignment_tab_uis
                        .get_mut(&(pcb_index as usize))?
                        .update(command, &mut UnitAssignmentsTabUiContext {
                            data: &state.data,
                        })
                };
                match unit_assignment_ui_action {
                    None => None,
                    Some(UnitAssignmentsTabUiAction::None) => None,
                    Some(UnitAssignmentsTabUiAction::UpdateUnitAssignments(UpdateUnitAssignmentsArgs {
                        pcb_index,
                        variant_map,
                    })) => {
                        let events = variant_map
                            .iter()
                            .enumerate()
                            .map(|(pcb_unit_index, variant_name)| {
                                let mut object_path = ObjectPath::default();
                                object_path.set_pcb_instance(pcb_index + 1);
                                object_path.set_pcb_unit(pcb_unit_index as u16 + 1);

                                Event::AssignVariantToUnit {
                                    variant: variant_name.clone(),
                                    unit: object_path,
                                }
                            })
                            .collect();

                        let tasks = self.run_core_events(events);

                        Some(ProjectAction::Task(key, Task::batch(tasks)))
                    }
                }
            }

            //
            // other
            //
            ProjectUiCommand::RefreshFromDesignVariants => {
                info!("Refreshing from design variants.");
                self.planner_core_service
                    .update(Event::RefreshFromDesignVariants)
                    .when_ok(key, |_| None)
            }
            ProjectUiCommand::RefreshPcbs => {
                info!("Refreshing PCBs.");
                self.planner_core_service
                    .update(Event::RefreshPcbs)
                    .when_ok(key, |_| None)
            }
        }
    }

    fn apply_process_definition(&mut self, key: ProjectKey, args: ProcessDefinitionArgs) -> Option<ProjectAction> {
        // the process reference may have changed, use the updated reference
        let updated_process_reference = args
            .process_definition
            .reference
            .clone();

        let old_process_reference = args.process_reference.clone();

        let process_reference_changed = !updated_process_reference.eq(&old_process_reference);

        if process_reference_changed {
            let state = self.project_ui_state.lock().unwrap();
            if state
                .process_tab_uis
                .contains_key(&updated_process_reference)
            {
                let now = chrono::DateTime::from(SystemTime::now());
                return Some(ProjectAction::UiCommand(ProjectUiCommand::Error(PlannerError::Other(
                    (now, tr!("process-error-name-already-in-use")),
                ))));
            }
        }

        match self
            .planner_core_service
            .update(Event::ApplyProcessDefinition {
                process_reference: args.process_reference,
                process_definition: args.process_definition,
            })
            .into_actions()
        {
            Ok(actions) => {
                if process_reference_changed {
                    debug!(
                        "process reference changed. old: {}, new: {}",
                        old_process_reference, updated_process_reference
                    );
                    self.rename_process_tab(key, &old_process_reference, &updated_process_reference);
                }

                let tasks = actions
                    .into_iter()
                    .map(Task::done)
                    .collect::<Vec<Task<ProjectAction>>>();

                Some(ProjectAction::Task(key, Task::batch(tasks)))
            }
            Err(service_error) => Some(service_error),
        }
    }

    fn rename_process_tab(&self, key: ProjectKey, old_reference: &ProcessReference, new_reference: &ProcessReference) {
        // the ui's mapper and the process it reads from contain the reference, so it's easier just to re-create the ui
        // instance instead of re-inserting it with an updated reference.
        {
            let mut state = self.project_ui_state.lock().unwrap();
            state
                .process_tab_uis
                .remove(old_reference);
            self.ensure_process_inner(key, new_reference.clone(), &mut state);
        }

        // update the tab's reference
        let tabs = self.project_tabs.lock().unwrap();
        let updated = tabs.filter_map_mut(|(_key, tab)| match tab {
            ProjectTabKind::Process(tab) if tab.process.eq(old_reference) => {
                tab.process = new_reference.clone();
                Some(true)
            }
            _ => None,
        });
        assert!(matches!(updated.first(), Some(true)));
    }
}

#[derive(Debug)]
pub struct ProjectUiState {
    /// the name given when creating a new project, otherwise the name comes from the overview view.
    name: Option<String>,

    /// the data received from the core, components read from this instead of keeping their own copies.
    data: ProjectData,

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
    /// Gives components that hold state derived from views a chance to rebuild it.
    ///
    /// Runs after commands have been handled, `ui` methods must never do this.
    fn sync_components(&mut self) {
        let Self {
            data,
            load_out_tab_uis,
            parts_tab_ui,
            phases_tab_uis,
            placements_ui,
            process_tab_uis,
            unit_assignment_tab_uis,
            ..
        } = self;

        parts_tab_ui.sync(data);
        placements_ui.sync(data);
        load_out_tab_uis
            .values_mut()
            .for_each(|tab_ui| tab_ui.sync(data));
        phases_tab_uis
            .values_mut()
            .for_each(|tab_ui| tab_ui.sync(data));
        process_tab_uis
            .values_mut()
            .for_each(|tab_ui| tab_ui.sync(data));
        unit_assignment_tab_uis
            .values_mut()
            .for_each(|tab_ui| tab_ui.sync(data));
    }

    pub fn new(
        key: ProjectKey,
        project_directory: PathBuf,
        name: Option<String>,
        sender: Enqueue<(ProjectKey, ProjectUiCommand)>,
    ) -> Self {
        let mut instance = Self {
            name,
            data: ProjectData::default(),
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

impl ProjectTabKind {
    /// The views this tab needs, these are what get fetched when the core's state changes.
    ///
    /// This is the only place that needs to know which views a tab depends on, command handlers do not need to know.
    pub fn required_views(&self, data: &ProjectData) -> Vec<ViewKey> {
        // the path of a pcb is only known after the view that links the pcb to the project has been received.
        fn pcb_overview(pcb_index: u16, data: &ProjectData) -> Option<ViewKey> {
            data.project_pcb(pcb_index)
                .map(|pcb| ViewKey::PcbOverview(pcb.pcb_path.clone()))
        }

        match self {
            ProjectTabKind::Explorer(_tab) => vec![ViewKey::ProjectTree],
            ProjectTabKind::Issues(_tab) => vec![ViewKey::ProjectReport],
            ProjectTabKind::LoadOut(tab) => vec![ViewKey::PhaseLoadOut(tab.phase.clone())],
            ProjectTabKind::Overview(_tab) => vec![ViewKey::Overview, ViewKey::Phases],
            ProjectTabKind::Parts(_tab) => vec![ViewKey::Overview, ViewKey::Parts],
            ProjectTabKind::Pcb(tab) => [
                Some(ViewKey::ProjectPcbOverview(tab.pcb_index)),
                pcb_overview(tab.pcb_index, data),
            ]
            .into_iter()
            .flatten()
            .collect(),
            ProjectTabKind::Phase(tab) => vec![
                ViewKey::Phases,
                ViewKey::PhaseOverview(tab.phase.clone()),
                ViewKey::PhasePlacements(tab.phase.clone()),
            ],
            ProjectTabKind::Placements(_tab) => vec![ViewKey::Phases, ViewKey::Placements],
            ProjectTabKind::Process(tab) => vec![ViewKey::ProcessDefinition(tab.process.clone())],
            ProjectTabKind::UnitAssignments(tab) => [
                Some(ViewKey::ProjectPcbOverview(tab.pcb_index)),
                Some(ViewKey::PcbUnitAssignments(tab.pcb_index)),
                pcb_overview(tab.pcb_index, data),
            ]
            .into_iter()
            .flatten()
            .collect(),
        }
    }
}

/// The views that need to be fetched, given what each open tab requires.
fn missing_views(required_by_tabs: Vec<Vec<ViewKey>>, data: &ProjectData) -> Vec<ViewKey> {
    // the project itself always needs these: the project name, processes, pcbs and library config for the toolbar actions
    // and modals, and the phases to be able to show load-outs.
    let required_by_project = vec![ViewKey::Overview, ViewKey::Phases];

    let mut seen = HashSet::new();

    required_by_project
        .into_iter()
        .chain(required_by_tabs.into_iter().flatten())
        .filter(|key| data.needs_fetch(key))
        .filter(|key| seen.insert(key.clone()))
        .collect()
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
    ProjectView(ProjectView),
    RepaintRequested,

    //
    // pcbs
    //
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use planner_app::{FileReference, ProjectPcbOverview, ProjectView, Reference};

    use super::data::{ProjectData, ViewKey};
    use super::tabs::parts_tab::PartsTab;
    use super::tabs::pcb_tab::PcbTab;
    use super::tabs::phase_tab::PhaseTab;
    use super::{ProjectTabKind, missing_views, pcb_file_path, pcb_file_reference};

    #[test]
    fn a_pcb_in_the_projects_folder_is_stored_relative_to_it() {
        let project = PathBuf::from("folder").join("project.mpnp.json");
        let pcb = PathBuf::from("folder")
            .join("pcbs")
            .join("panel.pcb.json");

        let reference = pcb_file_reference(&project, pcb.clone());

        assert_eq!(
            reference,
            FileReference::Relative(PathBuf::from("pcbs").join("panel.pcb.json"))
        );
        assert_eq!(pcb_file_path(&project, &reference), pcb);
    }

    #[test]
    fn a_pcb_outside_the_projects_folder_is_stored_as_given() {
        let project = PathBuf::from("folder").join("project.mpnp.json");
        let pcb = PathBuf::from("elsewhere").join("panel.pcb.json");

        let reference = pcb_file_reference(&project, pcb.clone());

        assert_eq!(reference, FileReference::Absolute(pcb.clone()));
        assert_eq!(pcb_file_path(&project, &reference), pcb);
    }

    #[test]
    fn a_phase_tab_requires_its_views_and_the_phases() {
        let phase = Reference::from_raw_str("top_1");
        let tab = ProjectTabKind::Phase(PhaseTab::new(phase.clone()));

        let required = tab.required_views(&ProjectData::default());

        assert_eq!(required, vec![
            ViewKey::Phases,
            ViewKey::PhaseOverview(phase.clone()),
            ViewKey::PhasePlacements(phase),
        ]);
    }

    #[test]
    fn a_pcb_tab_requires_the_pcb_overview_once_the_path_is_known() {
        let tab = ProjectTabKind::Pcb(PcbTab::new(0));
        let mut data = ProjectData::default();

        assert_eq!(tab.required_views(&data), vec![ViewKey::ProjectPcbOverview(0)]);

        let path = PathBuf::from("a.pcb.json");
        data.apply_project_view(ProjectView::PcbOverview(ProjectPcbOverview {
            index: 0,
            pcb_file: FileReference::Absolute(path.clone()),
            pcb_path: path.clone(),
        }));

        assert_eq!(tab.required_views(&data), vec![
            ViewKey::ProjectPcbOverview(0),
            ViewKey::PcbOverview(path),
        ]);
    }

    #[test]
    fn missing_views_includes_views_the_project_always_needs_without_duplicates() {
        let tab = ProjectTabKind::Parts(PartsTab::default());
        let data = ProjectData::default();

        let missing = missing_views(vec![tab.required_views(&data)], &data);

        assert_eq!(missing, vec![ViewKey::Overview, ViewKey::Phases, ViewKey::Parts]);
    }

    #[test]
    fn missing_views_excludes_views_that_failed() {
        let mut data = ProjectData::default();
        let _ = data.record_failed(ViewKey::Overview);

        let missing = missing_views(vec![], &data);

        assert_eq!(missing, vec![ViewKey::Phases]);
    }
}

/// Drives a real core, these cover how changes made by commands reach the data used by the tabs.
#[cfg(test)]
mod refresh_tests {
    use std::collections::VecDeque;
    use std::path::PathBuf;

    use futures::StreamExt;
    use planner_app::{Event, LoadOutSource, PcbSide, PlacementSortingItem, PlacementSortingMode, Reference};
    use tempfile::TempDir;
    use util::sorting::SortOrder;

    use super::tabs::overview_tab::OverviewTabUiCommand;
    use super::tabs::process_tab::ProcessDefinitionArgs;
    use super::*;
    use crate::task;

    fn run(project: &mut Project, key: ProjectKey, command: ProjectUiCommand) {
        let mut queue = VecDeque::from([command]);

        while let Some(command) = queue.pop_front() {
            let action = project.update((key, command), &mut ProjectContext {
                key,
            });

            match action {
                Some(ProjectAction::UiCommand(command)) => queue.push_back(command),
                Some(ProjectAction::Task(_key, task)) => {
                    if let Some(stream) = task::into_stream(task) {
                        for action in futures::executor::block_on(stream.collect::<Vec<_>>()) {
                            match action {
                                ProjectAction::UiCommand(command) => queue.push_back(command),
                                other => panic!("unsupported action in task: {:?}", other),
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn new_project() -> (Project, ProjectKey, TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let key = ProjectKey::default();

        let (mut project, commands) = Project::new(
            "test".to_string(),
            directory.path().join("test.mpnp.json"),
            key,
            make_tabs(key),
            Arc::new(crate::file_picker::NativeFileDialogs),
        );

        for command in commands {
            run(&mut project, key, command);
        }

        (project, key, directory)
    }

    fn create_phase(project: &mut Project, directory: &TempDir, phase: &Reference) {
        std::fs::write(directory.path().join("load_out.csv"), "").unwrap();

        let load_out =
            LoadOutSource::try_from_path(&directory.path().to_path_buf(), PathBuf::from("load_out.csv")).unwrap();

        project
            .planner_core_service
            .update(Event::CreatePhase {
                process: Reference::from_raw_str("pnp"),
                reference: phase.clone(),
                load_out,
                pcb_side: PcbSide::Top,
            })
            .unwrap();
    }

    /// a command that does nothing, views are refreshed after every command.
    fn refresh(project: &mut Project, key: ProjectKey) {
        run(project, key, ProjectUiCommand::None);
    }

    fn revision_of_tree(project: &Project) -> u64 {
        project
            .project_ui_state
            .lock()
            .unwrap()
            .data
            .project_tree
            .revision()
    }

    #[test]
    fn opening_a_project_fetches_the_views_for_the_default_tabs() {
        let (project, _key, _directory) = new_project();

        assert!(project.errors.is_empty(), "{:?}", project.errors);

        let state = project.project_ui_state.lock().unwrap();
        assert_eq!(state.data.overview.get().unwrap().name, "test");
        assert!(state.data.project_tree.get().is_some());
        assert!(state.data.report.get().is_some());
        assert!(state.data.phases.get().is_some());
    }

    #[test]
    fn a_change_reaches_an_open_phase_tab_without_the_handler_requesting_any_views() {
        let (mut project, key, directory) = new_project();
        let phase = Reference::from_raw_str("top_1");
        create_phase(&mut project, &directory, &phase);
        run(&mut project, key, ProjectUiCommand::ShowPhase(phase.clone()));

        let orderings = vec![PlacementSortingItem {
            mode: PlacementSortingMode::RefDes,
            sort_order: SortOrder::Asc,
        }];

        project
            .planner_core_service
            .update(Event::SetPlacementOrdering {
                phase: phase.clone(),
                placement_orderings: orderings.clone(),
            })
            .unwrap();
        refresh(&mut project, key);

        assert!(project.errors.is_empty(), "{:?}", project.errors);

        let state = project.project_ui_state.lock().unwrap();
        assert_eq!(
            state
                .data
                .phase_overview(&phase)
                .unwrap()
                .phase_placement_orderings,
            orderings
        );
        assert_eq!(
            state.data.phase_list()[0].phase_placement_orderings,
            orderings,
            "the same data is used by other tabs"
        );
    }

    #[test]
    fn a_change_that_does_not_affect_a_view_does_not_change_its_revision() {
        let (mut project, key, _directory) = new_project();
        let revision = revision_of_tree(&project);

        project
            .planner_core_service
            .update(Event::None)
            .unwrap();
        assert_ne!(
            project
                .planner_core_service
                .generation(),
            project.seen_generation
        );

        refresh(&mut project, key);

        assert_eq!(
            project
                .planner_core_service
                .generation(),
            project.seen_generation
        );
        assert_eq!(revision_of_tree(&project), revision);
    }

    #[test]
    fn deleting_a_phase_closes_its_tabs_without_errors() {
        let (mut project, key, directory) = new_project();
        let phase = Reference::from_raw_str("top_1");
        create_phase(&mut project, &directory, &phase);
        run(&mut project, key, ProjectUiCommand::ShowPhase(phase.clone()));
        run(&mut project, key, ProjectUiCommand::ShowPhaseLoadout {
            phase: phase.clone(),
        });

        run(
            &mut project,
            key,
            ProjectUiCommand::OverviewTabUiCommand(OverviewTabUiCommand::PhaseDeleteClicked(phase.clone())),
        );

        assert!(project.errors.is_empty(), "{:?}", project.errors);

        let open_tabs = project
            .project_tabs
            .lock()
            .unwrap()
            .filter_map(|(_key, tab)| match tab {
                ProjectTabKind::Phase(_) | ProjectTabKind::LoadOut(_) => Some(()),
                _ => None,
            });
        assert!(open_tabs.is_empty());

        let state = project.project_ui_state.lock().unwrap();
        assert!(state.phases_tab_uis.is_empty());
        assert!(state.load_out_tab_uis.is_empty());
        assert!(state.data.phase_list().is_empty());
    }

    #[test]
    fn showing_a_load_out_gets_the_source_from_the_phases() {
        let (mut project, key, directory) = new_project();
        let phase = Reference::from_raw_str("top_1");
        create_phase(&mut project, &directory, &phase);

        run(&mut project, key, ProjectUiCommand::ShowPhaseLoadout {
            phase: phase.clone(),
        });

        assert!(project.errors.is_empty(), "{:?}", project.errors);

        let state = project.project_ui_state.lock().unwrap();
        assert_eq!(state.load_out_tab_uis.len(), 1);
        assert!(state.data.load_out(&phase).is_some());
    }

    #[test]
    fn showing_a_load_out_for_an_unknown_phase_does_not_retry_forever() {
        let (mut project, key, _directory) = new_project();

        run(&mut project, key, ProjectUiCommand::ShowPhaseLoadout {
            phase: Reference::from_raw_str("unknown"),
        });

        assert!(
            project
                .project_ui_state
                .lock()
                .unwrap()
                .load_out_tab_uis
                .is_empty()
        );
    }

    #[test]
    fn a_tab_whose_view_keeps_failing_only_reports_the_error_once() {
        let (mut project, key, _directory) = new_project();

        run(
            &mut project,
            key,
            ProjectUiCommand::ShowPhase(Reference::from_raw_str("unknown")),
        );
        let reported = project.errors.len();
        assert!(reported > 0);

        project
            .planner_core_service
            .update(Event::None)
            .unwrap();
        refresh(&mut project, key);

        assert_eq!(project.errors.len(), reported);
    }

    #[test]
    fn views_are_not_fetched_when_a_project_fails_to_load() {
        let directory = tempfile::tempdir().unwrap();
        let key = ProjectKey::default();
        let (mut project, commands) = Project::from_path(
            directory
                .path()
                .join("missing.mpnp.json"),
            key,
            make_tabs(key),
            Arc::new(crate::file_picker::NativeFileDialogs),
        );

        for command in commands {
            run(&mut project, key, command);
        }

        assert_eq!(project.errors.len(), 1, "{:?}", project.errors);
        assert!(!project.ready);
    }

    #[test]
    fn renaming_a_process_moves_its_tab_to_the_new_reference() {
        let (mut project, key, _directory) = new_project();
        let old_reference = Reference::from_raw_str("pnp");
        let new_reference = Reference::from_raw_str("renamed");

        run(&mut project, key, ProjectUiCommand::ShowProcess(old_reference.clone()));

        let mut process_definition = project
            .project_ui_state
            .lock()
            .unwrap()
            .data
            .process_definition(&old_reference)
            .unwrap()
            .clone();
        process_definition.reference = new_reference.clone();

        let action = project.apply_process_definition(key, ProcessDefinitionArgs {
            process_reference: old_reference.clone(),
            process_definition,
        });
        assert!(matches!(action, Some(ProjectAction::Task(..))));
        refresh(&mut project, key);

        assert!(project.errors.is_empty(), "{:?}", project.errors);

        let state = project.project_ui_state.lock().unwrap();
        assert!(
            !state
                .process_tab_uis
                .contains_key(&old_reference)
        );
        assert!(
            state
                .process_tab_uis
                .contains_key(&new_reference)
        );
        assert!(
            state
                .data
                .process_definition(&new_reference)
                .is_some()
        );
    }
}
