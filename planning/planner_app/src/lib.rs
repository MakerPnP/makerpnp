// A core does no IO, see `clippy.toml`.
#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::anyhow;
pub use args::Arg;
use crux_core::command::CommandContext;
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
pub use planning::process::ProcessReference;
pub use planning::process::TaskReference;
pub use planning::process::TaskStatus;
pub use planning::process::{
    OperationDefinition, OperationReference, OperationStatus, ProcessDefinition, ProcessDefinitionError, ProcessEdit,
    ProcessPresetFactory, ProcessRuleReference, TaskAction, TaskDefinition, TaskKind, PROCESS_RULES,
};
use planning::process::{ProcessError, ProcessPresetFactoryError};
pub use planning::project::ProjectProcess;
use planning::project::{PartStateError, PcbOperationError, Project, ProjectError, ProjectPcb};
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
pub use stores::load_out::LoadOutSource;
pub use stores::package_mappings::PackageMappingsSource;
pub use stores::packages::PackagesSource;
pub use stores_protocol::{Document, Records, RecordsKind, RecordsOf, Revision, StoresOperation, StoresOutput};
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};
use util::source::{Source, SourceError};

use crate::effects::pcb_view_renderer::PcbViewRendererOperation;
use crate::effects::process_view_renderer::ProcessViewRendererOperation;
use crate::effects::project_view_renderer::ProjectViewRendererOperation;
use crate::effects::{pcb_view_renderer, process_view_renderer, project_view_renderer};
use crate::stored::StoredRecords;

pub mod effects;
pub mod stored;

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;

#[derive(Default, Clone)]
pub struct ModelProject {
    path: PathBuf,
    project_directory: PathBuf,
    project: Project,
    modified: bool,
    /// The contents being written, until they are, so the project is only said to be saved if it's still as written.
    writing: Option<String>,
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

#[derive(Clone)]
pub struct ModelPcb {
    pcb: Pcb,
    modified: bool,
    /// The contents being written, until they are, so the PCB is only said to be saved if it's still as written.
    writing: Option<String>,
}

type ModelPcbs = BTreeMap<PathBuf, ModelPcb>;

/// A process document, a process's definition in its own file.
#[derive(Clone)]
pub struct ModelProcess {
    definition: ProcessDefinition,
    modified: bool,
    /// The contents being written, until they are, so the process is only said to be saved if it's still as written.
    writing: Option<String>,
}

type ModelProcesses = BTreeMap<PathBuf, ModelProcess>;

#[derive(Default, Clone)]
pub struct Model {
    model_project: Option<ModelProject>,
    /// A project being created, until its file is written (`DOC-1`), so a project whose file can't be written isn't
    /// created, and the project before it is kept.
    creating_project: Option<ModelProject>,
    /// PCBs that have been created/loaded.
    ///
    /// Important: Can contain instances of [`ModelPcb`] that have been created or loaded, but not assigned to a project yet.
    model_pcbs: ModelPcbs,
    /// PCBs being created, until their files are written (`DOC-1`).
    creating_pcbs: ModelPcbs,

    /// Process documents that have been created/loaded.
    ///
    /// A project doesn't use these, it keeps its own copy of each process it uses, see [`ProjectProcess`].
    model_processes: ModelProcesses,
    /// Process documents being created, until their files are written (`DOC-1`).
    creating_processes: ModelProcesses,

    /// The records read for the event being processed (`STORE-11`), see [`stored`].
    stored: Arc<StoredRecords>,
    /// The records the event being processed has changed, written once it's done.
    pending_writes: Vec<(Source, RecordsOf)>,

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

