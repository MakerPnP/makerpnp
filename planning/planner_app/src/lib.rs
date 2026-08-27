use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Debug;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::{anyhow, Error};
pub use args::Arg;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
pub use crux_core::Core;
use crux_core::{render, App, Command};
use gerber::GerberFile;
pub use gerber::{GerberFileFunction, GerberFileFunctionDiscriminants, PcbSideRequirement};
use indexmap::IndexSet;
use nalgebra::Vector2;
use package_mapper::package_mapping::PackageMapping;
use petgraph::Graph;
pub use planning::actions::{AddOrRemoveAction, SetOrClearAction};
pub use planning::design::{DesignIndex, DesignName, DesignNumber, DesignVariant};
pub use planning::file::{FileReference, FileReferenceError};
pub use planning::library::LibraryConfig;
use planning::pcb::{Pcb, PcbError};
pub use planning::pcb::{PcbAssemblyFlip, PcbAssemblyOrientation};
pub use planning::phase::PhaseReference;
pub use planning::phase::PhaseStatus;
use planning::phase::{Phase, PhaseError, PhaseState};
pub use planning::placement::PlacementSortingItem;
pub use planning::placement::PlacementSortingMode;
pub use planning::placement::PlacementStatus;
pub use planning::placement::ProjectPlacementStatus;
pub use planning::placement::{PlacementOperation, PlacementState};
use planning::process::ProcessError;
pub use planning::process::ProcessReference;
pub use planning::process::TaskReference;
pub use planning::process::TaskStatus;
pub use planning::process::{
    OperationDefinition, OperationReference, OperationStatus, ProcessDefinition, ProcessRuleReference, TaskAction,
};
use planning::project::{
    PartStateError, PcbOperationError, ProcessPresetFactory, ProcessPresetFactoryError, Project, ProjectError,
    ProjectPcb,
};
pub use planning::report::{IssueKind, IssueSeverity, ProjectReport};
pub use planning::variant::VariantName;
use planning::{file, pcb, project, report};
pub use pnp::load_out::LoadOutItem;
pub use pnp::object_path::ObjectPath;
use pnp::package::Package;
pub use pnp::panel::{DesignSizing, Dimensions, FiducialParameters, PanelSizing, PcbUnitPositioning, Unit};
pub use pnp::part::Part;
pub use pnp::pcb::PcbSide;
pub use pnp::pcb::{PcbInstanceIndex, PcbInstanceNumber, PcbUnitIndex, PcbUnitNumber};
pub use pnp::placement::RefDes;
pub use pnp::placement::{Placement, PlacementPosition, PlacementPositionUnit};
pub use pnp::reference::Reference;
use regex::Regex;
use serde_with::serde_as;
use stores::load_out::LoadOutOperationError;
pub use stores::load_out::LoadOutSource;
pub use stores::package_mappings::PackageMappingsSource;
pub use stores::packages::PackagesSource;
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};
use util::source::SourceError;

use crate::effects::pcb_view_renderer::PcbViewRendererOperation;
use crate::effects::project_view_renderer::ProjectViewRendererOperation;
use crate::effects::{pcb_view_renderer, project_view_renderer};

pub mod effects;

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct ModelProject {
    path: PathBuf,
    project_directory: PathBuf,
    project: Project,
    modified: bool,
}

impl ModelProject {
    fn pcbs<'a, 'b>(&'a self, model_pcbs: &'b ModelPcbs) -> impl Iterator<Item = Option<&'b ModelPcb>> + use<'a, 'b> {
        self.project
            .pcbs
            .iter()
            .map(|project_pcb| {
                let pcb_path = project_pcb
                    .pcb_file
                    .build_path(&self.project_directory);
                model_pcbs.get(&pcb_path)
            })
    }
}

pub struct ModelPcb {
    pcb: Pcb,
    modified: bool,
}

type ModelPcbs = BTreeMap<PathBuf, ModelPcb>;

#[derive(Default)]
pub struct Model {
    model_project: Option<ModelProject>,
    /// PCBs that have been created/loaded.
    ///
    /// Important: Can contain instances of [`ModelPcb`] that have been created or loaded, but not assigned to a project yet.
    model_pcbs: ModelPcbs,

    error: Option<(chrono::DateTime<chrono::Utc>, String)>,
}

