use std::error::Error;
use std::path::PathBuf;
use anyhow::anyhow;
use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use regex::Regex;
use tracing::info;
use planning::design::{DesignName, DesignVariant};
use planning::process::ProcessName;
use planning::project;
use planning::project::{ProcessFactory, Project};
use planning::reference::Reference;
use planning::variant::VariantName;
use pnp::object_path::ObjectPath;
use pnp::part::Part;
use pnp::pcb::{PcbKind, PcbSide};
use stores::load_out::LoadOutSource;

extern crate serde_regex;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct Model {
    path: Option<PathBuf>,
    name: Option<String>,

    project: Option<Project>,
    modified: bool,

    error: Option<Box<dyn Error>>
}

#[derive(Effect)]
pub struct Capabilities {
    render: Render<Event>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct ViewModel {
    pub error: Option<String>
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None, // TODO REMOVE
    CreateProject {
        project_name: String,
        path: PathBuf,
    },
    Save,
    Load {
        project_name: String,
        path: PathBuf,
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
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::None => {}
            Event::CreateProject { project_name, path} => {
                let project = Project::new(project_name.to_string());
                model.project.replace(project);
                model.path.replace(path);
                model.name.replace(project_name);
                model.modified = true;

                self.update(Event::Save {}, model, caps); // TODO remove this?
            },
            Event::Load { project_name, path } => {

                let project_file_path = project::build_project_file_path(&project_name, &path);
                model.path.replace(path);
                model.name.replace(project_name);

                match project::load(&project_file_path) {
                    Ok(project) => {
                        model.project.replace(project);
                    },
                    Err(e) => {
                        model.error.replace(e.into());
                    }
                }
            },
            Event::Save => {
                if let (Some(project), Some(path), Some(project_name)) = (&model.project, &model.path, &model.name) {
                    let project_file_path = project::build_project_file_path(&project_name, path);

                    match project::save(project, &project_file_path) {
                        Ok(_) => {
                            info!("Created job: {}", project.name);
                            model.modified = false;
                        },
                        Err(e) => {
                            model.error.replace(e.into());
                        },
                    }
                } else {
                    model.error.replace(anyhow!("project, name and path required").into());
                }
            },
            Event::AddPcb { kind, name } => {
                if let Some(project) = &mut model.project {
                    match project::add_pcb(project, kind.clone().into(), name) {
                        Ok(_) => {
                            model.modified = true;

                            self.update(Event::Save {}, model, caps); // TODO remove this?
                        },
                        Err(e) => { model.error.replace(Box::new(e)); },
                    }
                    self.update(Event::Save {}, model, caps);
                } else {
                    model.error.replace(anyhow!("project required").into());
                }
            },
            Event::AssignVariantToUnit { design, variant, unit } => {
                let mut try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let (Some(project), Some(path)) = (&mut model.project, &model.path) {
                        project.update_assignment(unit.clone(), DesignVariant { design_name: design.clone(), variant_name: variant.clone() })?;
                        model.modified = true;
                        let _all_parts = Self::refresh_project(project, path)?;

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    }
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(e.into());
                };
            },
            Event::RefreshFromDesignVariants => {
                if let (Some(project), Some(path)) = (&mut model.project, &model.path) {
                    if let Err(e) = Self::refresh_project(project, path) {
                        model.error.replace(e.into());
                    };
                    model.modified = true;
                } else {
                    model.error.replace(anyhow!("project and path required").into());
                }
            },
            Event::AssignProcessToParts { process: process_name, manufacturer: manufacturer_pattern, mpn: mpn_pattern } => {
                let mut try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let (Some(project), Some(path)) = (&mut model.project, &model.path) {
                        let process = project.find_process(&process_name)?.clone();
                        let all_parts = Self::refresh_project(project, path)?;
                        model.modified = true;

                        project::update_applicable_processes(project, all_parts.as_slice(), process, manufacturer_pattern, mpn_pattern);
                        
                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } // TODO error handling
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(e.into());
                };
            },
            Event::CreatePhase { process: process_name, reference, load_out, pcb_side } => {
                let mut try_fn = |model: &mut Model| -> anyhow::Result<()> {
                    if let (Some(project), Some(path)) = (&mut model.project, &model.path) {
                        let process_name_str = process_name.to_string();
                        let process = ProcessFactory::by_name(process_name_str.as_str())?;

                        project.ensure_process(&process)?;
                        model.modified = true;

                        stores::load_out::ensure_load_out(&load_out)?;

                        project.update_phase(reference, process.name.clone(), load_out.to_string(), pcb_side)?;

                        self.update(Event::Save {}, model, caps); // TODO remove this?
                    } // TODO error handling
                    Ok(())
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(e.into());
                };
            }
        }

        caps.render.render();
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {

        let error: Option<String> = match &model.error {
            None => None,
            Some(error) => Some(format!("{:?}", error)),
        };

        ViewModel {
            error
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
        let expected_view = ViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}