    /// Requires each of the project's PCBs to be loaded, as they're read when the project is loaded, or added to it
    /// (`STORE-10`).
    fn require_project_pcbs_loaded(project: &Project, model_pcbs: &ModelPcbs, root: &PathBuf) -> Result<(), AppError> {
        match project
            .pcbs
            .iter()
            .map(|pcb| pcb.pcb_file.build_path(root))
            .find(|pcb_path| !model_pcbs.contains_key(pcb_path))
        {
            Some(pcb_path) => Err(AppError::OperationError(anyhow!(
                "PCB not loaded. path: {:?}",
                pcb_path
            ))),
            None => Ok(()),
        }
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

    /// A PCB, read for [`Event::LoadPcb`] or [`Event::RefreshPcbs`], no project required.
    fn pcb_read(&mut self, path: PathBuf, output: StoresOutput) -> Result<Command<Effect, Event>, AppError> {
        let pcb = Self::pcb_from_output(&path, output)?;

        self.model_pcbs.insert(path, ModelPcb {
            pcb,
            modified: false,
            writing: None,
        });

        Ok(render::render())
    }

    /// The PCB read, or why it couldn't be.
    fn pcb_from_output(path: &PathBuf, output: StoresOutput) -> Result<Pcb, AppError> {
        let StoresOutput::DocumentRead(result) = output else {
            return Err(AppError::UnexpectedOutput {
                output,
            });
        };
        let document = result
            .map_err(AppError::StoresError)?
            .ok_or_else(|| AppError::StoresError(format!("PCB not found. path: {:?}", path)))?;

        pcb::from_contents(&document.contents).map_err(|cause| {
            AppError::StoresError(format!("Unable to read the PCB. path: {:?}, cause: '{}'", path, cause))
        })
    }

    /// Has the PCB's file written (`STORE-10`), see [`Event::PcbWritten`].
    fn save_pcb(&mut self, path: &PathBuf) -> Result<Command<Effect, Event>, AppError> {
        info!("Saving PCB. path: {:?}", path);
        let model_pcb = self
            .model_pcbs
            .get_mut(path)
            .ok_or(AppError::OperationError(anyhow!("PCB not loaded. path: {:?}", path)))?;

        let contents = file::to_string(&model_pcb.pcb).map_err(|cause| AppError::IoError(cause.into()))?;
        model_pcb.writing = Some(contents.clone());

        Ok(Planner::write_pcb(path.clone(), contents))
    }

    /// The PCB's file was written, or wasn't, for [`Event::CreatePcb`], [`Event::CreateProjectPcb`],
    /// [`Event::SavePcb`] or [`Event::SaveAllPcbs`].
    fn pcb_written(&mut self, path: PathBuf, output: StoresOutput) -> Result<Command<Effect, Event>, AppError> {
        let StoresOutput::DocumentWritten(result) = output else {
            return Err(AppError::UnexpectedOutput {
                output,
            });
        };

        // `DOC-1`, a PCB being created is only created once its file is written.
        if let Some(mut model_pcb) = self.creating_pcbs.remove(&path) {
            result.map_err(AppError::StoresError)?;
            info!("Saved PCB. path: {:?}", path);
            model_pcb.modified = false;
            self.model_pcbs.insert(path, model_pcb);
            return Ok(render::render());
        }

        let model_pcb = self
            .model_pcbs
            .get_mut(&path)
            .ok_or(AppError::OperationError(anyhow!("PCB not loaded. path: {:?}", path)))?;
        let written = model_pcb.writing.take();
        // a refused write leaves the PCB's changes unsaved.
        result.map_err(AppError::StoresError)?;

        info!("Saved PCB. path: {:?}", path);
        // the PCB may have been changed while it was written, in which case those changes aren't saved.
        if written.is_some() && written == file::to_string(&model_pcb.pcb).ok() {
            model_pcb.modified = false;
        }

        Ok(render::render())
    }

    /// Has the project's file written (`STORE-10`), see [`Event::ProjectWritten`].
    fn save_project(&mut self) -> Result<Command<Effect, Event>, AppError> {
        let model_project = self
            .model_project
            .as_mut()
            .ok_or(AppError::OperationRequiresProject)?;

        info!("Save project. path: {:?}", &model_project.path);

        let contents = file::to_string(&model_project.project).map_err(|cause| AppError::IoError(cause.into()))?;
        model_project.writing = Some(contents.clone());

        Ok(Planner::write_project(model_project.path.clone(), contents))
    }

    /// The project's file was written, or wasn't, for [`Event::CreateProject`] or [`Event::Save`].
    fn project_written(&mut self, path: PathBuf, output: StoresOutput) -> Result<Command<Effect, Event>, AppError> {
        let StoresOutput::DocumentWritten(result) = output else {
            return Err(AppError::UnexpectedOutput {
                output,
            });
        };

        // `DOC-1`, a project being created is only created once its file is written, otherwise the project before it,
        // if any, is kept.
        if let Some(mut model_project) = self
            .creating_project
            .take_if(|model_project| model_project.path == path)
        {
            result.map_err(AppError::StoresError)?;
            model_project.modified = false;
            self.model_project
                .replace(model_project);
            info!("Created project successfully.");
            return Ok(render::render());
        }

        let model_project = self
            .model_project
            .as_mut()
            .filter(|model_project| model_project.path == path)
            .ok_or(AppError::OperationRequiresProject)?;
        let written = model_project.writing.take();
        // a refused write leaves the project's changes unsaved.
        result.map_err(AppError::StoresError)?;

        info!("Saved project. path: {:?}", path);
        // the project may have been changed while it was written, in which case those changes aren't saved.
        if written.is_some() && written == file::to_string(&model_project.project).ok() {
            model_project.modified = false;
        }

        Ok(render::render())
    }

    /// A process document, read for [`Event::LoadProcess`], no project required.
    fn process_read(&mut self, path: PathBuf, output: StoresOutput) -> Result<Command<Effect, Event>, AppError> {
        let definition = Self::process_from_output(&path, output)?;
        definition
            .validate()
            .map_err(|cause| AppError::ProcessError(ProcessError::InvalidProcess(cause)))?;

        self.model_processes
            .insert(path, ModelProcess {
                definition,
                modified: false,
                writing: None,
            });

        Ok(render::render())
    }

    /// The process read, or why it couldn't be.
    fn process_from_output(path: &PathBuf, output: StoresOutput) -> Result<ProcessDefinition, AppError> {
        let StoresOutput::DocumentRead(result) = output else {
            return Err(AppError::UnexpectedOutput {
                output,
            });
        };
        let document = result
            .map_err(AppError::StoresError)?
            .ok_or_else(|| AppError::StoresError(format!("Process not found. path: {:?}", path)))?;

        process_from_contents(path, &document.contents).map_err(AppError::StoresError)
    }

    /// Has the process's file written (`STORE-10`), see [`Event::ProcessWritten`].
    fn save_process(&mut self, path: &PathBuf) -> Result<Command<Effect, Event>, AppError> {
        info!("Saving process. path: {}", path.display());
        let model_process = self
            .model_processes
            .get_mut(path)
            .ok_or(AppError::ProcessNotLoaded(path.clone()))?;

        let contents = file::to_string(&model_process.definition).map_err(|cause| AppError::IoError(cause.into()))?;
        model_process.writing = Some(contents.clone());

        Ok(Planner::write_process(path.clone(), contents))
    }

    /// The process's file was written, or wasn't, for [`Event::CreateProcess`], [`Event::SaveProcess`] or
    /// [`Event::SaveAllProcesses`].
    fn process_written(&mut self, path: PathBuf, output: StoresOutput) -> Result<Command<Effect, Event>, AppError> {
        let StoresOutput::DocumentWritten(result) = output else {
            return Err(AppError::UnexpectedOutput {
                output,
            });
        };

        // `DOC-1`, a process being created is only created once its file is written.
        if let Some(mut model_process) = self.creating_processes.remove(&path) {
            result.map_err(AppError::StoresError)?;
            info!("Saved process. path: {:?}", path);
            model_process.modified = false;
            self.model_processes
                .insert(path, model_process);
            return Ok(render::render());
        }

        let model_process = self
            .model_processes
            .get_mut(&path)
            .ok_or(AppError::ProcessNotLoaded(path.clone()))?;
        let written = model_process.writing.take();
        // a refused write leaves the process's changes unsaved.
        result.map_err(AppError::StoresError)?;

        info!("Saved process. path: {:?}", path);
        // the process may have been changed while it was written, in which case those changes aren't saved.
        if written.is_some() && written == file::to_string(&model_process.definition).ok() {
            model_process.modified = false;
        }

        Ok(render::render())
    }

    fn model_process_mut(&mut self, path: &PathBuf) -> Result<&mut ModelProcess, AppError> {
        self.model_processes
            .get_mut(path)
            .ok_or(AppError::ProcessNotLoaded(path.clone()))
    }
}

/// Gives each of a project's processes that has no file, as in projects from before processes had files, a file of its
/// own in the project's folder.  The project then has unsaved changes.
///
/// A file is named after its process, e.g. `pnp.process.json`.  An existing file with the same definition is used as it
/// is, otherwise a number is added to the name, e.g. `pnp_2.process.json`.
///
/// Returns whether any process was given a file.
async fn migrate_processes_to_files(
    ctx: &Context,
    project: &mut Project,
    project_directory: &PathBuf,
) -> Result<bool, String> {
    let indexes = project.processes_without_files();

    for index in indexes.iter() {
        let project_process = &mut project.processes[*index];
        let definition = &project_process.definition;

        let base_name = definition
            .reference
            .to_string()
            .chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
                _ => '_',
            })
            .collect::<String>();

        let mut number = 1;
        let file_name = loop {
            let file_name = match number {
                1 => format!("{}.process.json", base_name),
                _ => format!("{}_{}.process.json", base_name, number),
            };
            let path = project_directory.join(&file_name);

            // `STORE-10`
            match read_document(ctx, &path).await {
                Ok(None) => {
                    info!("Saving process. path: {}", path.display());
                    let contents = file::to_string(definition).map_err(|cause| format!("{:?}", cause))?;
                    write_document(ctx, &path, contents).await?;
                    break file_name;
                }
                Ok(Some(document)) => {
                    if matches!(file::from_str::<ProcessDefinition>(&document.contents), Ok(existing) if existing.eq(definition))
                    {
                        break file_name;
                    }
                }
                // a file that can't be read isn't used, as one that's different isn't.
                Err(_) => {}
            }

            number += 1;
        };

        info!(
            "Moved process to its own file. process: '{}', file: {:?}",
            definition.reference, file_name
        );
        project_process.process_file = Some(FileReference::Relative(PathBuf::from(file_name)));
    }

