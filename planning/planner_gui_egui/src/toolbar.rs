use egui::Ui;
use egui_i18n::tr;
use tracing::debug;

use crate::tabs::TabKey;
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Debug, Clone)]
pub enum ToolbarUiCommand {
    ShowHomeTabClicked,
    CloseAllTabsClicked,
    OpenProjectClicked,
    SaveClicked(TabKey),
    NewProjectClicked,
    NewPcbClicked,
    OpenPcbClicked,
    NewProcessClicked,
    OpenProcessClicked,
    OpenPartsClicked,
}

pub enum ToolbarAction {
    ShowHomeTab,
    CloseAllTabs,

    PickProjectFile,
    AddNewProjectTab,

    PickPcbFile,
    AddNewPcbTab,

    PickProcessFile,
    AddNewProcessTab,

    AddPartsTab,

    SaveTab(TabKey),
}

pub struct ToolbarContext {
    pub active_tab: Option<TabKey>,
    pub can_save: bool,
}

pub struct Toolbar {
    pub component: ComponentState<ToolbarUiCommand>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            component: ComponentState::default(),
        }
    }
}

impl UiComponent for Toolbar {
    type UiContext<'context> = ToolbarContext;
    type UiCommand = ToolbarUiCommand;
    type UiAction = ToolbarAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        egui::Frame::new().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(tr!("toolbar-button-home"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::ShowHomeTabClicked);
                }

                ui.separator();

                if ui
                    .button(tr!("toolbar-button-new-project"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::NewProjectClicked);
                }

                if ui
                    .button(tr!("toolbar-button-open-project"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::OpenProjectClicked);
                }

                ui.separator();

                if ui
                    .button(tr!("toolbar-button-new-pcb"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::NewPcbClicked);
                }

                if ui
                    .button(tr!("toolbar-button-open-pcb"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::OpenPcbClicked);
                }

                ui.separator();

                if ui
                    .button(tr!("toolbar-button-new-process"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::NewProcessClicked);
                }

                if ui
                    .button(tr!("toolbar-button-open-process"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::OpenProcessClicked);
                }

                ui.separator();

                if ui
                    .button(tr!("toolbar-button-open-parts"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::OpenPartsClicked);
                }

                ui.separator();

                ui.add_enabled_ui(context.can_save, |ui| {
                    if ui
                        .button(tr!("toolbar-button-save"))
                        .clicked()
                    {
                        self.component
                            .send(ToolbarUiCommand::SaveClicked(context.active_tab.unwrap()));
                    }
                });

                if ui
                    .button(tr!("toolbar-button-close-all"))
                    .clicked()
                {
                    self.component
                        .send(ToolbarUiCommand::CloseAllTabsClicked);
                }
            });
        });
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("Toolbar command. command: {:?}", command);
        match command {
            ToolbarUiCommand::ShowHomeTabClicked => Some(ToolbarAction::ShowHomeTab),
            ToolbarUiCommand::CloseAllTabsClicked => Some(ToolbarAction::CloseAllTabs),
            ToolbarUiCommand::NewProjectClicked => Some(ToolbarAction::AddNewProjectTab),
            ToolbarUiCommand::OpenProjectClicked => Some(ToolbarAction::PickProjectFile),
            ToolbarUiCommand::NewPcbClicked => Some(ToolbarAction::AddNewPcbTab),
            ToolbarUiCommand::OpenPcbClicked => Some(ToolbarAction::PickPcbFile),
            ToolbarUiCommand::NewProcessClicked => Some(ToolbarAction::AddNewProcessTab),
            ToolbarUiCommand::OpenProcessClicked => Some(ToolbarAction::PickProcessFile),
            ToolbarUiCommand::OpenPartsClicked => Some(ToolbarAction::AddPartsTab),
            ToolbarUiCommand::SaveClicked(tab_key) => Some(ToolbarAction::SaveTab(tab_key)),
        }
    }
}