impl Model {
    /// an iterator over the pcbs for the project
    ///
    /// will return Some(None) for any PCB that hasn't been loaded.
    #[allow(dead_code)]
    fn project_model_pcbs(&self) -> impl Iterator<Item = Option<&ModelPcb>> + '_ {
        Self::project_model_pcbs_maybe(&self.model_project, &self.model_pcbs)
    }

    fn project_model_pcbs_maybe<'a, 'b>(
        model_project: &'a Option<ModelProject>,
        model_pcbs: &'b ModelPcbs,
    ) -> impl Iterator<Item = Option<&'b ModelPcb>> + use<'a, 'b> {
        model_project
            .as_ref()
            .into_iter()
            .flat_map(|model_project| model_project.pcbs(model_pcbs))
    }

    // FUTURE consider adding a 'refresh_project_pcbs' that re-loads them.

    /// Load PCBs for the project that are not already loaded.
    ///
    /// * Requires a project to be loaded.
    /// * Stops and returns on the first error.
    fn load_unloaded_project_pcbs(&mut self, root: &PathBuf) -> Result<(), AppError> {
        let Some(model_project) = &mut self.model_project else {
            return Err(AppError::OperationRequiresProject);
        };

        Self::load_project_pcbs_inner(&mut model_project.project, &mut self.model_pcbs, root)
    }

    fn load_project_pcbs_inner(
        project: &mut Project,
        model_pcbs: &mut ModelPcbs,
        root: &PathBuf,
    ) -> Result<(), AppError> {
        let pcbs_to_load = project
            .pcbs
            .iter_mut()
            .filter(|pcb| {
                let pcb_path = pcb.pcb_file.build_path(root);
                !model_pcbs.contains_key(&pcb_path)
            })
            .collect::<Vec<_>>();

        // Then, process one-by-one: load and insert
        for pcb in pcbs_to_load {
            let (_pcb_file, pcb_data, pcb_path) = pcb
                .load_pcb(root)
                .map_err(AppError::IoError)?;

            model_pcbs.insert(pcb_path.clone(), ModelPcb {
                pcb: pcb_data,
                modified: false,
            });
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn project_pcbs(&self) -> Vec<&Pcb> {
        let iter = self.project_model_pcbs();
        Self::project_pcbs_inner(iter)
    }

    /// Return the loaded PCBs for the project.
    ///
    /// The iterator here should be an iterator that returns `Some(None)` for any PCB that hasn't been loaded.
    /// See [`project_pcbs`], [`project_model_pcbs`]
    fn project_pcbs_inner<'a>(iter: impl Iterator<Item = Option<&'a ModelPcb>>) -> Vec<&'a Pcb> {
        iter.filter_map(|model_pcb| {
            model_pcb
                .as_ref()
                .map(|model_pcb| &model_pcb.pcb)
        })
        .collect::<Vec<_>>()
    }

    /// Load a PCB, no project required.
    fn load_pcb(&mut self, path: &PathBuf) -> Result<(), AppError> {
        let pcb = pcb::load_pcb(path).map_err(AppError::IoError)?;

        self.model_pcbs
            .insert(path.clone(), ModelPcb {
                pcb,
                modified: false,
            });

        Ok(())
    }

    /// Save a PCB, no project required.
    fn save_pcb(&mut self, path: &PathBuf) -> Result<(), AppError> {
        info!("Saving PCB. path: {:?}", path);
        let model_pcb = self
            .model_pcbs
            .get_mut(path)
            .ok_or(AppError::OperationError(anyhow!("PCB not loaded. path: {:?}", path)))?;

        file::save::<Pcb>(&model_pcb.pcb, path).map_err(AppError::IoError)?;

        model_pcb.modified = false;

        Ok(())
    }
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    ProjectView(ProjectViewRendererOperation),
    PcbView(PcbViewRendererOperation),
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbGerberItem {
    pub path: PathBuf,
    pub function: Option<GerberFileFunction>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct ProjectPcbOverview {
    pub index: u16,
    pub pcb_file: FileReference,
    pub pcb_path: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbOverview {
    pub path: PathBuf,

    pub name: String,
    pub units: u16,
    /// A list of unique designs, a panel can have multiple designs.
    pub designs: Vec<DesignName>,

    /// A map of design to units, some units may be un-assigned
    /// The name of the design can be obtained by looking indexing into `designs` with the `DesignIndex`
    pub unit_map: HashMap<PcbUnitIndex, DesignIndex>,

    /// The pcb itself can have gerbers
    pub pcb_gerbers: Vec<PcbGerberItem>,

    /// In EDA tools like DipTrace, an offset can be specified when exporting gerbers, e.g. (10,5).
    /// Use negative offsets here to relocate the gerber back to (0,0), e.g. (-10, -5)
    #[serde(default)]
    pub gerber_offset: Vector2<f64>,

    /// The outer vector index is the design index, and each design can have multiple gerbers (nested vector),
    pub design_gerbers: Vec<Vec<PcbGerberItem>>,

    pub orientation: PcbAssemblyOrientation,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PcbUnitAssignments {
    /// the design name for the pcb unit index can be obtained via the PCB overview
    /// not all pcb units may be assigned
    pub unit_assignments: HashMap<PcbUnitIndex, DesignVariant>,
    pub index: u16,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct LoadOut {
    pub phase_reference: PhaseReference,
    pub source: LoadOutSource,
    pub items: Vec<LoadOutItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct Phases {
    /// in the order defined by the project's phase orderings
    pub phases: Vec<PhaseOverview>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhaseOverview {
    pub phase_reference: PhaseReference,
    pub process: ProcessReference,
    pub load_out_source: LoadOutSource,
    pub pcb_side: PcbSide,
    pub phase_placement_orderings: Vec<PlacementSortingItem>,
    pub can_start: bool,
    pub state: PhaseState,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsItem {
    pub path: ObjectPath,
    pub state: PlacementState,
    pub ordering: usize,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PhasePlacements {
    pub phase_reference: PhaseReference,

    pub placements: Vec<PlacementsItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartWithState {
    pub part: Part,
    pub processes: Vec<ProcessReference>,
    pub ref_des_set: BTreeSet<RefDes>,
    pub quantity: usize,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PartStates {
    pub parts: Vec<PartWithState>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub struct PlacementsList {
    pub placements: Vec<PlacementsItem>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub struct ProjectTreeItem {
    pub key: String,
    pub args: HashMap<String, args::Arg>,

    /// "/" = root, paths are "/" separated.
    // FIXME path elements that contain a `/` need to be escaped and un-escaped.  e.g. a phase reference of `top/1`
    pub path: String,
}

impl Default for ProjectTreeItem {
    fn default() -> Self {
        Self {
            key: "unknown".to_string(),
            args: HashMap::new(),
            path: "/".to_string(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone, Eq)]
pub struct ProjectOverview {
    pub name: String,
    pub processes: Vec<ProcessReference>,
    pub library_config: LibraryConfig,

    pub pcbs: Vec<ProjectPcb>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct ProjectTreeView {
    /// A directed graph of ProjectTreeItem.
    ///
    /// The only relationships in the tree are parent->child, i.e. not parent->grandchild
    /// the first element is the only root element
    pub tree: Graph<ProjectTreeItem, ()>,
}

impl ProjectTreeView {
    fn new() -> Self {
        Self {
            tree: Graph::new(),
        }
    }
}

impl PartialEq for ProjectTreeView {
    fn eq(&self, other: &ProjectTreeView) -> bool {
        /// Acknowledgement: https://github.com/petgraph/petgraph/issues/199#issuecomment-484077775
        fn graph_eq<N, E, Ty, Ix>(a: &petgraph::Graph<N, E, Ty, Ix>, b: &petgraph::Graph<N, E, Ty, Ix>) -> bool
        where
            N: PartialEq,
            E: PartialEq,
            Ty: petgraph::EdgeType,
            Ix: petgraph::graph::IndexType + PartialEq,
        {
            let a_ns = a.raw_nodes().iter().map(|n| &n.weight);
            let b_ns = b.raw_nodes().iter().map(|n| &n.weight);
            let a_es = a
                .raw_edges()
                .iter()
                .map(|e| (e.source(), e.target(), &e.weight));
            let b_es = b
                .raw_edges()
                .iter()
                .map(|e| (e.source(), e.target(), &e.weight));
            a_ns.eq(b_ns) && a_es.eq(b_es)
        }

        graph_eq(&self.tree, &other.tree)
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub enum ProjectView {
    Overview(ProjectOverview),
    Parts(PartStates),
    PcbOverview(ProjectPcbOverview),
    PcbUnitAssignments(PcbUnitAssignments),
    Phases(Phases),
    PhaseLoadOut(LoadOut),
    PhaseOverview(PhaseOverview),
    PhasePlacements(PhasePlacements),
    Placements(PlacementsList),
    ProcessDefinition(ProcessDefinition),
    ProjectTree(ProjectTreeView),
    ProjectReport(ProjectReport),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum ProjectViewRequest {
    Overview,
    Parts,
    PcbOverview { pcb: u16 },
    PcbUnitAssignments { pcb: u16 },
    Phases,
    PhaseLoadOut { phase: PhaseReference },
    PhaseOverview { phase: PhaseReference },
    PhasePlacements { phase: PhaseReference },
    Placements,
    ProcessDefinition { process: ProcessReference },
    ProjectTree,
    ProjectReport,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub enum PcbView {
    PcbOverview(PcbOverview),
    PanelSizing(PanelSizing),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum PcbViewRequest {
    Overview { path: PathBuf },
    Panel { path: PathBuf },
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct PlannerOperationViewModel {
    pub project_modified: bool,
    pub pcbs_modified: bool,
    pub error: Option<(chrono::DateTime<chrono::Utc>, String)>,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,
    CreateProject {
        name: String,
        /// The name of the project file
        path: PathBuf,
        packages: Option<PackagesSource>,
        package_mappings: Option<PackageMappingsSource>,
    },
    // TODO consider if the 'shell' should be loading and saving the project, not the core?
    //      currently the core does all loading/saving and uses stores too, this might not be how
    //      crux is intended to be used.
    Save,
    Load {
        /// The name of the project file
        path: PathBuf,
    },
    AddPcb {
        pcb_file: FileReference,
    },
    RemovePcb {
        index: PcbUnitIndex,
    },
    CreateProjectPcb {
        name: String,
        units: u16,
        unit_map: BTreeMap<PcbUnitNumber, DesignName>,
    },
    RefreshPcbs,
    SaveAllPcbs,
    CreateProcessFromPreset {
        preset: ProcessReference,
    },
    ApplyProcessDefinition {
        process_reference: ProcessReference,
        process_definition: ProcessDefinition,
    },
    DeleteProcess {
        process_reference: ProcessReference,
    },
    ApplyPackageSources {
        packages_source: Option<PackagesSource>,
        package_mappings_source: Option<PackageMappingsSource>,
    },
    AssignVariantToUnit {
        unit: ObjectPath,
        /// some to make assignment, none to un-assign.
        variant: Option<VariantName>,
    },
    RefreshFromDesignVariants,
    AssignProcessToParts {
        process: ProcessReference,
        operation: AddOrRemoveAction,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    CreatePhase {
        process: ProcessReference,
        reference: PhaseReference,
        load_out: LoadOutSource,
        pcb_side: PcbSide,
    },
    DeletePhase {
        reference: PhaseReference,
    },
    SetPhaseOrdering {
        phases: Vec<PhaseReference>,
    },
    AssignPlacementsToPhase {
        phase: PhaseReference,
        operation: SetOrClearAction,

        /// to apply to object path (not refdes)
        #[serde(with = "serde_regex")]
        placements: Regex,
    },
    AddPartsToLoadout {
        phase: PhaseReference,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    AssignFeederToLoadOutItem {
        phase: PhaseReference,
        feeder_reference: Option<Reference>,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    SetPlacementOrdering {
        phase: PhaseReference,
        placement_orderings: Vec<PlacementSortingItem>,
    },
    GenerateArtifacts,
    RecordPhaseOperation {
        phase: PhaseReference,
        operation: OperationReference,
        task: TaskReference,
        action: TaskAction,
    },
    /// Record placements operation
    RecordPlacementsOperation {
        #[serde(with = "serde_regex")]
        object_path_patterns: Vec<Regex>,
        operation: PlacementOperation,
    },
    RemoveUsedPlacements {
        phase: Option<PhaseReference>,
    },
    /// Reset operations
    ResetOperations {},

    //
    // Project Views
    //
    RequestOverviewView {},
    RequestPlacementsView {},
    RequestProjectTreeView {},
    RequestPhasesView {},
    RequestPhaseOverviewView {
        phase_reference: PhaseReference,
    },
    RequestPhasePlacementsView {
        phase_reference: PhaseReference,
    },
    RequestPartStatesView,
    RequestPhaseLoadOutView {
        phase_reference: PhaseReference,
    },
    RequestProjectPcbOverviewView {
        /// index, 0-based
        pcb: u16,
    },
    RequestPcbUnitAssignmentsView {
        /// index, 0-based
        pcb: u16,
    },
    RequestProcessDefinitionView {
        process_reference: ProcessReference,
    },
    RequestProjectReportView {},

    //
    // PCB operations
    //
    CreatePcb {
        name: String,
        units: u16,
        path: PathBuf,
        unit_map: Option<BTreeMap<PcbUnitNumber, DesignName>>,
    },
    LoadPcb {
        path: PathBuf,
    },
    SavePcb {
        path: PathBuf,
    },
    ApplyPcbUnitConfiguration {
        path: PathBuf,
        units: u16,
        gerber_offset: Vector2<f64>,
        designs: Vec<DesignName>,
        unit_map: BTreeMap<PcbUnitIndex, DesignIndex>,
    },
    /// Takes a fully-configured PanelSizing
    ApplyPanelSizing {
        path: PathBuf,
        panel_sizing: PanelSizing,
    },
    /// Updates the panel sizing, based on optional arguments.
    ApplyPartialPanelSizing {
        path: PathBuf,
        edge_rails: Option<Dimensions<f64>>,
        size: Option<Vector2<f64>>,
        fiducials: Option<Vec<FiducialParameters>>,
        design_sizings: Option<HashMap<DesignName, DesignSizing>>,
        pcb_unit_positionings: Option<HashMap<PcbUnitNumber, PcbUnitPositioning>>,
    },
    ApplyAssemblyOrientation {
        path: PathBuf,
        assembly_orientation: PcbAssemblyOrientation,
    },
    AddGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
        // TODO use FileReferences, not paths?
        files: Vec<(PathBuf, Option<GerberFileFunction>)>,
    },
    RemoveGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
        files: Vec<PathBuf>,
    },
    RefreshGerberFiles {
        path: PathBuf,
        design: Option<DesignName>,
    },
    ApplyGerberFileFunctions {
        path: PathBuf,
        file_functions: Vec<(PathBuf, Option<GerberFileFunction>)>,
    },

    //
    // PCB views
    //
    RequestPcbOverviewView {
        path: PathBuf,
    },
    RequestPcbPanelSizingView {
        path: PathBuf,
    },
}

impl Planner {
    fn update_inner(
        &self,
        event: <Planner as App>::Event,
    ) -> Box<
        dyn FnOnce(
            &mut <Planner as App>::Model,
        ) -> Result<Command<<Planner as App>::Effect, <Planner as App>::Event>, AppError>,
    > {
        match event {
            Event::None => Box::new(|_model: &mut Model| Ok(render::render())),
            Event::CreateProject {
                name,
                path,
                packages,
                package_mappings,
            } => Box::new(|model: &mut Model| {
                info!("Creating project. path: {:?}", &path);

                let project_directory = path.parent().unwrap().to_path_buf();

                let project = Project::new(name, packages, package_mappings);
                model
                    .model_project
                    .replace(ModelProject {
                        path,
                        project_directory,
                        project,
                        modified: true,
                    });

                info!("Created project successfully.");
                Ok(render::render())
            }),
            Event::Load {
                path,
            } => Box::new(move |model: &mut Model| {
                info!("Load project. path: {:?}", &path);

                let project: Project = file::load(&path).map_err(AppError::IoError)?;

                let project_directory = path.parent().unwrap().to_path_buf();

                model
                    .model_project
                    .replace(ModelProject {
                        path: path.clone(),
                        project_directory: project_directory.clone(),
                        project,
                        modified: false,
                    });

                model.load_unloaded_project_pcbs(&project_directory)?;

                Ok(render::render())
            }),
            Event::Save => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                info!("Save project. path: {:?}", &path);

                file::save(project, path).map_err(AppError::IoError)?;

                info!("Saved project. path: {:?}", path);
                *modified = false;

                Ok(render::render())
            }),
            Event::CreateProjectPcb {
                name,
                units,
                unit_map,
            } => Box::new(move |model: &mut Model| {
                // only the project directory is required so a filename can be built
                let ModelProject {
                    project_directory, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let pcb_file_name = format!("{}.pcb.json", name);
                let mut pcb_path = project_directory.to_path_buf();
                pcb_path.push(pcb_file_name.clone());

                Self::create_and_add_pcb(name, units, unit_map, model, &pcb_path)?;

                Ok(render::render())
            }),
            Event::CreatePcb {
                path: pcb_path,
                name,
                units,
                unit_map,
            } => Box::new(move |model: &mut Model| {
                Self::create_and_add_pcb(name, units, unit_map.unwrap_or_default(), model, &pcb_path)?;

                Ok(render::render())
            }),
            Event::ApplyPcbUnitConfiguration {
                path: pcb_path,
                units,
                gerber_offset: placement_offset,
                designs,
                unit_map,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                info!(
                    "Applying PCB unit configuration. pcb_path: {:?}, units: {:?}, designs: {:?}, unit_map: {:?}",
                    pcb_path, units, designs, unit_map
                );

                // FUTURE consider wrapping all event args in structures that can be validated using a validation framework to have consistent error handling instead of this type of thing...
                // FUTURE and consider adding validation to everything that's deserialized too...

                // Save existing design name mapping for later reference
                let old_designs: Vec<DesignName> = pcb
                    .design_names
                    .iter()
                    .cloned()
                    .collect();

                let designs_length = designs.len();
                let design_index_max = designs_length - 1;
                // Create new design set and verify uniqueness
                let design_name_set: IndexSet<DesignName> = IndexSet::from_iter(designs.clone());
                if design_name_set.len() != designs_length {
                    return Err(AppError::PcbOperationError(PcbOperationError::InvalidDesignSet));
                }

                // Validate unit map
                for (&unit_index, &design_index) in unit_map.iter() {
                    if unit_index >= units {
                        return Err(AppError::PcbOperationError(PcbOperationError::PcbError(
                            PcbError::UnitIndexOutOfRange {
                                index: unit_index,
                                min: 0,
                                max: units - 1,
                            },
                        )));
                    }
                    if design_index > design_index_max {
                        return Err(AppError::PcbOperationError(PcbOperationError::PcbError(
                            PcbError::DesignIndexOutOfRange {
                                index: design_index,
                                min: 0,
                                max: design_index_max,
                            },
                        )));
                    }
                }

                // Before replacing the design names, create a mapping for design sizings
                let mut new_design_sizings = Vec::with_capacity(designs_length);
                new_design_sizings.resize_with(designs_length, Default::default);

                // Transfer existing design sizing information for designs that are kept
                for (new_idx, design_name) in designs.iter().enumerate() {
                    if let Some(old_idx) = old_designs
                        .iter()
                        .position(|d| d == design_name)
                    {
                        if old_idx < pcb.panel_sizing.design_sizings.len() {
                            new_design_sizings[new_idx] = pcb.panel_sizing.design_sizings[old_idx].clone();
                        }
                    }
                }

                pcb.units = units;
                pcb.gerber_offset = placement_offset;
                pcb.unit_map = unit_map;
                pcb.design_names = design_name_set;

                pcb.panel_sizing.design_sizings = new_design_sizings;
                pcb.panel_sizing
                    .ensure_unit_positionings(units);

                *modified = true;

                // Once a PCB has been modified, any project using it needs to re-load it and handle inconsistencies.
                Ok(render::render())
            }),
            Event::ApplyPanelSizing {
                path: pcb_path,
                panel_sizing,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                info!(
                    "Applying panel sizing. pcb_path: {:?}, panel_sizing: {:?}",
                    pcb_path, panel_sizing
                );

                let design_count = panel_sizing.design_sizings.len();
                let expected_design_count = pcb.design_names.len();
                if design_count != expected_design_count {
                    return Err(AppError::PcbOperationError(
                        PcbOperationError::DesignSizingCountMismatch {
                            expected: expected_design_count,
                            actual: design_count,
                        },
                    ));
                }

                let pcb_unit_count = panel_sizing.pcb_unit_positionings.len() as u16;
                let expected_pcb_unit_count = pcb.units;
                if pcb_unit_count != expected_pcb_unit_count {
                    return Err(AppError::PcbOperationError(
                        PcbOperationError::UnitSizingCountMismatch {
                            expected: expected_pcb_unit_count,
                            actual: pcb_unit_count,
                        },
                    ));
                }

                pcb.panel_sizing = panel_sizing;

                *modified = true;

                // Once a PCB has been modified, any project using it needs to re-load it and handle inconsistencies.
                Ok(render::render())
            }),
            Event::ApplyPartialPanelSizing {
                path: pcb_path,
                edge_rails,
                size,
                fiducials,
                design_sizings,
                pcb_unit_positionings,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                info!(
                    "Applying partial panel sizing. pcb_path: {:?}, edge_rails: {:?}, size: {:?}, fiducials: {:?}, design_sizings: {:?}, pcb_unit_positionings: {:?}",
                    pcb_path, edge_rails, size, fiducials, design_sizings, pcb_unit_positionings
                );

                if let Some(mut design_sizings) = design_sizings {
                    let design_count = design_sizings.len();
                    let expected_design_count = pcb.design_names.len();
                    if design_count != expected_design_count {
                        return Err(AppError::PcbOperationError(
                            PcbOperationError::DesignSizingCountMismatch {
                                expected: expected_design_count,
                                actual: design_count,
                            },
                        ));
                    }

                    let design_sizings = pcb
                        .design_names
                        .iter()
                        .map(|design_name| {
                            design_sizings
                                .remove(design_name)
                                .ok_or(AppError::PcbOperationError(PcbOperationError::MissingDesignSizing(
                                    design_name.to_string(),
                                )))
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    pcb.panel_sizing.design_sizings = design_sizings;
                    *modified = true;
                }

                if let Some(mut pcb_unit_positionings) = pcb_unit_positionings {
                    let pcb_unit_count = pcb_unit_positionings.len() as u16;
                    let expected_pcb_unit_count = pcb.units;
                    if pcb_unit_count != expected_pcb_unit_count {
                        return Err(AppError::PcbOperationError(
                            PcbOperationError::UnitSizingCountMismatch {
                                expected: expected_pcb_unit_count,
                                actual: pcb_unit_count,
                            },
                        ));
                    }

                    let pcb_unit_positionings = (1..=pcb_unit_count)
                        .map(|pcb_unit_index| {
                            pcb_unit_positionings
                                .remove(&pcb_unit_index)
                                .ok_or(AppError::PcbOperationError(
                                    PcbOperationError::MissingPcbUnitPositioning(pcb_unit_index),
                                ))
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    pcb.panel_sizing.pcb_unit_positionings = pcb_unit_positionings;
                    *modified = true;
                }

                if let Some(edge_rails) = edge_rails {
                    pcb.panel_sizing.edge_rails = edge_rails;
                    *modified = true;
                }

                if let Some(size) = size {
                    pcb.panel_sizing.size = size;
                    *modified = true;
                }

                if let Some(fiducials) = fiducials {
                    pcb.panel_sizing.fiducials = fiducials;
                    *modified = true;
                }

                // Once a PCB has been modified, any project using it needs to re-load it and handle inconsistencies.
                Ok(render::render())
            }),
            Event::ApplyAssemblyOrientation {
                path: pcb_path,
                assembly_orientation,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                info!(
                    "Applying assembly orientation. pcb_path: {:?}, assembly_orientation: {:?}",
                    pcb_path, assembly_orientation
                );

                pcb.orientation = assembly_orientation;
                *modified = true;

                Ok(render::render())
            }),
            Event::LoadPcb {
                path,
            } => Box::new(move |model: &mut Model| {
                // Note: doesn't require a project.
                info!("Load PCB. path: {:?}", &path);

                model.load_pcb(&path)?;

                Ok(render::render())
            }),
            Event::SavePcb {
                path,
            } => Box::new(move |model: &mut Model| {
                // Note: doesn't require a project.
                info!("Save PCB. path: {:?}", &path);

                model.save_pcb(&path)?;

                info!("Saved PCB. path: {:?}", path);

                Ok(render::render())
            }),
            Event::RefreshPcbs => Box::new(move |model: &mut Model| {
                info!("Refreshing PCBs");

                let paths = model
                    .model_pcbs
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();

                for path in paths.into_iter() {
                    model.load_pcb(&path)?;
                }

                Ok(render::render())
            }),
            Event::SaveAllPcbs => Box::new(|model: &mut Model| {
                for (path, model_pcb) in model.model_pcbs.iter_mut() {
                    let ModelPcb {
                        pcb,
                        modified,
                    } = model_pcb;

                    info!("Save PCB. path: {:?}", path);

                    file::save(pcb, &path).map_err(AppError::IoError)?;

                    info!("Saved PCB. path: {:?}", path);
                    *modified = false;
                }

                Ok(render::render())
            }),
            Event::AddPcb {
                pcb_file,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    path,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let project_directory = path.parent().unwrap();
                let pcb_path = pcb_file.build_path(&project_directory.to_path_buf());

                let pcb = pcb::load_pcb(&pcb_path).map_err(AppError::IoError)?;

                project::add_pcb(project, &pcb_file).map_err(AppError::PcbOperationError)?;

                model
                    .model_pcbs
                    .insert(pcb_path, ModelPcb {
                        pcb,
                        modified: false,
                    });

                *modified |= true;

                Ok(render::render())
            }),
            Event::RemovePcb {
                index,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    path,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let pcb_path = project::remove_pcb(project, index, path).map_err(AppError::PcbOperationError)?;

                model.model_pcbs.remove(&pcb_path);

                *modified |= true;

                Ok(render::render())
            }),
            Event::CreateProcessFromPreset {
                preset,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let preset_name_str = preset.to_string();
                let process =
                    ProcessPresetFactory::by_preset_name(preset_name_str.as_str()).map_err(|cause| match cause {
                        ProcessPresetFactoryError::UnknownPreset {
                            preset,
                        } => AppError::ProcessError(ProcessError::UnknownPreset {
                            preset,
                            presets: ProcessPresetFactory::available_presets(),
                        }),
                    })?;

                project
                    .ensure_process(&process)
                    .map_err(AppError::OperationError)?;
                *modified |= true;

                Ok(render::render())
            }),
            Event::ApplyProcessDefinition {
                process_reference,
                process_definition,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                enum ApplyMode {
                    OnlyModified,
                    RenamedAndModified,
                    New,
                }

                let process = project
                    .processes
                    .iter()
                    .find(|it| it.reference.eq(&process_reference));

                let mode = match &process {
                    Some(process)
                        if process
                            .reference
                            .eq(&process_definition.reference) =>
                    {
                        ApplyMode::OnlyModified
                    }
                    Some(_process) => ApplyMode::RenamedAndModified,
                    None => ApplyMode::New,
                };

                if matches!(mode, ApplyMode::RenamedAndModified) {
                    if let Some(_other_process) = project.processes.iter_mut().find(|it| {
                        it.reference
                            .eq(&process_definition.reference)
                    }) {
                        // reject a rename if the process reference is already in use
                        return Err(AppError::ProcessError(ProcessError::DuplicateProcessReference {
                            process_reference,
                        }));
                    }
                }

                project
                    .ensure_process_not_in_progress(&process_reference)
                    .map_err(AppError::ProcessError)?;

                let process = project
                    .processes
                    .iter_mut()
                    .find(|it| it.reference.eq(&process_reference))
                    .unwrap();

                let new_process_reference = process_definition.reference.clone();

                match mode {
                    ApplyMode::RenamedAndModified | ApplyMode::OnlyModified => {
                        *process = process_definition;

                        // find the phases to update
                        let phases_to_update = project
                            .phases
                            .iter()
                            .filter_map(|(phase_reference, phase)| {
                                if phase.process.eq(&process_reference) {
                                    Some((phase_reference.clone(), phase.load_out_source.clone(), phase.pcb_side))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        debug!("phases_to_update: {:?}", phases_to_update);

                        // update the phase states
                        for (phase_reference, load_out_source, pcb_side) in phases_to_update {
                            project
                                .update_phase(
                                    phase_reference,
                                    new_process_reference.clone(),
                                    load_out_source,
                                    pcb_side,
                                )
                                .map_err(AppError::PhaseError)?;
                        }
                    }
                    ApplyMode::New => {
                        project
                            .processes
                            .push(process_definition);
                    }
                }

                *modified = true;

                Ok(render::render())
            }),
            Event::DeleteProcess {
                process_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                // Sanity check the process exists
                let _process_definition = project
                    .find_process(&process_reference)
                    .map_err(AppError::ProcessError)?
                    .clone();

                project
                    .delete_process(&process_reference)
                    .map_err(AppError::ProcessError)?;

                *modified = true;

                Ok(render::render())
            }),
            Event::ApplyPackageSources {
                packages_source: packages,
                package_mappings_source: package_mappings,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let changed = match (
                    &project.library_config.package_source,
                    &project
                        .library_config
                        .package_mappings_source,
                    &packages,
                    &package_mappings,
                ) {
                    (
                        Some(current_package_source),
                        Some(current_package_mappings_source),
                        Some(packages),
                        Some(package_mappings),
                    ) if current_package_source.eq(packages)
                        && current_package_mappings_source.eq(package_mappings) =>
                    {
                        // both the same
                        false
                    }
                    _ => true,
                };

                if changed {
                    project.library_config.package_source = packages;
                    project
                        .library_config
                        .package_mappings_source = package_mappings;
                    *modified = true;
                }

                Ok(render::render())
            }),
            Event::AssignVariantToUnit {
                variant: variant_name,
                unit,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                project
                    .update_assignment(&pcbs, unit.clone(), variant_name)
                    .map_err(AppError::OperationError)?;
                *modified |= true;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::ProjectError)?;
                *modified |= refresh_result;

                Ok(render::render())
            }),
            Event::RefreshFromDesignVariants => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;
                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::ProjectError)?;
                *modified |= refresh_result;

                Ok(render::render())
            }),
            Event::AssignProcessToParts {
                process: process_name,
                operation,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        modified,
                        ..
                    },
                    ..,
                ) = { Self::model_project_and_directory(model) }?;

                let process = project
                    .find_process(&process_name)
                    .map_err(AppError::ProcessError)?
                    .clone();

                let unique_parts = Self::unique_parts(project)
                    .into_iter()
                    .collect::<Vec<_>>();

                let parts_to_modify =
                    project::find_parts_to_modify(project, unique_parts.as_slice(), manufacturer_pattern, mpn_pattern);

                *modified |= project::update_applicable_processes(project, parts_to_modify, process, operation);

                Ok(render::render())
            }),
            Event::CreatePhase {
                process: process_reference,
                reference,
                load_out,
                pcb_side,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let process = project
                    .find_process(&process_reference)
                    .map_err(AppError::ProcessError)?
                    .clone();

                *modified |= true;

                stores::load_out::ensure_load_out(&load_out).map_err(AppError::OperationError)?;

                project
                    .update_phase(reference, process.reference.clone(), load_out.to_string(), pcb_side)
                    .map_err(AppError::PhaseError)?;

                Ok(render::render())
            }),
            Event::DeletePhase {
                reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                project
                    .delete_phase(reference)
                    .map_err(AppError::PhaseError)?;

                *modified |= true;

                Ok(render::render())
            }),
            Event::SetPhaseOrdering {
                phases,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let new_phase_orderings = IndexSet::from_iter(phases);

                fn are_sets_equal_in_order<T: PartialEq>(a: &IndexSet<T>, b: &IndexSet<T>) -> bool {
                    a.iter().eq(b.iter())
                }

                if !are_sets_equal_in_order(&project.phase_orderings, &new_phase_orderings) {
                    project.phase_orderings = new_phase_orderings;
                    *modified = true;
                }

                Ok(render::render())
            }),
            Event::AssignPlacementsToPhase {
                phase: phase_reference,
                operation,
                placements: placements_pattern,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    directory,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::ProjectError)?;
                *modified |= refresh_result;

                let phase = project
                    .phases
                    .get_mut(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?
                    .clone();

                let parts = project::assign_placements_to_phase(project, &phase, operation.clone(), placements_pattern)
                    .map_err(|cause| AppError::ProjectError(ProjectError::UnableToAssignPhaseToPlacements(cause)))?;

                trace!("Required load_out parts: {:?}", parts);

                *modified |= project::refresh_phase_operation_states(project);

                let load_out_source =
                    try_build_phase_load_out_source(&directory, &phase).map_err(AppError::SourceError)?;

                match operation {
                    SetOrClearAction::Set => {
                        for part in parts.iter() {
                            let part_state = project
                                .part_states
                                .get_mut(&part)
                                .ok_or_else(|| PartStateError::NoPartStateFound {
                                    part: part.clone(),
                                })
                                .map_err(AppError::PartError)?;

                            *modified |= project::add_process_to_part(part_state, part, phase.process.clone());
                        }
                        stores::load_out::add_parts_to_load_out(&load_out_source, parts)
                            .map_err(AppError::LoadoutError)?;
                    }
                    SetOrClearAction::Clear => {
                        // FUTURE not currently sure if cleanup should happen automatically or if it should be explicit.
                    }
                }
                Ok(render::render())
            }),
            Event::AddPartsToLoadout {
                phase: phase_reference,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                let phase = project
                    .phases
                    .get_mut(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(&directory, phase).map_err(AppError::SourceError)?;

                let parts = project::find_phase_parts(project, &phase_reference, manufacturer_pattern, mpn_pattern);

                stores::load_out::add_parts_to_load_out(&load_out_source, parts).map_err(AppError::LoadoutError)?;

                Ok(render::render())
            }),
            Event::RemoveUsedPlacements {
                phase: phase_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                *modified |= project.remove_unused_placements(phase_reference);

                Ok(render::render())
            }),
            Event::AssignFeederToLoadOutItem {
                phase: phase_reference,
                feeder_reference,
                manufacturer,
                mpn,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let process = project
                    .find_process(&phase.process)
                    .map_err(AppError::ProcessError)?
                    .clone();

                let load_out_source =
                    try_build_phase_load_out_source(&directory, phase).map_err(AppError::SourceError)?;

                stores::load_out::assign_feeder_to_load_out_item(
                    &load_out_source,
                    &process,
                    feeder_reference,
                    manufacturer,
                    mpn,
                )
                .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::SetPlacementOrdering {
                phase: reference,
                placement_orderings,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        path,
                        modified,
                        ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let refresh_result = Self::refresh_project(project, &pcbs, path).map_err(AppError::ProjectError)?;
                *modified |= refresh_result;

                *modified |= project::update_placement_orderings(project, &reference, &placement_orderings)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::GenerateArtifacts => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project,
                        modified,
                        ..
                    },
                    pcbs,
                    project_directory,
                ) = { Self::model_project_and_pcbs(model) }?;

                *modified |= project::refresh_phase_operation_states(project);

                let phase_load_out_item_map = Self::build_phase_load_out_item_map(project, &project_directory)
                    .map_err(AppError::OperationError)?;

                let mut packages = Vec::new();
                let mut package_mappings = Vec::new();
                let part_packages_map = Self::load_part_packages_map(project, &mut packages, &mut package_mappings)?;

                project::generate_artifacts(
                    project,
                    &pcbs,
                    &project_directory,
                    phase_load_out_item_map,
                    &part_packages_map,
                )
                .map_err(|cause| AppError::OperationError(cause.into()))?;
                Ok(render::render())
            }),
            Event::RecordPhaseOperation {
                phase: reference,
                operation,
                task,
                action,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let directory = path.parent().unwrap();
                *modified |=
                    project::apply_phase_operation_task_action(project, directory, &reference, operation, task, action)
                        .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::RecordPlacementsOperation {
                object_path_patterns,
                operation,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    path,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                let directory = path.parent().unwrap();
                *modified |= project::update_placements_operation(project, directory, object_path_patterns, operation)
                    .map_err(AppError::OperationError)?;
                Ok(render::render())
            }),
            Event::ResetOperations {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;
                project::reset_operations(project).map_err(AppError::OperationError)?;

                *modified |= true;
                Ok(render::render())
            }),

            //
            // Gerber file management
            //
            Event::AddGerberFiles {
                path: pcb_path,
                design,
                files,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                debug!(
                    "Adding gerbers to pcb. pcb_file: {:?}, design: {:?} files: {:?}",
                    pcb_path, design, files
                );

                *modified |= pcb
                    .update_gerbers(design, files)
                    .map_err(|e| AppError::PcbOperationError(PcbOperationError::PcbError(e)))?;

                Ok(render::render())
            }),
            Event::RemoveGerberFiles {
                path: pcb_path,
                design,
                files,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                debug!(
                    "Removing gerbers from pcb. pcb_file: {:?}, design: {:?} files: {:?}",
                    pcb_path, design, files
                );
                let (was_modified, unremoved_files) = pcb
                    .remove_gerbers(design, files)
                    .map_err(|e| AppError::PcbOperationError(PcbOperationError::PcbError(e)))?;

                if !unremoved_files.is_empty() {
                    warn!("Unable to remove the following gerbers. files: {:?}", unremoved_files);
                }
                *modified |= was_modified;

                Ok(render::render())
            }),
            Event::RefreshGerberFiles {
                path: pcb_path,
                design,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                debug!(
                    "Refreshing gerbers from pcb. pcb_file: {:?}, design: {:?}",
                    pcb_path, design
                );
                let was_modified = pcb
                    .update_gerbers(design, vec![])
                    .map_err(|e| AppError::PcbOperationError(PcbOperationError::PcbError(e)))?;

                *modified |= was_modified;

                Ok(render::render())
            }),
            Event::ApplyGerberFileFunctions {
                path: pcb_path,
                file_functions,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    modified,
                    pcb,
                    ..
                } = model
                    .model_pcbs
                    .get_mut(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                let was_modified = pcb.apply_file_functions(file_functions);

                *modified |= was_modified;

                Ok(render::render())
            }),

            //
            // Views
            //
            Event::RequestOverviewView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let overview = ProjectOverview {
                    name: project.name.clone(),
                    processes: project
                        .processes
                        .iter()
                        .map(|process| process.reference.clone())
                        .collect(),
                    library_config: project.library_config.clone(),
                    pcbs: project.pcbs.to_vec(),
                };
                Ok(project_view_renderer::view(ProjectView::Overview(overview)))
            }),
            Event::RequestProjectPcbOverviewView {
                pcb: pcb_index,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    project_directory,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                // we need to make sure the index is valid before attempting to get the corresponding PCB from the model.
                let project_pcb = project
                    .pcbs
                    .get(pcb_index as usize)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::Unknown))?;

                let pcb_path = project_pcb
                    .pcb_file
                    .build_path(project_directory);
                let pcb_overview = ProjectPcbOverview {
                    index: pcb_index,
                    pcb_file: project_pcb.pcb_file.clone(),
                    pcb_path,
                };

                Ok(project_view_renderer::view(ProjectView::PcbOverview(pcb_overview)))
            }),
            Event::RequestPcbPanelSizingView {
                path: pcb_path,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    pcb, ..
                } = &model
                    .model_pcbs
                    .get(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                let panel_sizing = pcb.panel_sizing.clone();

                let view = pcb_view_renderer::view(PcbView::PanelSizing(panel_sizing));
                Ok(view)
            }),
            Event::RequestPcbOverviewView {
                path: pcb_path,
            } => Box::new(move |model: &mut Model| {
                let ModelPcb {
                    pcb, ..
                } = &model
                    .model_pcbs
                    .get(&pcb_path)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::PcbNotLoaded))?;

                let designs = pcb
                    .unique_designs_iter()
                    .cloned()
                    .collect::<Vec<_>>();

                let design_gerbers = designs
                    .iter()
                    .enumerate()
                    .map(|(index, _design_name)| {
                        let design_index = DesignIndex::from(index);

                        let design_gerbers = pcb
                            .design_gerbers
                            .get(&design_index)
                            .map_or(Vec::new(), |v| {
                                v.iter()
                                    .map(Self::gerber_file_to_pcb_gerber_item)
                                    .collect::<Vec<_>>()
                            });

                        design_gerbers
                    })
                    .collect::<Vec<_>>();

                let pcb_gerbers = pcb
                    .pcb_gerbers
                    .iter()
                    .map(Self::gerber_file_to_pcb_gerber_item)
                    .collect::<Vec<_>>();

                let unit_map = pcb
                    .unit_map
                    .iter()
                    .map(|(a, b)| (*a, *b))
                    .collect::<HashMap<PcbUnitIndex, DesignIndex>>();

                let pcb_overview = PcbOverview {
                    path: pcb_path.clone(),
                    name: pcb.name.clone(),
                    units: pcb.units,
                    designs,
                    unit_map,
                    pcb_gerbers,
                    gerber_offset: pcb.gerber_offset,
                    design_gerbers,
                    orientation: pcb.orientation.clone(),
                };

                Ok(pcb_view_renderer::view(PcbView::PcbOverview(pcb_overview)))
            }),
            Event::RequestPcbUnitAssignmentsView {
                pcb: pcb_index,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let project_pcb = project
                    .pcbs
                    .get(pcb_index as usize)
                    .ok_or(AppError::PcbOperationError(PcbOperationError::Unknown))?;

                let unit_assignments = project_pcb
                    .unit_assignments
                    .iter()
                    .map(|(pcb_unit_index, design_variant)| (*pcb_unit_index, design_variant.clone()))
                    .collect::<HashMap<_, _>>();

                let pcb_unit_assignments = PcbUnitAssignments {
                    index: pcb_index,
                    unit_assignments,
                };
                Ok(project_view_renderer::view(ProjectView::PcbUnitAssignments(
                    pcb_unit_assignments,
                )))
            }),
            Event::RequestPlacementsView {} => Box::new(|model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let placements = project
                    .placements
                    .iter()
                    .enumerate()
                    .map(|(ordering, (path, state))| PlacementsItem {
                        path: path.clone(),
                        state: state.clone(),
                        ordering,
                    })
                    .collect();

                let placements = PlacementsList {
                    placements,
                };

                Ok(project_view_renderer::view(ProjectView::Placements(placements)))
            }),
            Event::RequestProjectTreeView {} => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    pcbs,
                    ..,
                ) = { Self::model_project_and_pcbs(model) }?;

                let add_test_nodes = false;

                let mut project_tree = ProjectTreeView::new();

                let root_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "root".to_string(),
                        path: "/".to_string(),
                        ..ProjectTreeItem::default()
                    });

                let issues_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "issues".to_string(),
                        path: "/issues".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, issues_node, ());

                let parts_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "parts".to_string(),
                        path: "/parts".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, parts_node, ());

                let placements_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "placements".to_string(),
                        path: "/placements".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, placements_node, ());

                let pcbs_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "pcbs".to_string(),
                        path: "/pcbs".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, pcbs_node, ());

                for (pcb_index, (project_pcb, pcb)) in project
                    .pcbs
                    .iter()
                    .zip(pcbs)
                    .enumerate()
                {
                    let pcb_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "pcb".to_string(),
                            args: HashMap::from([("name".to_string(), Arg::String(pcb.name.clone()))]),
                            path: format!("/pcbs/{}", pcb_index).to_string(),
                        });
                    project_tree
                        .tree
                        .add_edge(pcbs_node, pcb_node, ());

                    let unit_assignments_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "unit-assignments".to_string(),
                            path: format!("/pcbs/{}/units", pcb_index).to_string(),
                            ..ProjectTreeItem::default()
                        });
                    project_tree
                        .tree
                        .add_edge(pcb_node, unit_assignments_node, ());

                    for (map_index, (pcb_unit_index, design_index)) in pcb.unit_map.iter().enumerate() {
                        let mut object_path = ObjectPath::default();
                        object_path.set_pcb_instance((pcb_index + 1) as u16);
                        object_path.set_pcb_unit(pcb_unit_index + 1);

                        let mut args = HashMap::from([("name".to_string(), Arg::String(object_path.to_string()))]);

                        let design_name = pcb
                            .design_names
                            .iter()
                            .nth(*design_index as usize)
                            .unwrap();
                        args.insert("design_name".to_string(), Arg::String(design_name.to_string()));

                        if let Some(assignment_design_variant) = project_pcb
                            .unit_assignments
                            .get(pcb_unit_index)
                        {
                            let design_changed = assignment_design_variant
                                .design_name
                                .ne(design_name);
                            if design_changed {
                                // It's invalid for these to be mismatched; if this occurs, then the pcb variant map is
                                // out of sync with the pcb unit assignments and needs to be re-synced; since that
                                // should happen before this code, ignore this `unit_map` entry.
                                error!("PCB unit map is out of sync with pcb unit assignments. map_index: {}, unit: {}, assignment_design_variant: {}, design_index: {}",
                                    map_index, pcb_unit_index, assignment_design_variant, design_index
                                );
                                continue;
                            } else {
                                args.insert(
                                    "variant_name".to_string(),
                                    Arg::String(
                                        assignment_design_variant
                                            .variant_name
                                            .to_string(),
                                    ),
                                );
                            }
                        }

                        let unit_assignment_node = project_tree
                            .tree
                            .add_node(ProjectTreeItem {
                                key: "unit-assignment".to_string(),
                                args,
                                path: format!("/pcbs/{}/units/{}", pcb_index, pcb_unit_index).to_string(),
                            });

                        project_tree
                            .tree
                            .add_edge(unit_assignments_node, unit_assignment_node, ());
                    }
                }

                let processes_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "processes".to_string(),
                        path: "/processes".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, processes_node, ());

                for process in project.processes.iter() {
                    let process_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "process".to_string(),
                            args: HashMap::from([("name".to_string(), Arg::String(process.reference.to_string()))]),
                            path: format!("/processes/{}", process.reference).to_string(),
                        });

                    project_tree
                        .tree
                        .add_edge(processes_node, process_node, ());
                }

                let phases_node = project_tree
                    .tree
                    .add_node(ProjectTreeItem {
                        key: "phases".to_string(),
                        path: "/phases".to_string(),
                        ..ProjectTreeItem::default()
                    });
                project_tree
                    .tree
                    .add_edge(root_node, phases_node, ());

                for reference in &project.phase_orderings {
                    let phase = project.phases.get(reference).unwrap();
                    //
                    // add phase node
                    //
                    let phase_path = format!("/phases/{}", reference).to_string();
                    let phase_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "phase".to_string(),
                            args: HashMap::from([
                                ("reference".to_string(), Arg::String(reference.to_string())),
                                ("process".to_string(), Arg::String(phase.process.to_string())),
                                ("pcb_side".to_string(), Arg::String(phase.pcb_side.to_string())),
                            ]),
                            path: phase_path.clone(),
                        });
                    project_tree
                        .tree
                        .add_edge(phases_node, phase_node, ());

                    //
                    // add loadout node to the phase
                    //
                    let loadout_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "phase-loadout".to_string(),
                            args: HashMap::from([(
                                "source".to_string(),
                                Arg::String(phase.load_out_source.to_string()),
                            )]),
                            path: format!("{}/loadout", phase_path),
                        });
                    project_tree
                        .tree
                        .add_edge(phase_node, loadout_node, ());

                    if add_test_nodes {
                        let test_node = project_tree
                            .tree
                            .add_node(ProjectTreeItem {
                                key: "test".to_string(),
                                path: format!("/phases/{}/test", reference).to_string(),
                                ..ProjectTreeItem::default()
                            });
                        project_tree
                            .tree
                            .add_edge(phase_node, test_node, ());
                    }
                }

                if add_test_nodes {
                    let test_node = project_tree
                        .tree
                        .add_node(ProjectTreeItem {
                            key: "test".to_string(),
                            path: "/test".to_string(),
                            ..ProjectTreeItem::default()
                        });
                    project_tree
                        .tree
                        .add_edge(root_node, test_node, ());
                }

                Ok(project_view_renderer::view(ProjectView::ProjectTree(project_tree)))
            }),
            Event::RequestPhasesView {} => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                let phases = project
                    .phase_orderings
                    .iter()
                    .map(|phase_reference| {
                        let phase = project
                            .phases
                            .get(phase_reference)
                            .unwrap();
                        let phase_state = project
                            .phase_states
                            .get(phase_reference)
                            .unwrap();
                        let can_start = project.can_start_phase(&phase_reference);

                        // FUTURE try and avoid the [`unwrap`] here, ideally by ensuring load-out sources are always correct
                        //        for every situation instead of using [`try_build_phase_load_out_source`]
                        try_build_phase_overview(&directory, phase_reference.clone(), phase, can_start, phase_state)
                            .unwrap()
                    })
                    .collect::<Vec<PhaseOverview>>();

                let phases = Phases {
                    phases,
                };

                Ok(project_view_renderer::view(ProjectView::Phases(phases)))
            }),

            Event::RequestPhaseOverviewView {
                phase_reference,
            } => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;
                let phase_state = project
                    .phase_states
                    .get(&phase_reference)
                    .unwrap();
                let can_start = project.can_start_phase(&phase_reference);

                let phase_overview =
                    try_build_phase_overview(&directory, phase_reference, phase, can_start, phase_state)
                        .map_err(AppError::SourceError)?;

                Ok(project_view_renderer::view(ProjectView::PhaseOverview(phase_overview)))
            }),
            Event::RequestProcessDefinitionView {
                process_reference,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let process = project
                    .processes
                    .iter()
                    .find(|it| it.reference.eq(&process_reference))
                    .ok_or(AppError::UnknownProcessReference(process_reference.clone()))?;

                Ok(project_view_renderer::view(ProjectView::ProcessDefinition(
                    process.clone(),
                )))
            }),
            Event::RequestPhasePlacementsView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    pcbs,
                    project_directory,
                ) = Self::model_project_and_pcbs(model)?;
                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(&project_directory, &phase).map_err(AppError::SourceError)?;

                let loadout_items = stores::load_out::load_items(&load_out_source).map_err(AppError::OperationError)?;

                let mut placements: Vec<(&ObjectPath, &PlacementState)> = project
                    .placements
                    .iter()
                    .filter(|(_path, state)| match &state.phase {
                        Some(candidate_phase) if phase_reference == *candidate_phase => true,
                        _ => false,
                    })
                    .collect();

                let pcb_unit_positioning_map = project::build_pcbs_unit_positioning_map(&pcbs);

                let mut packages = Vec::new();
                let mut package_mappings = Vec::new();
                let part_packages_map = Self::load_part_packages_map(project, &mut packages, &mut package_mappings)?;

                project::sort_placements(
                    &mut placements,
                    &phase.placement_orderings,
                    &loadout_items,
                    &part_packages_map,
                    &pcb_unit_positioning_map,
                );

                let placements = placements
                    .into_iter()
                    .enumerate()
                    .map(|(ordering, (path, state))| PlacementsItem {
                        path: path.clone(),
                        state: state.clone(),
                        ordering,
                    })
                    .collect();

                let phase_placements = PhasePlacements {
                    phase_reference,
                    placements,
                };
                Ok(project_view_renderer::view(ProjectView::PhasePlacements(
                    phase_placements,
                )))
            }),
            Event::RequestPartStatesView {} => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project, ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                let mut parts = project
                    .part_states
                    .iter()
                    .map(|(part, state)| {
                        let processes = state
                            .applicable_processes
                            .iter()
                            .cloned()
                            .collect();
                        PartWithState {
                            part: part.clone(),
                            processes,
                            ref_des_set: Default::default(),
                            quantity: 0,
                        }
                    })
                    .collect::<Vec<_>>();

                //
                // add the set of ref_des and count the quantity for each part.
                //
                for (_object_path, placement_state) in project.placements.iter_mut() {
                    if let Some(part) = parts
                        .iter_mut()
                        .find(|part_with_state| {
                            part_with_state
                                .part
                                .eq(&placement_state.placement.part)
                        })
                    {
                        part.quantity += 1;
                        let _inserted = part.ref_des_set.insert(
                            placement_state
                                .placement
                                .ref_des
                                .clone(),
                        );
                    }
                }

                let part_states_view = PartStates {
                    parts,
                };

                Ok(project_view_renderer::view(ProjectView::Parts(part_states_view)))
            }),
            Event::RequestPhaseLoadOutView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?;

                let load_out_source =
                    try_build_phase_load_out_source(&directory, &phase).map_err(AppError::SourceError)?;

                let items = stores::load_out::load_items(&load_out_source).map_err(AppError::OperationError)?;

                let load_out_view = LoadOut {
                    phase_reference,
                    source: load_out_source,
                    items,
                };

                Ok(project_view_renderer::view(ProjectView::PhaseLoadOut(load_out_view)))
            }),
            Event::RequestProjectReportView {} => Box::new(|model: &mut Model| {
                let (
                    ModelProject {
                        project, ..
                    },
                    pcbs,
                    project_directory,
                ) = { Self::model_project_and_pcbs(model) }?;

                let phase_load_out_item_map = Self::build_phase_load_out_item_map(project, &project_directory)
                    .map_err(AppError::OperationError)?;

                let report = report::project_generate_report(project, &pcbs, &phase_load_out_item_map);

                Ok(project_view_renderer::view(ProjectView::ProjectReport(report)))
            }),
        }
    }

    fn build_phase_load_out_item_map(
        project: &mut Project,
        project_directory: &PathBuf,
    ) -> Result<BTreeMap<Reference, Vec<LoadOutItem>>, Error> {
        project.phases.iter().try_fold(
            BTreeMap::<Reference, Vec<LoadOutItem>>::new(),
            |mut map, (reference, phase)| {
                let load_out_items = stores::load_out::load_items(&LoadOutSource::try_from_path(
                    &project_directory,
                    PathBuf::from_str(&phase.load_out_source)?,
                )?)?;
                map.insert(reference.clone(), load_out_items);
                Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
            },
        )
    }

    fn load_packages(packages_source: &PackagesSource) -> Result<Vec<Package>, AppError> {
        let packages: Vec<Package> = stores::packages::load_packages(packages_source)
            .map_err(|error| AppError::OperationError(anyhow!("package source error. cause: {:?}", error)))?;

        Ok(packages)
    }

    fn load_package_mappings<'a>(
        package_mappings_source: &PackageMappingsSource,
        packages: &'a Vec<Package>,
    ) -> Result<Vec<PackageMapping<'a>>, AppError> {
        let package_mappings = stores::package_mappings::load_package_mappings(packages, package_mappings_source)
            .map_err(|error| AppError::OperationError(anyhow!("package source error. cause: {:?}", error)))?;

        Ok(package_mappings)
    }

    fn project_unique_parts(project: &Project) -> BTreeSet<&Part> {
        let unique_parts = BTreeSet::from_iter(
            project
                .placements
                .iter()
                .map(|(_path, placement_state)| &placement_state.placement.part),
        );

        unique_parts
    }

    fn build_project_part_package_map<'a, 'b, 'c, 'd>(
        unique_parts: &'a BTreeSet<&'b Part>,
        package_mappings: &'c Vec<PackageMapping<'d>>,
    ) -> Result<BTreeMap<&'b Part, &'c Package>, AppError> {
        let part_packages_map = package_mapper::PackageMapper::process(unique_parts, package_mappings)
            .map_err(|mapper_error| AppError::OperationError(anyhow!("part mapper error. cause: {:?}", mapper_error)))?
            .into_iter()
            .filter_map(|result| {
                result
                    .package
                    .map(|package| (result.part, package))
            })
            .collect::<BTreeMap<_, _>>();
        Ok(part_packages_map)
    }

    // Due to the lifetimes of the mappings, the storage for the packages and package mappings needs to be provided
    // up-front.
    fn load_part_packages_map<'a, 'b>(
        project: &'a Project,
        // Provide mutable references to variables that will be populated
        packages_storage: &'b mut Vec<Package>,
        package_mappings_storage: &'b mut Vec<PackageMapping<'b>>,
    ) -> Result<BTreeMap<&'a Part, &'b Package>, AppError> {
        // Check if both sources are available
        let (Some(packages_source), Some(package_mappings_source)) = (
            project
                .library_config
                .package_source
                .as_ref(),
            project
                .library_config
                .package_mappings_source
                .as_ref(),
        ) else {
            return Ok(BTreeMap::new());
        };

        // Load the packages and store them in the provided output vector
        *packages_storage = Self::load_packages(packages_source)?;

        // Load the mappings and store them in the provided output vector
        *package_mappings_storage = Self::load_package_mappings(package_mappings_source, packages_storage)?;

        // Build the mapping
        let unique_parts = Self::project_unique_parts(project);
        let part_packages_map = Self::build_project_part_package_map(&unique_parts, package_mappings_storage)?;

        Ok(part_packages_map)
    }

    fn create_and_add_pcb(
        name: String,
        units: u16,
        unit_map: BTreeMap<PcbUnitNumber, DesignName>,
        model: &mut Model,
        pcb_path: &PathBuf,
    ) -> Result<(), AppError> {
        let pcb = planning::pcb::create_pcb(name, units, unit_map).map_err(AppError::PcbOperationError)?;

        model
            .model_pcbs
            .insert(pcb_path.clone(), ModelPcb {
                pcb: pcb.clone(),
                // not saved, yet
                modified: true,
            });

        model.save_pcb(pcb_path)?;
        Ok(())
    }

    fn gerber_file_to_pcb_gerber_item(gerber_file: &GerberFile) -> PcbGerberItem {
        // convert from project type to view type
        PcbGerberItem {
            path: gerber_file.file.clone(),
            function: gerber_file.function,
        }
    }

    fn model_project_and_pcbs(model: &mut Model) -> Result<(&mut ModelProject, Vec<&Pcb>, PathBuf), AppError> {
        let Some(model_project) = model.model_project.as_mut() else {
            return Err(AppError::OperationRequiresProject);
        };

        let project_directory = model_project
            .path
            .parent()
            .unwrap()
            .to_path_buf();

        let model_pcbs = &mut model.model_pcbs;
        Model::load_project_pcbs_inner(&mut model_project.project, model_pcbs, &project_directory)?;

        let iter = model_project.pcbs(&model.model_pcbs);
        let pcbs = Model::project_pcbs_inner(iter);

        Ok((model_project, pcbs, project_directory))
    }

    fn model_project_and_directory(model: &mut Model) -> Result<(&mut ModelProject, PathBuf), AppError> {
        let Some(model_project) = model.model_project.as_mut() else {
            return Err(AppError::OperationRequiresProject);
        };

        let project_directory = model_project
            .path
            .parent()
            .unwrap()
            .to_path_buf();
        Ok((model_project, project_directory))
    }
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = PlannerOperationViewModel;
    type Effect = Effect;

    fn update(&self, event: Self::Event, model: &mut Self::Model) -> Command<Self::Effect, Self::Event> {
        let try_fn = self.update_inner(event);

        match try_fn(model) {
            Err(e) => {
                model
                    .error
                    .replace((chrono::DateTime::from(SystemTime::now()), format!("{:?}", e)));
                render::render()
            }
            Ok(command) => {
                model.error.take();
                command
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        let project_modified = model
            .model_project
            .as_ref()
            .map_or(false, |project| project.modified);

        let pcbs_modified = model
            .model_pcbs
            .iter()
            .any(|(_file_reference, pcb)| pcb.modified);

        let view_model = PlannerOperationViewModel {
            project_modified,
            pcbs_modified,
            error: model.error.clone(),
        };

        trace!("view model: {:?}", view_model);

        view_model
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation requires a project")]
    OperationRequiresProject,
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
    #[error("Phase error. cause: {0}")]
    PhaseError(PhaseError),
    #[error("Project error, cause: {0}")]
    ProjectError(ProjectError),
    #[error("Process error. cause: {0}")]
    ProcessError(ProcessError),
    #[error("Part error. cause: {0}")]
    PartError(PartStateError),
    #[error("Source error. cause: {0}")]
    SourceError(SourceError),
    #[error("Loadout error. cause: {0}")]
    LoadoutError(LoadOutOperationError),
    #[error("PCB error. cause: {0}")]
    PcbOperationError(PcbOperationError),
    #[error("IO error. cause: {0}")]
    IoError(std::io::Error),

    #[error("Unknown phase reference. reference: {0}")]
    UnknownPhaseReference(Reference),
    #[error("Unknown process reference. reference: {0}")]
    UnknownProcessReference(ProcessReference),
}

impl Planner {
    fn refresh_project(project: &mut Project, pcbs: &[&Pcb], path: &PathBuf) -> Result<bool, ProjectError> {
        let directory = path.parent().unwrap();

        let unique_design_variants = project.unique_design_variants(pcbs);

        let design_variant_placement_map = stores::placements::load_all_placements(unique_design_variants, directory)
            .map_err(ProjectError::UnableToLoadPlacements)?;
        let refresh_result = project::refresh_from_design_variants(project, pcbs, design_variant_placement_map);

        if let Ok(modified) = &refresh_result {
            trace!("Refreshed from design variants. modified: {}", modified);
        }

        refresh_result
    }

    fn unique_parts<'a>(project: &'a Project) -> impl IntoIterator<Item = &'a Part> + 'a {
        let unique_parts = project
            .placements
            .iter()
            .map(|(_path, state)| &state.placement.part)
            .collect::<IndexSet<_>>();
        unique_parts
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;

    #[test]
    fn minimal() {
        let app = Planner;
        let mut model = Model::default();

        // Call 'update' and request effects
        app.update(Event::None, &mut model)
            .expect_only_render();

        // Make sure the view matches our expectations
        let actual_view = app.view(&model);
        let expected_view = PlannerOperationViewModel::default();
        assert_eq!(actual_view, expected_view);
    }
}

/// Build a load-out source, where the load-out source *may* be a relative or absolute path.
fn try_build_phase_load_out_source(project_path: &PathBuf, phase: &Phase) -> Result<LoadOutSource, SourceError> {
    assert!(project_path.is_dir());

    let directory = project_path
        .parent()
        .unwrap()
        .to_path_buf();

    LoadOutSource::try_from_path(&directory, PathBuf::from(&phase.load_out_source))
}

fn try_build_phase_overview(
    directory: &PathBuf,
    phase_reference: PhaseReference,
    phase: &Phase,
    can_start: bool,
    state: &PhaseState,
) -> Result<PhaseOverview, SourceError> {
    let load_out_source = try_build_phase_load_out_source(directory, phase)?;

    Ok(PhaseOverview {
        phase_reference,
        process: phase.process.clone(),
        load_out_source,
        pcb_side: phase.pcb_side.clone(),
        phase_placement_orderings: phase.placement_orderings.clone(),
        can_start,
        state: state.clone(),
    })
}
