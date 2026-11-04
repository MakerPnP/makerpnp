use egui::Ui;
use egui_i18n::tr;
use tracing::debug;

use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ProjectToolbarUiCommand {
    ProjectExplorerClicked,
    AddPcbClicked,
    AddProcessClicked,
    AddPhaseClicked,
    PackageSourcesClicked,
    RefreshClicked,
    GenerateArtifactsClicked,
    RemoveUnusedPlacementsClicked,
    ResetOperationsClicked,
}

pub enum ProjectToolbarAction {
    ShowProjectExplorer,
    PickPcbFile,
    PickProcessFile,
    ShowAddPhaseDialog,
    ShowPackageSourcesDialog,
    Refresh,
    GenerateArtifacts,
    RemoveUnusedPlacements,
    ResetOperations,
}

#[derive(Default, Debug)]
pub struct ProjectToolbar {
    pub component: ComponentState<ProjectToolbarUiCommand>,
}

impl ProjectToolbar {}

impl UiComponent for ProjectToolbar {
    type UiContext<'context> = ();
    type UiCommand = ProjectToolbarUiCommand;
    type UiAction = ProjectToolbarAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        ui.horizontal(|ui| {
            if ui
                .button(tr!("project-toolbar-button-show-explorer"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::ProjectExplorerClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-generate-artifacts"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::GenerateArtifactsClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-refresh"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::RefreshClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-remove-unused-placements"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::RemoveUnusedPlacementsClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-add-pcb"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddPcbClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-add-process"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddProcessClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-add-phase"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::AddPhaseClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-package-sources"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::PackageSourcesClicked)
            }
            if ui
                .button(tr!("project-toolbar-button-reset-operations"))
                .clicked()
            {
                self.component
                    .send(ProjectToolbarUiCommand::ResetOperationsClicked)
            }
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("ProjectToolbar command. command: {:?}", command);
        match command {
            ProjectToolbarUiCommand::ProjectExplorerClicked => Some(ProjectToolbarAction::ShowProjectExplorer),
            ProjectToolbarUiCommand::RefreshClicked => Some(ProjectToolbarAction::Refresh),
            ProjectToolbarUiCommand::AddPcbClicked => Some(ProjectToolbarAction::PickPcbFile),
            ProjectToolbarUiCommand::AddProcessClicked => Some(ProjectToolbarAction::PickProcessFile),
            ProjectToolbarUiCommand::AddPhaseClicked => Some(ProjectToolbarAction::ShowAddPhaseDialog),
            ProjectToolbarUiCommand::PackageSourcesClicked => Some(ProjectToolbarAction::ShowPackageSourcesDialog),
            ProjectToolbarUiCommand::GenerateArtifactsClicked => Some(ProjectToolbarAction::GenerateArtifacts),
            ProjectToolbarUiCommand::RemoveUnusedPlacementsClicked => {
                Some(ProjectToolbarAction::RemoveUnusedPlacements)
            }
            ProjectToolbarUiCommand::ResetOperationsClicked => Some(ProjectToolbarAction::ResetOperations),
        }
    }
}
