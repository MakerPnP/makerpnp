use std::error::Error;
use std::path::PathBuf;
use anyhow::anyhow;
use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use tracing::info;
use planning::project;
use planning::project::Project;
use pnp::pcb::PcbKind;

#[derive(Default)]
pub struct Planner;

#[derive(Default)]
pub struct Model {
    project_file_path: Option<PathBuf>,

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
    None, // we can't instantiate an empty enum, so let's have a dummy variant for now
    CreateProject {
        project_name: String,
        project_file_path: PathBuf,
    },
    Save,
    Load {
        project_file_path: PathBuf,
    },
    AddPcb { kind: PcbKind, name: String },
}

impl App for Planner {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::None => {}
            Event::CreateProject { project_name, project_file_path} => {
                let project = Project::new(project_name.to_string());
                model.project.replace(project);
                model.project_file_path.replace(project_file_path);

                self.update(Event::Save {}, model, caps);
            },
            Event::Load { project_file_path } => {
                if let Ok(project) = project::load(&project_file_path) {
                    model.project.replace(project);
                    model.project_file_path.replace(project_file_path);
                } else {
                    todo!()
                }
            },
            Event::Save => {
                if let (Some(project), Some(profile_file_path)) = (&model.project, &model.project_file_path) {
                    match project::save(project, profile_file_path) {
                        Ok(_) => { info!("Created job: {}", project.name); },
                        Err(e) => { model.error.replace(e.into()); },

                    }
                } else {
                    model.error.replace(anyhow!("Attempt to save without project and path").into());
                }
            },
            Event::AddPcb { kind, name } => {

                if let Some(project) = &mut model.project {
                    match project::add_pcb(project, kind.clone().into(), name) {
                        Ok(_) => { model.modified = true; },
                        Err(e) => { model.error.replace(Box::new(e)); },
                    }
                    self.update(Event::Save {}, model, caps);
                } else {
                    todo!()
                }
            },            
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