    Ok(!indexes.is_empty())
}

/// What a store operation is given, in a command that does more than one, see [`Command::new`].
type Context = CommandContext<Effect, Event>;

/// The document at the path, if there is one, read by the stores service (`STORE-10`).
async fn read_document(ctx: &Context, path: &PathBuf) -> Result<Option<Document>, String> {
    match ctx
        .request_from_shell(StoresOperation::ReadDocument {
            path: path.clone(),
        })
        .await
    {
        StoresOutput::DocumentRead(result) => result,
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

/// Writes the document, by the stores service (`STORE-10`).
async fn write_document(ctx: &Context, path: &PathBuf, contents: String) -> Result<Revision, String> {
    match ctx
        .request_from_shell(StoresOperation::WriteDocument {
            path: path.clone(),
            contents,
        })
        .await
    {
        StoresOutput::DocumentWritten(result) => result,
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

/// The records of the data source, read by the stores service (`STORE-11`).
async fn read_records(ctx: &Context, kind: RecordsKind, source: &Source) -> Result<Option<Records>, String> {
    match ctx
        .request_from_shell(StoresOperation::ReadRecords {
            kind,
            source: source.clone(),
        })
        .await
    {
        StoresOutput::RecordsRead(result) => result,
        output => Err(format!("Unexpected output. output: {:?}", output)),
    }
}

/// A process, from its document's contents.
fn process_from_contents(path: &PathBuf, contents: &str) -> Result<ProcessDefinition, String> {
    file::from_str::<ProcessDefinition>(contents)
        .map_err(|cause| format!("Unable to read the process. path: {:?}, cause: '{}'", path, cause))
}

/// The processes read for [`Event::RefreshProcesses`], by the reference of the process each replaces.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ProcessDefinitions(Vec<(ProcessReference, ProcessDefinition)>);

/// Says which processes were read, not what's in them, as the event is logged.
impl Debug for ProcessDefinitions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(
                self.0
                    .iter()
                    .map(|(reference, _)| reference.to_string()),
            )
            .finish()
    }
}

/// A project read for [`Event::Load`], with the PCBs it uses that weren't loaded already.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LoadedProject {
    project: Project,
    /// Whether its processes were moved to their own files, so it has unsaved changes.
    migrated: bool,
    pcbs: Vec<(PathBuf, Pcb)>,
    /// Why a PCB couldn't be read. The project is still loaded, with the PCBs read before it, as it was when the core
    /// read its files itself.
    pcb_error: Option<String>,
}

/// Says which PCBs were read, not what's in them, as the event is logged.
impl Debug for LoadedProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedProject")
            .field("migrated", &self.migrated)
            .field(
                "pcbs",
                &self
                    .pcbs
                    .iter()
                    .map(|(path, _)| path)
                    .collect::<Vec<_>>(),
            )
            .field("pcb_error", &self.pcb_error)
            .finish()
    }
}

/// Reads the project, moves its processes to their own files, and reads the PCBs it uses that aren't loaded.
async fn read_project(ctx: &Context, path: &PathBuf, loaded_pcbs: &BTreeSet<PathBuf>) -> Result<LoadedProject, String> {
    let document = read_document(ctx, path)
        .await?
        .ok_or_else(|| format!("Project not found. path: {:?}", path))?;
    let mut project: Project = file::from_str(&document.contents)
        .map_err(|cause| format!("Unable to read the project. path: {:?}, cause: '{}'", path, cause))?;

    let project_directory = path.parent().unwrap().to_path_buf();
    let migrated = migrate_processes_to_files(ctx, &mut project, &project_directory).await?;

    let mut pcbs = vec![];
    let mut pcb_error = None;
    for project_pcb in project.pcbs.iter() {
        let pcb_path = project_pcb
            .pcb_file
            .build_path(&project_directory);
        if loaded_pcbs.contains(&pcb_path) {
            continue;
        }
        info!("Loading PCB. path: {:?}", pcb_path);
        let pcb = read_document(ctx, &pcb_path)
            .await
            .and_then(|document| document.ok_or_else(|| format!("PCB not found. path: {:?}", pcb_path)))
            .and_then(|document| {
                pcb::from_contents(&document.contents)
                    .map_err(|cause| format!("Unable to read the PCB. path: {:?}, cause: '{}'", pcb_path, cause))
            });
        match pcb {
            Ok(pcb) => pcbs.push((pcb_path, pcb)),
            // stops at the first error.
            Err(error) => {
                pcb_error = Some(error);
                break;
            }
        }
    }

    Ok(LoadedProject {
        project,
        migrated,
        pcbs,
        pcb_error,
    })
}

#[effect]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    ProjectView(ProjectViewRendererOperation),
    PcbView(PcbViewRendererOperation),
    ProcessView(ProcessViewRendererOperation),
    /// A store operation, done by the stores service, as the core does none itself (`STORE-7`).
    Stores(StoresOperation),
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
    /// The project's copy of a process, and the file it came from.
    ProcessDefinition(ProjectProcess),
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

