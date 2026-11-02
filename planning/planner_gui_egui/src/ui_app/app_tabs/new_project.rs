use std::path::PathBuf;

use derivative::Derivative;
use egui::{Button, TextEdit, Ui, Widget, WidgetText};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignContent, AlignItems, Display, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::file_picker::Picker;
use crate::forms::Form;
use crate::forms::transforms::resize_x_transform;
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::ui_component::{ComponentState, UiComponent};

#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct NewProjectTab {
    fields: Value<NewProjectFields>,

    #[serde(skip)]
    pub component: ComponentState<NewProjectTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,
}

#[derive(Debug, Clone)]
pub enum NewProjectTabUiCommand {
    NameChanged(String),
    Submit,
    PickDirectoryClicked,
    DirectoryPicked(PathBuf),
}

#[derive(Debug)]
pub enum NewProjectTabAction {
    Submit(NewProjectArgs),
}

pub struct NewProjectTabContext {
    pub tab_key: TabKey,
}

impl Tab for NewProjectTab {
    type Context = NewProjectTabContext;

    fn label(&self) -> WidgetText {
        egui::widget_text::WidgetText::from(tr!("tab-label-new-project"))
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, _context: &mut Self::Context) {
        let mut new_project_tab_context = NewProjectTabContext {
            tab_key: tab_key.clone(),
        };
        UiComponent::ui(self, ui, &mut new_project_tab_context);
    }
}

impl UiComponent for NewProjectTab {
    type UiContext<'context> = NewProjectTabContext;
    type UiCommand = NewProjectTabUiCommand;
    type UiAction = NewProjectTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, _context: &mut Self::UiContext<'context>) {
        if let Ok(picked_directory) = self
            .file_picker
            .lock()
            .unwrap()
            .picked()
        {
            self.component
                .send(NewProjectTabUiCommand::DirectoryPicked(picked_directory.clone()));
        }

        ui.ctx().global_style_mut(|style| {
            // if this is not done, text in labels/checkboxes/etc wraps
            style.wrap_mode = Some(egui::TextWrapMode::Extend);
        });

        let form = Form::new(&self.fields, &self.component.sender, ());

        self.show_form(ui, &form);
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        _context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        match command {
            NewProjectTabUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            NewProjectTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let args = NewProjectArgs {
                    name: fields.name.clone(),
                    directory: fields.directory.clone().unwrap(),
                };
                Some(NewProjectTabAction::Submit(args))
            }
            NewProjectTabUiCommand::PickDirectoryClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_folder();
                None
            }
            NewProjectTabUiCommand::DirectoryPicked(picked_directory) => {
                self.fields.lock().unwrap().directory = Some(picked_directory);
                None
            }
        }
    }
}

impl NewProjectTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<NewProjectFields, NewProjectTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_project_form"))
            .reserve_available_width()
            .style(Style {
                align_items: Some(AlignItems::Center),
                flex_direction: FlexDirection::Column,
                size: Size {
                    width: percent(1.),
                    height: auto(),
                },
                padding: length(8.),
                gap: length(8.),
                ..default_style()
            })
            .show(|tui| {
                form.show_fields_vertical(tui, |form, tui| {
                    form.add_field_ui("name", tr!("form-new-project-input-name"), tui, {
                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                        move |ui: &mut Ui, fields, sender| {
                            let mut name_clone = fields.name.clone();
                            let output = TextEdit::singleline(&mut name_clone)
                                .desired_width(ui.available_width())
                                .show(ui);

                            if !fields.name.eq(&name_clone) {
                                sender
                                    .send(NewProjectTabUiCommand::NameChanged(name_clone))
                                    .expect("sent")
                            }

                            output.response.response
                        }
                    });

                    form.add_field_tui("directory", tr!("form-new-project-input-directory"), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            tui.style(Style {
                                display: Display::Flex,
                                align_content: Some(AlignContent::Stretch),
                                flex_grow: 1.0,
                                ..default_style()
                            })
                            .add(|tui| {
                                tui.style(Style {
                                    flex_grow: 1.0,
                                    ..default_style()
                                })
                                .ui_add_manual(
                                    |ui| {
                                        // NOTE text input does not resize with grid cell when using `.ui_add`, known issue - https://discord.com/channels/900275882684477440/904461220592119849/1338883750922293319
                                        //      as a workaround we use `ui_add_manual` for now, with `no_transform`.
                                        let mut chosen_directory = fields
                                            .directory
                                            .as_ref()
                                            .map_or("".to_string(), |path| path.display().to_string());

                                        TextEdit::singleline(&mut chosen_directory)
                                            .desired_width(ui.available_width())
                                            .interactive(false)
                                            .ui(ui)
                                    },
                                    resize_x_transform,
                                );

                                if tui
                                    .style(Style {
                                        flex_grow: 0.0,
                                        ..default_style()
                                    })
                                    .ui_add(Button::new("..."))
                                    .clicked()
                                {
                                    sender
                                        .send(NewProjectTabUiCommand::PickDirectoryClicked)
                                        .expect("sent");
                                }
                            })
                        }
                    });
                });
                if tui
                    .style(Style {
                        ..default_style()
                    })
                    .enabled_ui(form.is_valid())
                    .ui_add(Button::new(tr!("form-common-button-ok")))
                    .clicked()
                {
                    self.component
                        .send(NewProjectTabUiCommand::Submit);
                }
            });
    }
}

#[derive(Clone, Debug, Default, Validate, serde::Deserialize, serde::Serialize)]
struct NewProjectFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,

    #[validate(required(code = "form-option-error-required"))]
    directory: Option<PathBuf>,
}

/// Value object
#[derive(Debug, Clone)]
pub struct NewProjectArgs {
    pub name: String,
    pub directory: PathBuf,
}

impl NewProjectArgs {
    pub fn build_path(&self) -> PathBuf {
        let Self {
            name,
            directory,
        } = self;

        let mut project_file_path: PathBuf = directory.clone();
        project_file_path.push(format!("project-{}.mpnp.json", name));
        project_file_path
    }
}
