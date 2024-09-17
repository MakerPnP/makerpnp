use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;
use anyhow::anyhow;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use crux_core::App;
use crux_core::capability::{CapabilityContext, Operation};
use crux_core::macros::{Capability, Effect};
use crux_core::render::Render;
use regex::Regex;
use serde_with::serde_as;
use tracing::{info, trace};
use planning::design::{DesignName, DesignVariant};
use planning::phase::PhaseError;
use planning::placement::{PlacementOperation, PlacementSortingItem};
use planning::process::{ProcessName, ProcessOperationKind, ProcessOperationSetItem};
use planning::project;
use planning::project::{PartStateError, ProcessFactory, Project};
use planning::reference::Reference;
use planning::variant::VariantName;
use pnp::load_out::LoadOutItem;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::pcb::{PcbKind, PcbSide};
use stores::load_out::LoadOutSource;

pub use crux_core::Core;
use thiserror::Error;
use pnp::placement::Placement;
use crate::view_renderer::ViewRenderer;

pub mod view_renderer; 

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;


#[derive(Default)]
pub struct ModelProject {
    file_path: PathBuf,
    name: String,
    project: Project,
    modified: bool,
    directory_path: PathBuf,
}

#[derive(Default)]
pub struct Model {
    model_project: Option<ModelProject>,
 
    error: Option<String>
}