/// A view of a process document.
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone)]
pub enum ProcessView {
    Definition {
        path: PathBuf,
        definition: ProcessDefinition,
    },
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct PlannerOperationViewModel {
    pub project_modified: bool,
    pub pcbs_modified: bool,
    /// Whether any process document has unsaved changes.
    pub processes_modified: bool,
    pub error: Option<(chrono::DateTime<chrono::Utc>, String)>,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum Event {
    None,
    /// Internal: the event, given the records it needs, read by the stores service (`STORE-11`), see [`stored`].
    WithRecords {
        event: Box<Event>,
        records: StoredRecords,
    },
    /// Internal: the records an event changed were written, or weren't (`STORE-11`).
    RecordsWritten {
        source: Source,
        output: StoresOutput,
    },
    CreateProject {
        name: String,
        /// The name of the project file
        path: PathBuf,
        packages: Option<PackagesSource>,
        package_mappings: Option<PackageMappingsSource>,
    },
    Save,
    /// Internal: the project's file was written, or wasn't, for [`Event::CreateProject`] or [`Event::Save`]
    /// (`STORE-10`).
    ProjectWritten {
        path: PathBuf,
        output: StoresOutput,
    },
    Load {
        /// The name of the project file
        path: PathBuf,
    },
    /// Internal: the project, read for [`Event::Load`], or why it couldn't be (`STORE-10`).
    ProjectLoaded {
        path: PathBuf,
        loaded: Result<LoadedProject, String>,
    },
    AddPcb {
        pcb_file: FileReference,
    },
    /// Internal: the PCB's file, read for [`Event::AddPcb`], or why it couldn't be (`STORE-10`).
    ProjectPcbRead {
        pcb_file: FileReference,
        path: PathBuf,
        output: StoresOutput,
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
    /// Adds a copy of a process, from its file, to the project.
    AddProcess {
        process_file: FileReference,
    },
    /// Internal: the process's file, read for [`Event::AddProcess`], or why it couldn't be (`STORE-10`).
    ProjectProcessRead {
        process_file: FileReference,
        path: PathBuf,
        output: StoresOutput,
    },
    /// Removes a process from the project.  Its file is not deleted.
    RemoveProcess {
        process_reference: ProcessReference,
    },
    /// Updates the project's copy of each process from its file.
    ///
    /// A process that has changed can't be updated while a phase that uses it is in progress, and then no process is
    /// updated.
    RefreshProcesses,
    /// Internal: each of the project's processes, read for [`Event::RefreshProcesses`], or why one couldn't be
    /// (`STORE-10`).
    ProcessesRead {
        definitions: Result<ProcessDefinitions, String>,
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
    /// Internal: the PCB's file, read for [`Event::LoadPcb`] or [`Event::RefreshPcbs`], or why it couldn't be
    /// (`STORE-10`).
    PcbRead {
        path: PathBuf,
        output: StoresOutput,
    },
    SavePcb {
        path: PathBuf,
    },
    /// Internal: the PCB's file was written, or wasn't, for [`Event::CreatePcb`], [`Event::CreateProjectPcb`],
    /// [`Event::SavePcb`] or [`Event::SaveAllPcbs`] (`STORE-10`).
    PcbWritten {
        path: PathBuf,
        output: StoresOutput,
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

    //
    // Process operations, no project required.
    //
    /// Creates a process document, and saves it.
    CreateProcess {
        path: PathBuf,
        reference: ProcessReference,
        /// The process to start from, see [`ProcessPresetFactory`], otherwise the process has no operations.
        preset: Option<String>,
    },
    LoadProcess {
        path: PathBuf,
    },
    /// Internal: the process's file, read for [`Event::LoadProcess`], or why it couldn't be (`STORE-10`).
    ProcessRead {
        path: PathBuf,
        output: StoresOutput,
    },
    SaveProcess {
        path: PathBuf,
    },
    /// Internal: the process's file was written, or wasn't, for [`Event::CreateProcess`], [`Event::SaveProcess`] or
    /// [`Event::SaveAllProcesses`] (`STORE-10`).
    ProcessWritten {
        path: PathBuf,
        output: StoresOutput,
    },
    SaveAllProcesses,
    /// Replaces the definition in a process document.
    ApplyProcessDefinition {
        path: PathBuf,
        process_definition: ProcessDefinition,
    },
    /// Makes one change to the definition in a process document.
    EditProcess {
        path: PathBuf,
        edit: ProcessEdit,
    },

    //
    // Process views
    //
    RequestProcessView {
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
            // given its records, and processed, by `update`.
            Event::WithRecords {
                ..
            } => unreachable!(),
            Event::RecordsWritten {
                source,
                output,
            } => Box::new(move |_model: &mut Model| {
                let StoresOutput::RecordsWritten(result) = output else {
                    return Err(AppError::UnexpectedOutput {
                        output,
                    });
                };
                result.map_err(AppError::StoresError)?;
                info!("Stored records. source: '{}'", source);

                Ok(render::render())
            }),
            Event::CreateProject {
                name,
                path,
                packages,
                package_mappings,
            } => Box::new(|model: &mut Model| {
                info!("Creating project. path: {:?}", &path);

                let project_directory = path.parent().unwrap().to_path_buf();

                let project = Project::new(name, packages, package_mappings);
                let contents = file::to_string(&project).map_err(|cause| AppError::IoError(cause.into()))?;

                // `DOC-1`, as with a new PCB, a new project is saved as it is created, and is created once it's saved,
                // see `Event::ProjectWritten`.
                model
                    .creating_project
                    .replace(ModelProject {
                        path: path.clone(),
                        project_directory,
                        project,
                        modified: true,
                        writing: None,
                    });

                Ok(Self::write_project(path, contents))
            }),
            Event::Load {
                path,
            } => Box::new(move |model: &mut Model| {
                info!("Load project. path: {:?}", &path);

                // `STORE-10`, the PCBs already loaded aren't read again.
                let loaded_pcbs = model
                    .model_pcbs
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>();

                Ok(Command::new(|ctx| async move {
                    let loaded = read_project(&ctx, &path, &loaded_pcbs).await;
                    ctx.send_event(Event::ProjectLoaded {
                        path,
                        loaded,
                    });
                }))
            }),
            Event::ProjectLoaded {
                path,
                loaded,
            } => Box::new(move |model: &mut Model| {
                let LoadedProject {
                    project,
                    migrated,
                    pcbs,
                    pcb_error,
                } = loaded.map_err(AppError::StoresError)?;

                let project_directory = path.parent().unwrap().to_path_buf();
                model
                    .model_project
                    .replace(ModelProject {
                        path,
                        project_directory,
                        project,
                        modified: migrated,
                        writing: None,
                    });
                for (pcb_path, pcb) in pcbs {
                    // loaded while the project was read.
                    model
                        .model_pcbs
                        .entry(pcb_path)
                        .or_insert(ModelPcb {
                            pcb,
                            modified: false,
                            writing: None,
                        });
                }
                if let Some(error) = pcb_error {
                    return Err(AppError::Kept(Box::new(AppError::StoresError(error))));
                }

                Ok(render::render())
            }),
            Event::Save => Box::new(|model: &mut Model| model.save_project()),
            Event::ProjectWritten {
                path,
                output,
            } => Box::new(move |model: &mut Model| model.project_written(path, output)),
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

                Self::create_and_add_pcb(name, units, unit_map, model, &pcb_path)
            }),
            Event::CreatePcb {
                path: pcb_path,
                name,
                units,
                unit_map,
            } => Box::new(move |model: &mut Model| {
                Self::create_and_add_pcb(name, units, unit_map.unwrap_or_default(), model, &pcb_path)
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
            } => Box::new(move |_model: &mut Model| {
                // Note: doesn't require a project.
                info!("Load PCB. path: {:?}", &path);

                Ok(Self::read_pcb(path))
            }),
            Event::PcbRead {
                path,
                output,
            } => Box::new(move |model: &mut Model| model.pcb_read(path, output)),
            Event::SavePcb {
                path,
            } => Box::new(move |model: &mut Model| {
                // Note: doesn't require a project.
                info!("Save PCB. path: {:?}", &path);

                model.save_pcb(&path)
            }),
            Event::PcbWritten {
                path,
                output,
            } => Box::new(move |model: &mut Model| model.pcb_written(path, output)),
            Event::RefreshPcbs => Box::new(move |model: &mut Model| {
                info!("Refreshing PCBs");

                // `STORE-10`, each is read again, replacing the PCB loaded.
                Ok(Command::all(
                    model
                        .model_pcbs
                        .keys()
                        .cloned()
                        .map(Self::read_pcb)
                        .collect::<Vec<_>>(),
                ))
            }),
            Event::CreateProcess {
                path,
                reference,
                preset,
            } => Box::new(move |model: &mut Model| {
                // Note: doesn't require a project.
                info!(
                    "Create process. path: {:?}, reference: {}, preset: {:?}",
                    &path, reference, preset
                );

                let mut definition = match preset {
                    Some(preset) => ProcessPresetFactory::by_preset_name(&preset).map_err(|cause| match cause {
                        ProcessPresetFactoryError::UnknownPreset {
                            preset,
                        } => AppError::ProcessError(ProcessError::UnknownPreset {
                            preset,
                            presets: ProcessPresetFactory::available_presets(),
                        }),
                    })?,
                    None => ProcessDefinition::new(reference.clone()),
                };
                definition.reference = reference;

                // `DOC-1`, as with a new project or PCB, a new process is saved as it is created, and is created once
                // it's saved, see `Event::ProcessWritten`.
                info!("Saving process. path: {}", path.display());
                let contents = file::to_string(&definition).map_err(|cause| AppError::IoError(cause.into()))?;
                model
                    .creating_processes
                    .insert(path.clone(), ModelProcess {
                        definition,
                        modified: true,
                        writing: None,
                    });

                Ok(Self::write_process(path, contents))
            }),
            Event::LoadProcess {
                path,
            } => Box::new(move |_model: &mut Model| {
                // Note: doesn't require a project.
                info!("Load process. path: {:?}", &path);

                Ok(Self::read_process(path.clone(), move |output| Event::ProcessRead {
                    path: path.clone(),
                    output,
                }))
            }),
            Event::ProcessRead {
                path,
                output,
            } => Box::new(move |model: &mut Model| model.process_read(path, output)),
            Event::SaveProcess {
                path,
            } => Box::new(move |model: &mut Model| model.save_process(&path)),
            Event::ProcessWritten {
                path,
                output,
            } => Box::new(move |model: &mut Model| model.process_written(path, output)),
            Event::SaveAllProcesses => Box::new(|model: &mut Model| {
                let paths = model
                    .model_processes
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();

                let mut commands = vec![];
                for path in paths {
                    commands.push(model.save_process(&path)?);
                }

                Ok(Command::all(commands))
            }),
            Event::ApplyProcessDefinition {
                path,
                process_definition,
            } => Box::new(move |model: &mut Model| {
                let ModelProcess {
                    definition,
                    modified,
                    ..
                } = model.model_process_mut(&path)?;

                process_definition
                    .validate()
                    .map_err(|cause| AppError::ProcessError(ProcessError::InvalidProcess(cause)))?;

                if !(*definition).eq(&process_definition) {
                    info!(
                        "Applied process definition. path: {:?}, process: {:?}",
                        path, process_definition
                    );
                    *definition = process_definition;
                    *modified = true;
                }

                Ok(render::render())
            }),
            Event::EditProcess {
                path,
                edit,
            } => Box::new(move |model: &mut Model| {
                let ModelProcess {
                    definition,
                    modified,
                    ..
                } = model.model_process_mut(&path)?;

                info!("Edit process. path: {:?}, edit: {:?}", path, edit);

                *modified |= definition
                    .apply_edit(edit)
                    .map_err(|cause| AppError::ProcessError(ProcessError::InvalidProcess(cause)))?;

                Ok(render::render())
            }),
            Event::RequestProcessView {
                path,
            } => Box::new(move |model: &mut Model| {
                let ModelProcess {
                    definition, ..
                } = model.model_process_mut(&path)?;

                Ok(process_view_renderer::view(ProcessView::Definition {
                    path: path.clone(),
                    definition: definition.clone(),
                }))
            }),
            Event::SaveAllPcbs => Box::new(|model: &mut Model| {
                let paths = model
                    .model_pcbs
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();

                let mut commands = vec![];
                for path in paths {
                    info!("Save PCB. path: {:?}", path);
                    commands.push(model.save_pcb(&path)?);
                }

                Ok(Command::all(commands))
            }),
            Event::AddPcb {
                pcb_file,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project_directory, ..
                } = model
                    .model_project
                    .as_ref()
                    .ok_or(AppError::OperationRequiresProject)?;

                let pcb_path = pcb_file.build_path(project_directory);

                // `STORE-10`, added once it's read, see `Event::ProjectPcbRead`.
                Ok(Command::request_from_shell(StoresOperation::ReadDocument {
                    path: pcb_path.clone(),
                })
                .then_send(move |output| Event::ProjectPcbRead {
                    pcb_file: pcb_file.clone(),
                    path: pcb_path.clone(),
                    output,
                }))
            }),
            Event::ProjectPcbRead {
                pcb_file,
                path: pcb_path,
                output,
            } => Box::new(move |model: &mut Model| {
                let pcb = Model::pcb_from_output(&pcb_path, output)?;

                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                project::add_pcb(project, &pcb_file).map_err(AppError::PcbOperationError)?;

                model
                    .model_pcbs
                    .insert(pcb_path, ModelPcb {
                        pcb,
                        modified: false,
                        writing: None,
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
            Event::AddProcess {
                process_file,
            } => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project_directory, ..
                } = model
                    .model_project
                    .as_ref()
                    .ok_or(AppError::OperationRequiresProject)?;

                let process_path = process_file.build_path(project_directory);

                // `STORE-10`, added once it's read, see `Event::ProjectProcessRead`.
                Ok(Self::read_process(process_path.clone(), move |output| {
                    Event::ProjectProcessRead {
                        process_file: process_file.clone(),
                        path: process_path.clone(),
                        output,
                    }
                }))
            }),
            Event::ProjectProcessRead {
                process_file,
                path: process_path,
                output,
            } => Box::new(move |model: &mut Model| {
                let definition = Model::process_from_output(&process_path, output)?;

                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                project
                    .add_process(process_file, definition)
                    .map_err(AppError::ProcessError)?;

                *modified |= true;

                Ok(render::render())
            }),
            Event::RemoveProcess {
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

                project
                    .remove_process(&process_reference)
                    .map_err(AppError::ProcessError)?;

                *modified = true;

                Ok(render::render())
            }),
            Event::RefreshProcesses => Box::new(move |model: &mut Model| {
                let ModelProject {
                    project,
                    project_directory,
                    ..
                } = model
                    .model_project
                    .as_ref()
                    .ok_or(AppError::OperationRequiresProject)?;

                let files = project
                    .processes
                    .iter()
                    .filter_map(|project_process| {
                        project_process
                            .process_file
                            .as_ref()
                            .map(|process_file| {
                                (
                                    project_process
                                        .definition
                                        .reference
                                        .clone(),
                                    process_file.build_path(project_directory),
                                )
                            })
                    })
                    .collect::<Vec<_>>();

                // `STORE-10`, read every file first, so that nothing changes if one can't be read, see
                // `Event::ProcessesRead`.
                Ok(Command::new(|ctx| async move {
                    let mut definitions = vec![];
                    let mut error = None;
                    for (process_reference, process_path) in files {
                        info!("Loading process. path: {}", process_path.display());
                        let definition = read_document(&ctx, &process_path)
                            .await
                            .and_then(|document| {
                                document.ok_or_else(|| format!("Process not found. path: {:?}", process_path))
                            })
                            .and_then(|document| process_from_contents(&process_path, &document.contents));
                        match definition {
                            Ok(definition) => definitions.push((process_reference, definition)),
                            Err(cause) => {
                                error = Some(cause);
                                break;
                            }
                        }
                    }
                    ctx.send_event(Event::ProcessesRead {
                        definitions: match error {
                            Some(error) => Err(error),
                            None => Ok(ProcessDefinitions(definitions)),
                        },
                    });
                }))
            }),
            Event::ProcessesRead {
                definitions,
            } => Box::new(move |model: &mut Model| {
                let ProcessDefinitions(definitions) = definitions.map_err(AppError::StoresError)?;

                let ModelProject {
                    project,
                    modified,
                    ..
                } = model
                    .model_project
                    .as_mut()
                    .ok_or(AppError::OperationRequiresProject)?;

                // update a copy, so that nothing changes if one process can't be updated.
                let mut updated_project = project.clone();
                let mut changed = false;
                for (process_reference, definition) in definitions {
                    changed |= updated_project
                        .replace_process(&process_reference, definition)
                        .map_err(AppError::ProcessError)?;
                }

                if changed {
                    *project = updated_project;
                    *modified = true;
                } else {
                    info!("Processes unchanged.");
                }

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
                let stored = model.stored.clone();
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

                // `STORE-11`, the placements of the variant being assigned are read before the project is changed.
                let assigning = assigning_design_variant(&pcbs, &unit, &variant_name);
                stored.require(&Self::placements_needed(project, &pcbs, path, assigning))?;

                project
                    .update_assignment(&pcbs, unit.clone(), variant_name)
                    .map_err(AppError::OperationError)?;
                *modified |= true;

                let refresh_result = Self::refresh_project(project, &pcbs, path, &stored)?;
                *modified |= refresh_result;

                Ok(render::render())
            }),
            Event::RefreshFromDesignVariants => Box::new(|model: &mut Model| {
                let stored = model.stored.clone();
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
                let refresh_result = Self::refresh_project(project, &pcbs, path, &stored)?;
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
                let stored = model.stored.clone();
                let (
                    ModelProject {
                        project,
                        modified,
                        ..
                    },
                    directory,
                ) = Self::model_project_and_directory(model)?;

                // the phase keeps the load-out as it was given, and it's read and written relative to the project's
                // directory, as every event reads it (`ISSUE-7`).
                let load_out_source = project_load_out_source(&directory, &load_out.to_string());

                // `STORE-11`, read first, so the load-out is made only if there isn't one.
                stored.require(&[(RecordsKind::LoadOut, load_out_source.clone())])?;

                let process = project
                    .find_process(&process_reference)
                    .map_err(AppError::ProcessError)?
                    .clone();

                *modified |= true;

                // Written only if the event succeeds.
                let create_load_out = stored
                    .load_out(&load_out_source)?
                    .is_none();
                if create_load_out {
                    info!("Created load-out. source: '{}'", load_out_source);
                }

                project
                    .update_phase(reference, process.reference.clone(), load_out.to_string(), pcb_side)
                    .map_err(AppError::PhaseError)?;

                if create_load_out {
                    model
                        .pending_writes
                        .push((load_out_source, RecordsOf::LoadOut(vec![])));
                }

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
                let stored = model.stored.clone();
                let mut writes = vec![];
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

                let phase = project
                    .phases
                    .get(&phase_reference)
                    .ok_or(AppError::UnknownPhaseReference(phase_reference.clone()))?
                    .clone();

                let load_out_source =
                    try_build_phase_load_out_source(&directory, &phase).map_err(AppError::SourceError)?;

                // `STORE-11`, all read before the project is changed.
                let mut needed = Self::placements_needed(project, &pcbs, path, None);
                needed.push((RecordsKind::LoadOut, load_out_source.clone()));
                stored.require(&needed)?;

                let refresh_result = Self::refresh_project(project, &pcbs, path, &stored)?;
                *modified |= refresh_result;

                let parts = project::assign_placements_to_phase(project, &phase, operation.clone(), placements_pattern)
                    .map_err(|cause| AppError::ProjectError(ProjectError::UnableToAssignPhaseToPlacements(cause)))?;

                trace!("Required load_out parts: {:?}", parts);

                *modified |= project::refresh_phase_operation_states(project);

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
                        let mut items = stored.existing_load_out(&load_out_source)?;
                        stores::load_out::add_parts(&mut items, &parts);
                        writes.push((load_out_source, RecordsOf::LoadOut(items)));
                    }
                    SetOrClearAction::Clear => {
                        // FUTURE not currently sure if cleanup should happen automatically or if it should be explicit.
                    }
                }
                model.pending_writes.extend(writes);
                Ok(render::render())
            }),
            Event::AddPartsToLoadout {
                phase: phase_reference,
                manufacturer: manufacturer_pattern,
                mpn: mpn_pattern,
            } => Box::new(move |model: &mut Model| {
                let stored = model.stored.clone();
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
                    try_build_phase_load_out_source(&directory, phase).map_err(AppError::SourceError)?;
                stored.require(&[(RecordsKind::LoadOut, load_out_source.clone())])?;

                let parts = project::find_phase_parts(project, &phase_reference, manufacturer_pattern, mpn_pattern);

                let mut items = stored.existing_load_out(&load_out_source)?;
                stores::load_out::add_parts(&mut items, &parts);
                model
                    .pending_writes
                    .push((load_out_source, RecordsOf::LoadOut(items)));

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
                let stored = model.stored.clone();
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
                stored.require(&[(RecordsKind::LoadOut, load_out_source.clone())])?;

                let mut items = stored.existing_load_out(&load_out_source)?;
                stores::load_out::assign_feeder(&mut items, &process, &feeder_reference, &manufacturer, &mpn)
                    .map_err(|cause| AppError::OperationError(cause.into()))?;
                model
                    .pending_writes
                    .push((load_out_source, RecordsOf::LoadOut(items)));
                Ok(render::render())
            }),
            Event::SetPlacementOrdering {
                phase: reference,
                placement_orderings,
            } => Box::new(move |model: &mut Model| {
                let stored = model.stored.clone();
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

                let refresh_result = Self::refresh_project(project, &pcbs, path, &stored)?;
                *modified |= refresh_result;

                *modified |= project::update_placement_orderings(project, &reference, &placement_orderings)
                    .map_err(AppError::OperationError)?;

                Ok(render::render())
            }),
            Event::GenerateArtifacts => Box::new(|model: &mut Model| {
                let stored = model.stored.clone();
                let (
                    ModelProject {
                        project,
                        modified,
                        ..
                    },
                    pcbs,
                    project_directory,
                ) = { Self::model_project_and_pcbs(model) }?;

                // `STORE-11`, all read before the project is changed.
                let mut needed = Self::phase_load_outs_needed(project, &project_directory)?;
                needed.extend(Self::packages_needed(project));
                stored.require(&needed)?;

                *modified |= project::refresh_phase_operation_states(project);

                let phase_load_out_item_map =
                    Self::build_phase_load_out_item_map(project, &project_directory, &stored)?;

                let mut package_mappings = Vec::new();
                let part_packages_map = Self::part_packages_map(project, &stored, &mut package_mappings)?;

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
                        .map(|process| process.definition.reference.clone())
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
                            args: HashMap::from([(
                                "name".to_string(),
                                Arg::String(process.definition.reference.to_string()),
                            )]),
                            path: format!("/processes/{}", process.definition.reference).to_string(),
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
                    .find_project_process(&process_reference)
                    .map_err(|_| AppError::UnknownProcessReference(process_reference.clone()))?;

                Ok(project_view_renderer::view(ProjectView::ProcessDefinition(
                    process.clone(),
                )))
            }),
            Event::RequestPhasePlacementsView {
                phase_reference,
            } => Box::new(move |model: &mut Model| {
                let stored = model.stored.clone();
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

                // `STORE-11`
                let mut needed = vec![(RecordsKind::LoadOut, load_out_source.clone())];
                needed.extend(Self::packages_needed(project));
                stored.require(&needed)?;

                let loadout_items = stored.existing_load_out(&load_out_source)?;

                let mut placements: Vec<(&ObjectPath, &PlacementState)> = project
                    .placements
                    .iter()
                    .filter(|(_path, state)| match &state.phase {
                        Some(candidate_phase) if phase_reference == *candidate_phase => true,
                        _ => false,
                    })
                    .collect();

                let pcb_unit_positioning_map = project::build_pcbs_unit_positioning_map(&pcbs);

                let mut package_mappings = Vec::new();
                let part_packages_map = Self::part_packages_map(project, &stored, &mut package_mappings)?;

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
                let stored = model.stored.clone();
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

                // `STORE-11`
                let items = stored.existing_load_out(&load_out_source)?;

                let load_out_view = LoadOut {
                    phase_reference,
                    source: load_out_source,
                    items,
                };

                Ok(project_view_renderer::view(ProjectView::PhaseLoadOut(load_out_view)))
            }),
            Event::RequestProjectReportView {} => Box::new(|model: &mut Model| {
                let stored = model.stored.clone();
                let (
                    ModelProject {
                        project, ..
                    },
                    pcbs,
                    project_directory,
                ) = { Self::model_project_and_pcbs(model) }?;

                let phase_load_out_item_map =
                    Self::build_phase_load_out_item_map(project, &project_directory, &stored)?;

                let report = report::project_generate_report(project, &pcbs, &phase_load_out_item_map);

                Ok(project_view_renderer::view(ProjectView::ProjectReport(report)))
            }),
        }
    }

    /// Each phase's load-out source.
    fn phase_load_out_sources(
        project: &Project,
        project_directory: &PathBuf,
    ) -> Result<Vec<(Reference, LoadOutSource)>, AppError> {
        project
            .phases
            .iter()
            .map(|(reference, phase)| {
                let source =
                    try_build_phase_load_out_source(project_directory, phase).map_err(AppError::SourceError)?;
                Ok((reference.clone(), source))
            })
            .collect()
    }

    /// The records needed for [`Self::build_phase_load_out_item_map`].
    fn phase_load_outs_needed(
        project: &Project,
        project_directory: &PathBuf,
    ) -> Result<Vec<(RecordsKind, Source)>, AppError> {
        Ok(Self::phase_load_out_sources(project, project_directory)?
            .into_iter()
            .map(|(_, source)| (RecordsKind::LoadOut, source))
            .collect())
    }

    fn build_phase_load_out_item_map(
        project: &Project,
        project_directory: &PathBuf,
        stored: &StoredRecords,
    ) -> Result<BTreeMap<Reference, Vec<LoadOutItem>>, AppError> {
        stored.require(&Self::phase_load_outs_needed(project, project_directory)?)?;

        Self::phase_load_out_sources(project, project_directory)?
            .into_iter()
            .map(|(reference, source)| Ok((reference, stored.existing_load_out(&source)?)))
            .collect()
    }

    /// The records needed for [`Self::part_packages_map`], none if the project has no package sources.
    fn packages_needed(project: &Project) -> Vec<(RecordsKind, Source)> {
        match (
            project
                .library_config
                .package_source
                .as_ref(),
            project
                .library_config
                .package_mappings_source
                .as_ref(),
        ) {
            (Some(packages_source), Some(package_mappings_source)) => vec![
                (RecordsKind::Packages, packages_source.clone()),
                (RecordsKind::PackageMappings, package_mappings_source.clone()),
            ],
            _ => vec![],
        }
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

    // Due to the lifetimes of the mappings, the storage for the package mappings needs to be provided up-front.
    fn part_packages_map<'a, 'b>(
        project: &'a Project,
        stored: &'b StoredRecords,
        // Provide a mutable reference to a variable that will be populated
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
        stored.require(&Self::packages_needed(project))?;

        // `STORE-11`, read by the stores service.
        let packages = stored.packages(packages_source)?;
        let rows = stored.package_mappings(package_mappings_source)?;

        // Make the mappings and store them in the provided output vector
        *package_mappings_storage = stores::package_mappings::build_package_mappings(packages, rows)
            .map_err(|error| AppError::OperationError(anyhow!("package source error. cause: {:?}", error)))?;

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
    ) -> Result<Command<Effect, Event>, AppError> {
        let pcb = planning::pcb::create_pcb(name, units, unit_map).map_err(AppError::PcbOperationError)?;

        info!("Saving PCB. path: {:?}", pcb_path);
        let contents = file::to_string(&pcb).map_err(|cause| AppError::IoError(cause.into()))?;
        // `DOC-1`, created once its file is written, see `Event::PcbWritten`.
        model
            .creating_pcbs
            .insert(pcb_path.clone(), ModelPcb {
                pcb,
                // not saved, yet
                modified: true,
                writing: None,
            });

        Ok(Self::write_pcb(pcb_path.clone(), contents))
    }

    /// Has the stores service read the PCB's file, then gives the core [`Event::PcbRead`].
    fn read_pcb(path: PathBuf) -> Command<Effect, Event> {
        info!("Loading PCB. path: {:?}", path);
        Command::request_from_shell(StoresOperation::ReadDocument {
            path: path.clone(),
        })
        .then_send(move |output| Event::PcbRead {
            path: path.clone(),
            output,
        })
    }

    /// Has the stores service read the process's file, then gives the core the event made from the output.
    fn read_process(
        path: PathBuf,
        event: impl FnOnce(StoresOutput) -> Event + Send + 'static,
    ) -> Command<Effect, Event> {
        info!("Loading process. path: {}", path.display());
        Command::request_from_shell(StoresOperation::ReadDocument {
            path,
        })
        .then_send(event)
    }

    /// Has the stores service write the process's file, then gives the core [`Event::ProcessWritten`].
    fn write_process(path: PathBuf, contents: String) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::WriteDocument {
            path: path.clone(),
            contents,
        })
        .then_send(move |output| Event::ProcessWritten {
            path: path.clone(),
            output,
        })
    }

    /// Has the stores service write the PCB's file, then gives the core [`Event::PcbWritten`].
    fn write_pcb(path: PathBuf, contents: String) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::WriteDocument {
            path: path.clone(),
            contents,
        })
        .then_send(move |output| Event::PcbWritten {
            path: path.clone(),
            output,
        })
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

        Model::require_project_pcbs_loaded(&model_project.project, &model.model_pcbs, &project_directory)?;

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

