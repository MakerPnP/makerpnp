use std::path::PathBuf;

use egui::{Ui, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use planner_app::{ObjectPath, PcbSide, PlacementPositionUnit};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use tracing::debug;

use crate::project::tabs::ProjectTabs;
use crate::project::{Project, ProjectAction, ProjectContext, ProjectKey, ProjectUiCommand};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};

/// This is persisted between application restarts
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct ProjectTab {
    pub project_key: ProjectKey,

    // path is required here so the project can be loaded when the application restarts
    pub path: PathBuf,
    pub label: String,

    #[serde(skip)]
    pub modified: bool,

    #[serde(skip)]
    pub component: ComponentState<ProjectTabUiCommand>,

    pub project_tabs: Value<ProjectTabs>,
}

#[derive(Debug, Clone)]
pub enum ProjectTabUiCommand {
    ProjectCommand { key: ProjectKey, command: ProjectUiCommand },
}

#[derive(Debug)]
pub enum ProjectTabAction {
    ProjectTask(ProjectKey, Task<ProjectAction>),
    SetModifiedState(bool),
    RequestRepaint,
    ShowPcb(PathBuf),
    ShowProcess(PathBuf),
    LocateComponent {
        pcb_file: PathBuf,
        object_path: ObjectPath,
        pcb_side: PcbSide,
        design_position: PlacementPositionUnit,
        unit_position: PlacementPositionUnit,
    },
}

pub struct ProjectTabContext {
    pub tab_key: TabKey,
    pub projects: Value<SlotMap<ProjectKey, Project>>,
}

impl ProjectTab {
    pub fn new(label: String, path: PathBuf, project_key: ProjectKey, project_tabs: Value<ProjectTabs>) -> Self {
        debug!(
            "Creating project tab. key: {:?}, path: {}",
            &project_key,
            &path.display()
        );
        Self {
            project_key,
            project_tabs,
            path,
            label,
            modified: false,
            component: ComponentState::default(),
        }
    }
}

impl Tab for ProjectTab {
    type Context = ProjectTabContext;

    fn label(&self) -> WidgetText {
        let mut label = egui::RichText::new(self.label.clone());

        if self.modified {
            label = label.italics();
        }

        egui::widget_text::WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, tab_context: &mut Self::Context) {
        let mut project_tab_context = ProjectTabContext {
            tab_key: tab_key.clone(),
            projects: tab_context.projects.clone(),
        };

        UiComponent::ui(self, ui, &mut project_tab_context);
    }

    fn on_close(&mut self, _tab_key: &TabKey, _tab_context: &mut Self::Context) -> OnCloseResponse {
        debug!("closing project. key: {:?}", self.project_key);
        let mut projects = _tab_context.projects.lock().unwrap();
        projects.remove(self.project_key);

        OnCloseResponse::Close
    }
}

impl UiComponent for ProjectTab {
    type UiContext<'context> = ProjectTabContext;
    type UiCommand = ProjectTabUiCommand;
    type UiAction = ProjectTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let projects = context.projects.lock().unwrap();
        let project = projects.get(self.project_key).unwrap();

        let mut project_context = ProjectContext {
            key: self.project_key,
        };

        project.ui(ui, &mut project_context);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            ProjectTabUiCommand::ProjectCommand {
                key,
                command,
            } => {
                let mut projects = context.projects.lock().unwrap();
                let project = projects
                    .get_mut(self.project_key)
                    .unwrap();

                let mut project_context = ProjectContext {
                    key: self.project_key,
                };

                let action: Option<ProjectAction> = project.update((key, command), &mut project_context);
                match action {
                    Some(ProjectAction::Task(key, task)) => Some(ProjectTabAction::ProjectTask(key, task)),
                    Some(ProjectAction::SetModifiedState(modified_state)) => {
                        Some(ProjectTabAction::SetModifiedState(modified_state))
                    }
                    None => None,
                    Some(ProjectAction::UiCommand(command)) => project
                        .update((key, command), &mut project_context)
                        .map(|action| ProjectTabAction::ProjectTask(key, Task::done(action))),
                    Some(ProjectAction::RequestRepaint) => Some(ProjectTabAction::RequestRepaint),
                    Some(ProjectAction::ShowPcb(path)) => Some(ProjectTabAction::ShowPcb(path)),
                    Some(ProjectAction::ShowProcess(path)) => Some(ProjectTabAction::ShowProcess(path)),
                    Some(ProjectAction::LocateComponent {
                        pcb_file,
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }) => Some(ProjectTabAction::LocateComponent {
                        pcb_file,
                        object_path,
                        pcb_side,
                        design_position,
                        unit_position,
                    }),
                }
            }
        }
    }
}