#[derive(Effect)]
pub struct Capabilities {
    // TODO remove 'render'
    render: Render<Event>,
    view: ViewRenderer<Event>,
    navigate: Navigator<Event>
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct ProjectOperationView {
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct PhaseOverview {
    pub reference: Reference,
    pub process: ProcessName,
    pub load_out_source: String,
    pub pcb_side: PcbSide,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct PhasePlacementOrderings {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub placement_orderings: Vec<PlacementSortingItem>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub struct PlacementsList {
    // FUTURE consider introducing PlacementListItem, a subset of Placement
    placements: Vec<Placement>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Eq)]
pub struct ProjectTreeItem {
    pub name: String,
    pub path: String,
    
}
#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug, Clone, Eq)]
pub struct ProjectTree {
    pub items: Vec<ProjectTreeItem>
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
pub enum ProjectView {
    ProjectTree(ProjectTree),
    Placements(PlacementsList),
    PhaseOverview(PhaseOverview),
    PhasePlacementOrderings(PhasePlacementOrderings),
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct ProjectOperationViewModel {
    pub project: Option<ProjectOperationView>,
    pub error: Option<String>
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None, // TODO REMOVE
    CreateProject {
        project_name: String,
        /// The directory to create the project in
        directory_path: PathBuf,
    },
    CreatedProject(Result<(), ()>),
    Save,
    Load {
        project_name: String,
        /// The directory to create the project in
        directory_path: PathBuf,
    },
    AddPcb {
        kind: PcbKind,
        name: String,
    },
    AssignVariantToUnit {
        design: DesignName,
        variant: VariantName,
        unit: ObjectPath,
    },
    RefreshFromDesignVariants,
    AssignProcessToParts {
        process: ProcessName,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    CreatePhase {
        process: ProcessName,
        reference: Reference,
        load_out: LoadOutSource,
        pcb_side: PcbSide,
    },
    AssignPlacementsToPhase {
        phase: Reference,
        #[serde(with = "serde_regex")]
        placements: Regex,
    },
    AssignFeederToLoadOutItem {
        phase: Reference,
        feeder_reference: Reference,
        #[serde(with = "serde_regex")]
        manufacturer: Regex,
        #[serde(with = "serde_regex")]
        mpn: Regex,
    },
    SetPlacementOrdering {
        phase: Reference,
        placement_orderings: Vec<PlacementSortingItem>
    },
    GenerateArtifacts,
    RecordPhaseOperation {
        phase: Reference,
        operation: ProcessOperationKind,
        set: ProcessOperationSetItem,
    },
    /// Record placements operation
    RecordPlacementsOperation {
        #[serde(with = "serde_regex")]
        object_path_patterns: Vec<Regex>,
        operation: PlacementOperation,
    },
    /// Reset operations
    ResetOperations {
    },
    
    //
    // Views
    //
    ProjectTree { reference: String, path: PathBuf }
    
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ProjectOperationViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        let mut default_render = true;
        match event {
            Event::None => {}
            Event::CreateProject { project_name, directory_path} => {
                let project_file_path = project::build_project_file_path(&project_name, &directory_path);
                info!("Creating project. name: {}, directory: {:?}, path: {:?}", &project_name, &directory_path, &project_file_path);

                let project = Project::new(project_name.to_string());
                model.model_project.replace(ModelProject{
                    file_path: project_file_path,
                    directory_path,
                    name: project_name,
                    project,
                    modified: true,
                });

                default_render = false;
                self.update(Event::Save {}, model, caps); // TODO remove this?
                self.update( Event::CreatedProject(Ok(())), model, caps);
            },
            Event::CreatedProject(Ok(_)) => {
                info!("Created project successfully.");
                default_render = false;

                let path_encoded = BASE64_STANDARD.encode(model.model_project.as_ref().unwrap().file_path.to_str().unwrap());

                let path = format!("/project/load/{}", path_encoded);
                
                caps.navigate.navigate(path, |_| Event::None);
            },
            Event::CreatedProject(Err(error)) => {
                info!("Creating project failed.");
                
                model.error.replace(format!("creating project failed. cause: {:?}", error));
            },
            Event::Load { project_name, directory_path } => {
                let project_file_path = project::build_project_file_path(&project_name, &directory_path);
                info!("Load project. name: {}, directory: {:?}, path: {:?}", &project_name, &directory_path, &project_file_path);

                match project::load(&project_file_path) {
                    Ok(project) => {
                        model.model_project.replace(ModelProject {
                            file_path: project_file_path,
                            directory_path,
                            name: project_name,
                            project,
                            modified: false,
                        });
                    },
                    Err(e) => {
                        model.error.replace(format!("{:?}", e));
                    }
                }
            },
            Event::Save => {
                if let Some(ModelProject { file_path: path, name: project_name, project, modified, .. }) = &mut model.model_project {
                    info!("Save project. name: {}, path: {:?}", &project_name, &path);

                    match project::save(project, &path) {
                        Ok(_) => {
                            info!("Created job: {}, path: {:?}", project.name, path);
                            *modified = false;
                        },
                        Err(e) => {
                            model.error.replace(format!("{:?}", e));
                        },
                    }
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AddPcb { kind, name } => {
                if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                    match project::add_pcb(project, kind.clone().into(), name) {
                        Ok(_) => {
                            *modified = true;

                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        },
                        Err(e) => {
                            model.error.replace(format!("{:?}", e));
                        },
                    }
                    self.update(Event::Save {}, model, caps);
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AssignVariantToUnit { design, variant, unit } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;
                        *modified = true;
                        let _all_parts = Self::refresh_project(project, directory_path)?;

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RefreshFromDesignVariants => {
                if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                    if let Err(e) = Self::refresh_project(project, directory_path) {
                        model.error.replace(format!("{:?}", e));
                    };
                    *modified = true;
                } else {
                    model.error.replace("project required".to_string());
                }
            },
            Event::AssignProcessToParts { process: process_name, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        let process = project.find_process(&process_name)?.clone();
                        let all_parts = Self::refresh_project(project, directory_path)?;
                        *modified = true;

                        project::update_applicable_processes(project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::CreatePhase { process: process_name, reference, load_out, pcb_side } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                        let process_name_str = process_name.to_string();
                        let process = ProcessFactory::by_name(process_name_str.as_str())?;

                        project.ensure_process(&process)?;
                        *modified = true;

                        stores::load_out::ensure_load_out(&load_out)?;

                        project.update_phase(reference, process.name.clone(), load_out.to_string(), pcb_side)?;

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::AssignPlacementsToPhase { phase: reference, placements: placements_pattern } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        let _all_parts = Self::refresh_project(project, directory_path)?;
                        *modified = true;

                        let phase = project.phases.get(&reference)
                            .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                        let parts = project::assign_placements_to_phase(project, &phase, placements_pattern);
                        trace!("Required load_out parts: {:?}", parts);

                        let _modified = project::update_phase_operation_states(project);

                        for part in parts.iter() {
                            let part_state = project.part_states.get_mut(&part)
                                .ok_or_else(|| PartStateError::NoPartStateFound { part: part.clone() })?;

                            project::add_process_to_part(part_state, part, phase.process.clone());
                        }

                        stores::load_out::add_parts_to_load_out(&LoadOutSource::from_str(&phase.load_out_source).unwrap(), parts)?;

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::AssignFeederToLoadOutItem { phase: reference, feeder_reference, manufacturer, mpn } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, .. }) = &mut model.model_project {
                        let phase = project.phases.get(&reference)
                            .ok_or(PhaseError::UnknownPhase(reference))?.clone();

                        let process = project.find_process(&phase.process)?.clone();

                        stores::load_out::assign_feeder_to_load_out_item(&phase, &process, &feeder_reference, manufacturer, mpn)?;
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::SetPlacementOrdering { phase: reference, placement_orderings } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        let _all_parts = Self::refresh_project(project, directory_path)?;
                        *modified = true;

                        *modified = project::update_placement_orderings(project, &reference, &placement_orderings)?;

                        if *modified {
                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        }
                    } else {
                        model.error.replace("project and path required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::GenerateArtifacts => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, name: project_name, project, modified, .. }) = &mut model.model_project {
                        *modified = project::update_phase_operation_states(project);

                        let phase_load_out_item_map = project.phases.iter().try_fold(BTreeMap::<Reference, Vec<LoadOutItem>>::new(), |mut map, (reference, phase) | {
                            let load_out_items = stores::load_out::load_items(&LoadOutSource::from_str(&phase.load_out_source).unwrap())?;
                            map.insert(reference.clone(), load_out_items);
                            Ok::<BTreeMap<Reference, Vec<LoadOutItem>>, anyhow::Error>(map)
                        })?;

                        project::generate_artifacts(&project, directory_path, project_name, phase_load_out_item_map)?;

                        if *modified {
                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        }
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RecordPhaseOperation { phase: reference, operation, set } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        *modified = project::update_phase_operation(project, directory_path, &reference, operation.into(), set.into())?;
                        if *modified {
                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        }
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::RecordPlacementsOperation { object_path_patterns, operation } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { directory_path, project, modified, .. }) = &mut model.model_project {
                        *modified = project::update_placements_operation(project, directory_path, object_path_patterns, operation.into())?;
                        if *modified {
                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        }
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            Event::ResetOperations { } => {
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {
                        project::reset_operations(project)?;
                        *modified = true;
                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            },
            
            Event::ProjectTree { reference, path } => {

                default_render = false;
                
                // TODO use the path to find the right project to work on, for now use the only project
                
                let try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let Some(ModelProject { project, modified, .. }) = &mut model.model_project {

                        let mut project_tree = ProjectTree {
                            items: vec![
                                ProjectTreeItem { name: "Phases".to_string(), path: "/phases".to_string() }
                            ],
                        };
                        
                        for (reference, phase) in &project.phases {
                            project_tree.items.push ( ProjectTreeItem { 
                                name: reference.to_string(), 
                                path: format!("/phases/{}", reference).to_string() }
                            )
                        }
                        
                        caps.view.view(reference, ProjectView::ProjectTree(project_tree), |_|Event::None)    
                        
                    } else {
                        model.error.replace("project required".to_string());
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
            
        }

        if default_render {
            caps.render.render();
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        
        let project: Option<ProjectOperationView> = model.model_project.as_ref().map(|project| {
            ProjectOperationView {
                name: project.name.clone()
            }
        });
        
        ProjectOperationViewModel {
            project,
            error: model.error.clone(),
        }
    }
}

impl Planner {
    fn refresh_project(project: &mut Project, path: &PathBuf) -> anyhow::Result<Vec<Part>> {
        let unique_design_variants = project.unique_design_variants();
        let design_variant_placement_map = stores::placements::load_all_placements(
            &unique_design_variants,
            path
        )?;
        let all_parts = project::refresh_from_design_variants(project, design_variant_placement_map);

        // TODO make this return a 'modified' flag too
        Ok(all_parts)
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use crux_core::{assert_effect, testing::AppTester};

    #[test]
    fn minimal() {
        let hello = AppTester::<Planner, _>::default();
        let mut model = Model::default();

        // Call 'update' and request effects
        let update = hello.update(Event::None, &mut model);

        // Check update asked us to `Render`
        assert_effect!(update, Effect::Render(_));

        // Make sure the view matches our expectations
        let actual_view = &hello.view(&model);
        let expected_view = ProjectOperationViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}

#[derive(Capability)]
struct Navigator<Ev> {
    context: CapabilityContext<NavigationOperation, Ev>,
}

impl<Ev> Navigator<Ev> {
    pub fn new(context: CapabilityContext<NavigationOperation, Ev>) -> Self {
        Self {
            context,
        }
    }
}
impl<Ev: 'static> Navigator<Ev> {

    pub fn navigate<F>(&self, path: String, make_event: F)
    where
        F: FnOnce(Result<Option<String>, NavigationError>) -> Ev + Send + Sync + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            async move {
                let response = navigate(&context, path).await;
                context.update_app(make_event(response))
            }
        });
    }
}


async fn navigate<Ev: 'static>(
    context: &CapabilityContext<NavigationOperation, Ev>,
    path: String,
) -> Result<Option<String>, NavigationError> {
    context
        .request_from_shell(NavigationOperation::Navigate { path })
        .await
        .unwrap_set()
}


#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationResult {
    Ok { response: NavigationResponse },
    Err { error: NavigationError },
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationResponse {
    Navigate { previous: String }
}

impl NavigationResult {
    fn unwrap_set(self) -> Result<Option<String>, NavigationError> {
        match self {
            NavigationResult::Ok { response } => match response {
                NavigationResponse::Navigate { previous } => Ok(previous.into()),
                // _ => {
                //     panic!("attempt to convert NavigationResponse other than Ok to Option<String>")
                // }
            },
            NavigationResult::Err { error } => Err(error.clone()),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub enum NavigationOperation {
    Navigate { path: String }
}

impl Operation for NavigationOperation {
    type Output = NavigationResult;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Error)]
#[serde(rename_all = "camelCase")]
pub enum NavigationError {
    #[error("other error: {message}")]
    Other { message: String },
}