impl Planner {
    /// Has the stores service write the project's file, then gives the core [`Event::ProjectWritten`].
    fn write_project(path: PathBuf, contents: String) -> Command<Effect, Event> {
        Command::request_from_shell(StoresOperation::WriteDocument {
            path: path.clone(),
            contents,
        })
        .then_send(move |output| Event::ProjectWritten {
            path: path.clone(),
            output,
        })
    }
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = PlannerOperationViewModel;
    type Effect = Effect;

    fn update(&self, event: Self::Event, model: &mut Self::Model) -> Command<Self::Effect, Self::Event> {
        // `LOG-1`, every event, so the logs show what each front end asked for, in order.
        debug!("Received event. event: {:?}", event);

        // `STORE-11`, given the records it needs, see `stored`.
        let (event, records) = match event {
            Event::WithRecords {
                event,
                records,
            } => (*event, records),
            event => (event, StoredRecords::default()),
        };
        let retry = event.clone();
        // what the event changes is undone if it fails, so a refused event changes nothing.
        let before = model.clone();

        model.stored = Arc::new(records);
        let try_fn = self.update_inner(event);
        let result = try_fn(model);
        let records = std::mem::take(&mut model.stored);
        let writes = std::mem::take(&mut model.pending_writes);

        match result {
            Err(AppError::NeedsRecords(needed)) => {
                *model = before;
                debug!("Reading records for the event. records: {:?}", needed);
                let mut records = Arc::unwrap_or_clone(records);
                Command::new(|ctx| async move {
                    for (kind, source) in needed {
                        let result = read_records(&ctx, kind, &source).await;
                        records.insert(kind, source, result);
                    }
                    ctx.send_event(Event::WithRecords {
                        event: Box::new(retry),
                        records,
                    });
                })
            }
            Err(e) => {
                let e = match e {
                    AppError::Kept(e) => *e,
                    e => {
                        *model = before;
                        e
                    }
                };
                // `LOG-2`, so a refusal can be seen in the logs, whichever front end sent the event.
                warn!("Event failed. error: {:?}", e);
                model
                    .error
                    .replace((chrono::DateTime::from(SystemTime::now()), format!("{:?}", e)));
                render::render()
            }
            Ok(command) => {
                model.error.take();
                // `STORE-11`, the records the event changed.
                let writes = writes
                    .into_iter()
                    .map(|(source, records)| {
                        Command::request_from_shell(StoresOperation::WriteRecords {
                            source: source.clone(),
                            records,
                        })
                        .then_send(move |output| Event::RecordsWritten {
                            source: source.clone(),
                            output,
                        })
                    })
                    .collect::<Vec<_>>();
                command.and(Command::all(writes))
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

        let processes_modified = model
            .model_processes
            .values()
            .any(|process| process.modified);

        let view_model = PlannerOperationViewModel {
            project_modified,
            pcbs_modified,
            processes_modified,
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
    #[error("PCB error. cause: {0}")]
    PcbOperationError(PcbOperationError),
    #[error("IO error. cause: {0}")]
    IoError(std::io::Error),
    /// A store operation was refused, saying why, for example a file that can't be written (`STORE-10`).
    #[error("Stores error. cause: {0}")]
    StoresError(String),
    /// The shell gave the result of a different operation, a fault in the shell (`STORE-7`).
    #[error("Unexpected output. output: {output:?}")]
    UnexpectedOutput { output: StoresOutput },
    /// The event needs these records, which haven't been read, so it's processed again once they are (`STORE-11`).
    #[error("Needs records. records: {0:?}")]
    NeedsRecords(Vec<(RecordsKind, Source)>),
    /// The event failed, but what it changed is kept, for example a project loaded without one of its PCBs.
    #[error("{0}")]
    Kept(Box<AppError>),

    #[error("Unknown phase reference. reference: {0}")]
    UnknownPhaseReference(Reference),
    #[error("Unknown process reference. reference: {0}")]
    UnknownProcessReference(ProcessReference),
    #[error("Process not loaded. path: {0:?}")]
    ProcessNotLoaded(PathBuf),
}

impl Planner {
    fn refresh_project(
        project: &mut Project,
        pcbs: &[&Pcb],
        path: &PathBuf,
        stored: &StoredRecords,
    ) -> Result<bool, AppError> {
        let directory = path.parent().unwrap();

        let unique_design_variants = project.unique_design_variants(pcbs);

        // `STORE-11`, read by the stores service.
        let needed = unique_design_variants
            .iter()
            .map(|design_variant| (RecordsKind::Placements, placements_source(directory, design_variant)))
            .collect::<Vec<_>>();
        stored.require(&needed)?;
        let design_variant_placement_map = unique_design_variants
            .into_iter()
            .map(|design_variant| {
                let placements = stored
                    .placements(&placements_source(directory, &design_variant))?
                    .to_vec();
                Ok((design_variant, placements))
            })
            .collect::<Result<BTreeMap<_, _>, AppError>>()?;
        let refresh_result = project::refresh_from_design_variants(project, pcbs, design_variant_placement_map);

        if let Ok(modified) = &refresh_result {
            trace!("Refreshed from design variants. modified: {}", modified);
        }

        refresh_result.map_err(AppError::ProjectError)
    }

    /// The placements needed by [`Self::refresh_project`], with those of a variant about to be assigned, so they're
    /// read before the project is changed (`STORE-11`).
    fn placements_needed(
        project: &Project,
        pcbs: &[&Pcb],
        path: &PathBuf,
        assigning: Option<DesignVariant>,
    ) -> Vec<(RecordsKind, Source)> {
        let directory = path.parent().unwrap();
        let mut design_variants = project.unique_design_variants(pcbs);
        design_variants.extend(assigning);
        design_variants
            .iter()
            .map(|design_variant| (RecordsKind::Placements, placements_source(directory, design_variant)))
            .collect()
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

/// Build a load-out source, where the load-out source *may* be a path relative to the project's directory, or an
/// absolute one (`ISSUE-7`).
///
/// It isn't checked that it exists, as the core does no IO; the stores service says if it doesn't (`STORE-11`).
fn try_build_phase_load_out_source(project_directory: &PathBuf, phase: &Phase) -> Result<LoadOutSource, SourceError> {
    Ok(project_load_out_source(project_directory, &phase.load_out_source))
}

/// A load-out, relative to the project's directory unless it's absolute (`ISSUE-7`).
fn project_load_out_source(project_directory: &Path, load_out: &str) -> LoadOutSource {
    LoadOutSource::from_path(project_directory, Path::new(load_out))
}

/// The design variant a unit is about to be assigned, if it can be, so its placements can be read first (`STORE-11`).
fn assigning_design_variant(
    pcbs: &[&Pcb],
    unit: &ObjectPath,
    variant_name: &Option<VariantName>,
) -> Option<DesignVariant> {
    let variant_name = variant_name.clone()?;
    // reminder: instance and pcb_unit are 1-based in the object path
    let (pcb_instance, pcb_unit) = unit.pcb_instance_and_unit().ok()?;
    let pcb = pcbs.get(pcb_instance.checked_sub(1)? as usize)?;
    let design_index = pcb
        .unit_map
        .get(&pcb_unit.checked_sub(1)?)?;
    let design_name = pcb
        .design_names
        .get_index(*design_index as usize)?
        .clone();
    Some(DesignVariant {
        design_name,
        variant_name,
    })
}

/// A design variant's placements, `<directory>/<design>_<variant>_placements.csv`.
fn placements_source(directory: &Path, design_variant: &DesignVariant) -> Source {
    let DesignVariant {
        design_name,
        variant_name,
    } = design_variant;
    Source::File(directory.join(format!("{}_{}_placements.csv", design_name, variant_name)))
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
