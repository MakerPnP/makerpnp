//! A variant builder's tab: for a new one, first the form to name it and choose its folder, then the variant builder,
//! whose variant is defined, and each data source chosen, then saved, and built (`VBGUI`).

use std::collections::BTreeMap;
use std::mem::discriminant;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use derivative::Derivative;
use egui::{Button, TextEdit, Ui, Widget, WidgetText};
use egui_i18n::tr;
use egui_taffy::taffy::prelude::{auto, length, percent};
use egui_taffy::taffy::{AlignItems, FlexDirection, Size, Style};
use egui_taffy::{Tui, TuiBuilderLogic, tui};
use serde::{Deserialize, Serialize};
use tracing::debug;
use validator::Validate;
use variantbuilder_app::variant_builders::{EXTENSION, stored_path};
use variantbuilder_app::{
    EdaTool, Event, VariantBuilderChange, VariantBuilderFile, VariantBuilderStatus, VariantBuilderView,
};

use crate::file_picker::{FileDialogs, Picker};
use crate::forms::{Form, NamedResponse, PickerFieldAction, picker_field};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::variant_builder_core::VariantBuilderCoreService;

/// A field of a variant builder whose value is a file the user chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileField {
    Placements,
    Parts,
    PartMappings,
    LoadOut,
    AssemblyRules,
    /// A list of files, to which those chosen are added.
    Substitutions,
}

/// A field of a variant builder that's a list of ref-des, typed separated by commas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ListField {
    RefDesList,
    RefDesExcludeList,
    RefDesDisableList,
}

/// The tab is persisted, so that it can be shown again when the application restarts.
#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct VariantBuilderTab {
    /// The new variant builder's name and folder, until "Ok" creates it.
    fields: Value<NewVariantBuilderFields>,

    /// The variant builder's file, once it's created or opened.
    pub path: Option<PathBuf>,

    /// The changes made to the variant builder that are to be saved, at most one for each field, applied to what its
    /// file says to show it (`VBGUI-3`). Kept once saved, when they're what the file says, so that a change the core
    /// refuses isn't lost (`FORM-5`).
    #[serde(skip)]
    pending: Vec<VariantBuilderChange>,

    /// The ref-des lists as typed, so that a comma being typed isn't removed as the list is read.
    #[serde(skip)]
    list_texts: BTreeMap<ListField, String>,

    /// The field whose file is being chosen.
    #[serde(skip)]
    picking: Option<FileField>,

    #[serde(skip)]
    pub component: ComponentState<VariantBuilderTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,
}

#[derive(Debug, Clone)]
pub enum VariantBuilderTabUiCommand {
    NameChanged(String),
    ClearDirectoryClicked,
    PickDirectoryClicked,
    DirectoryPicked(PathBuf),
    Submit,

    Changed(VariantBuilderChange),
    ListChanged(ListField, String),
    ClearClicked(FileField),
    PickClicked(FileField),
    FilesPicked(FileField, Vec<PathBuf>),
    SaveClicked,
    BuildClicked,

    // internal
    /// Opens the variant builder, reading its file.
    Open,
}

#[derive(Debug)]
pub enum VariantBuilderTabAction {
    /// Commands for this tab to handle next, in order.
    Task(Task<VariantBuilderTabUiCommand>),
}

pub struct VariantBuilderTabContext {
    pub tab_key: TabKey,
    pub variant_builders: Value<VariantBuilderCoreService>,
}

impl VariantBuilderTab {
    /// A tab for a new variant builder, or for the one whose file is given.
    pub fn new(file_dialogs: Arc<dyn FileDialogs>, path: Option<PathBuf>) -> Self {
        Self {
            path,
            file_picker: Value::new(Picker::new(file_dialogs)),
            ..Self::default()
        }
    }

    /// For a tab restored when the application starts, which has no file dialogs of its own.
    pub fn set_file_dialogs(&mut self, file_dialogs: Arc<dyn FileDialogs>) {
        self.file_picker = Value::new(Picker::new(file_dialogs));
    }

    /// What the core knows of the variant builder: its file, if it's open, and its status.
    fn known(
        &self,
        variant_builders: &Value<VariantBuilderCoreService>,
    ) -> (Option<VariantBuilderView>, VariantBuilderStatus) {
        let Some(path) = &self.path else {
            return (None, VariantBuilderStatus::default());
        };
        let mut view = variant_builders.lock().unwrap().view();
        (
            view.variant_builders.remove(path),
            view.statuses
                .remove(path)
                .unwrap_or_default(),
        )
    }

    /// What the variant builder's file says, with the changes to be saved.
    fn shown(&self, view: &VariantBuilderView) -> VariantBuilderFile {
        let mut file = view.file.clone();
        for change in self.pending.iter().cloned() {
            file.apply(change);
        }
        file
    }

    /// Keeps the change to save, replacing any to the same field.
    fn change(&mut self, change: VariantBuilderChange) {
        self.pending
            .retain(|pending| discriminant(pending) != discriminant(&change));
        self.pending.push(change);
    }

    fn process(&self, variant_builders: &Value<VariantBuilderCoreService>, event: impl FnOnce(PathBuf) -> Event) {
        let Some(path) = &self.path else {
            return;
        };
        variant_builders
            .lock()
            .unwrap()
            .process(event(path.clone()));
    }

    /// The path to keep in the variant builder's file for a file chosen (`VARB-1`).
    fn stored(&self, path: &Path) -> PathBuf {
        match &self.path {
            Some(variant_builder) => stored_path(variant_builder, path),
            None => path.to_path_buf(),
        }
    }
}

impl Tab for VariantBuilderTab {
    type Context = VariantBuilderTabContext;

    fn label(&self) -> WidgetText {
        let label = self
            .path
            .as_ref()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| tr!("tab-label-new-variant-builder"));

        WidgetText::from(label)
    }

    fn ui(&mut self, ui: &mut Ui, tab_key: &TabKey, context: &mut Self::Context) {
        let mut tab_context = VariantBuilderTabContext {
            tab_key: *tab_key,
            variant_builders: context.variant_builders.clone(),
        };
        UiComponent::ui(self, ui, &mut tab_context);
    }
}

impl UiComponent for VariantBuilderTab {
    type UiContext<'context> = VariantBuilderTabContext;
    type UiCommand = VariantBuilderTabUiCommand;
    type UiAction = VariantBuilderTabAction;

    #[profiling::function]
    fn ui<'context>(&self, ui: &mut Ui, context: &mut Self::UiContext<'context>) {
        let mut file_picker = self.file_picker.lock().unwrap();
        match (&self.path, self.picking) {
            (None, _) => {
                if let Ok(picked) = file_picker.picked() {
                    self.component
                        .send(VariantBuilderTabUiCommand::DirectoryPicked(picked));
                }
            }
            (Some(_), Some(FileField::Substitutions)) => {
                if let Ok(picked) = file_picker.picked_multi() {
                    self.component
                        .send(VariantBuilderTabUiCommand::FilesPicked(
                            FileField::Substitutions,
                            picked,
                        ));
                }
            }
            (Some(_), Some(field)) => {
                if let Ok(picked) = file_picker.picked() {
                    self.component
                        .send(VariantBuilderTabUiCommand::FilesPicked(field, vec![picked]));
                }
            }
            (Some(_), None) => {}
        }
        drop(file_picker);

        match &self.path {
            None => {
                let form = Form::new(&self.fields, &self.component.sender, ());
                self.show_form(ui, &form);
            }
            Some(path) => {
                let (view, status) = self.known(&context.variant_builders);
                self.show_variant_builder(ui, path, view, status);
            }
        }
    }

    #[profiling::function]
    fn update<'context>(
        &mut self,
        command: Self::UiCommand,
        context: &mut Self::UiContext<'context>,
    ) -> Option<Self::UiAction> {
        // `LOG-3`
        debug!("VariantBuilderTab command. command: {:?}", command);
        match command {
            VariantBuilderTabUiCommand::NameChanged(name) => {
                self.fields.lock().unwrap().name = name;
                None
            }
            VariantBuilderTabUiCommand::ClearDirectoryClicked => {
                self.fields.lock().unwrap().directory = None;
                None
            }
            VariantBuilderTabUiCommand::PickDirectoryClicked => {
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_folder();
                None
            }
            VariantBuilderTabUiCommand::DirectoryPicked(directory) => {
                self.fields.lock().unwrap().directory = Some(directory);
                None
            }
            VariantBuilderTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let Some(directory) = &fields.directory else {
                    // "Ok" is disabled until the form is valid.
                    return None;
                };
                let path = directory.join(format!("{}{}", fields.name.trim(), EXTENSION));
                drop(fields);

                self.path = Some(path);
                // `VBGUI-1`, an existing file isn't replaced.
                self.process(&context.variant_builders, |path| Event::Create {
                    path,
                    overwrite: false,
                });
                None
            }
            VariantBuilderTabUiCommand::Open => {
                self.process(&context.variant_builders, |path| Event::Open {
                    path,
                });
                None
            }
            VariantBuilderTabUiCommand::Changed(change) => {
                self.change(change);
                None
            }
            VariantBuilderTabUiCommand::ListChanged(field, text) => {
                let list = text
                    .split(',')
                    .map(str::trim)
                    .filter(|ref_des| !ref_des.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                self.list_texts.insert(field, text);
                self.change(match field {
                    ListField::RefDesList => VariantBuilderChange::RefDesList(list),
                    ListField::RefDesExcludeList => VariantBuilderChange::RefDesExcludeList(list),
                    ListField::RefDesDisableList => VariantBuilderChange::RefDesDisableList(list),
                });
                None
            }
            VariantBuilderTabUiCommand::ClearClicked(field) => {
                self.change(file_change(field, vec![]));
                None
            }
            VariantBuilderTabUiCommand::PickClicked(field) => {
                self.picking = Some(field);
                let mut file_picker = self.file_picker.lock().unwrap();
                match field {
                    FileField::Substitutions => file_picker.pick_files(),
                    _ => file_picker.pick_file(),
                }
                None
            }
            VariantBuilderTabUiCommand::FilesPicked(field, paths) => {
                self.picking = None;
                let mut paths = paths
                    .iter()
                    .map(|path| self.stored(path))
                    .collect::<Vec<_>>();
                if field == FileField::Substitutions {
                    // added to those there are.
                    let (view, _) = self.known(&context.variant_builders);
                    let mut substitutions = view
                        .map(|view| self.shown(&view).substitutions)
                        .unwrap_or_default();
                    substitutions.append(&mut paths);
                    paths = substitutions;
                }
                self.change(file_change(field, paths));
                None
            }
            VariantBuilderTabUiCommand::SaveClicked => {
                // `VBGUI-3`, only the fields that differ from what the file says.
                let (Some(view), _) = self.known(&context.variant_builders) else {
                    return None;
                };
                let changes = self
                    .pending
                    .iter()
                    .filter(|change| {
                        let mut file = view.file.clone();
                        file.apply((*change).clone());
                        file != view.file
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                self.process(&context.variant_builders, |path| Event::Configure {
                    path,
                    changes,
                });
                None
            }
            VariantBuilderTabUiCommand::BuildClicked => {
                self.process(&context.variant_builders, |path| Event::Build {
                    path,
                });
                None
            }
        }
    }
}

/// The change that sets the field to the files, the first, or none, for a field with one file.
fn file_change(field: FileField, paths: Vec<PathBuf>) -> VariantBuilderChange {
    let first = paths.first().cloned();
    match field {
        FileField::Placements => VariantBuilderChange::Placements(first),
        FileField::Parts => VariantBuilderChange::Parts(first),
        FileField::PartMappings => VariantBuilderChange::PartMappings(first),
        FileField::LoadOut => VariantBuilderChange::LoadOut(first),
        FileField::AssemblyRules => VariantBuilderChange::AssemblyRules(first),
        FileField::Substitutions => VariantBuilderChange::Substitutions(paths),
    }
}

impl VariantBuilderTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<NewVariantBuilderFields, VariantBuilderTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        tui(ui, ui.id().with("new_variant_builder_form"))
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
                    form.add_field_ui("name", tr!("form-new-variant-builder-input-name"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            let mut name = fields.name.clone();
                            let output = TextEdit::singleline(&mut name)
                                .desired_width(ui.available_width())
                                .hint_text(tr!("form-new-variant-builder-input-name-placeholder"))
                                .show(ui);

                            if fields.name != name {
                                sender
                                    .send(VariantBuilderTabUiCommand::NameChanged(name))
                                    .expect("sent")
                            }

                            output.response.response
                        }
                    });

                    let directory_label = tr!("form-new-variant-builder-input-directory");
                    form.add_field_tui("directory", directory_label.clone(), tui, {
                        move |tui: &mut Tui, fields, sender| {
                            let value = fields
                                .directory
                                .as_ref()
                                .map_or("".to_string(), |path| path.display().to_string());

                            match picker_field(tui, &directory_label, &value) {
                                Some(PickerFieldAction::Clear) => sender
                                    .send(VariantBuilderTabUiCommand::ClearDirectoryClicked)
                                    .expect("sent"),
                                Some(PickerFieldAction::Pick) => sender
                                    .send(VariantBuilderTabUiCommand::PickDirectoryClicked)
                                    .expect("sent"),
                                None => {}
                            }
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
                        .send(VariantBuilderTabUiCommand::Submit);
                }
            });
    }

    /// `view` is what the variant builder's file says, once it's read, and `status` what the core is doing with it,
    /// and how its last event went (`VARB-7`).
    fn show_variant_builder(
        &self,
        ui: &mut Ui,
        path: &Path,
        view: Option<VariantBuilderView>,
        status: VariantBuilderStatus,
    ) {
        egui::Frame::new()
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.label(tr!("variant-builder-path", { path: path.display().to_string() }));

                if let Some(error) = &status.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                let Some(view) = view else {
                    if status.busy {
                        ui.label(tr!("variant-builder-loading"));
                    }
                    return;
                };

                ui.separator();

                let file = self.shown(&view);
                egui::Grid::new(ui.id().with("variant_builder"))
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| self.show_fields(ui, &file));

                ui.separator();

                let unsaved = file != view.file;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(unsaved && !status.busy, Button::new(tr!("variant-builder-button-save")))
                        .clicked()
                    {
                        self.component
                            .send(VariantBuilderTabUiCommand::SaveClicked);
                    }
                    // `VBGUI-4`, a variant is built from what its file says, so only once the changes are saved.
                    if ui
                        .add_enabled(
                            !unsaved && !status.busy,
                            Button::new(tr!("variant-builder-button-build")),
                        )
                        .clicked()
                    {
                        self.component
                            .send(VariantBuilderTabUiCommand::BuildClicked);
                    }
                    if status.busy {
                        ui.spinner();
                    } else if status.built {
                        ui.label(tr!("variant-builder-built"));
                    }
                });
            });
    }

    fn show_fields(&self, ui: &mut Ui, file: &VariantBuilderFile) {
        let send = |command| self.component.send(command);

        let name_label = tr!("variant-builder-input-variant-name");
        ui.label(&name_label);
        let mut name = file.variant.name.clone();
        if TextEdit::singleline(&mut name)
            .ui(ui)
            .named(name_label)
            .changed()
        {
            send(VariantBuilderTabUiCommand::Changed(VariantBuilderChange::VariantName(
                name,
            )));
        }
        ui.end_row();

        self.show_list(
            ui,
            file,
            ListField::RefDesList,
            tr!("variant-builder-input-ref-des-list"),
        );

        let eda_label = tr!("variant-builder-input-eda-tool");
        ui.label(&eda_label);
        let eda_tool_label = |eda_tool: Option<EdaTool>| match eda_tool {
            None => tr!("variant-builder-eda-tool-none"),
            Some(EdaTool::DipTrace) => "DipTrace".to_string(),
            Some(EdaTool::KiCad) => "KiCad".to_string(),
            Some(EdaTool::EasyEda) => "EasyEDA".to_string(),
        };
        egui::ComboBox::from_id_salt(ui.id().with("eda_tool"))
            .selected_text(eda_tool_label(file.eda_tool))
            .show_ui(ui, |ui| {
                for eda_tool in [
                    None,
                    Some(EdaTool::DipTrace),
                    Some(EdaTool::KiCad),
                    Some(EdaTool::EasyEda),
                ] {
                    if ui
                        .add(Button::selectable(file.eda_tool == eda_tool, eda_tool_label(eda_tool)))
                        .clicked()
                    {
                        send(VariantBuilderTabUiCommand::Changed(VariantBuilderChange::EdaTool(
                            eda_tool,
                        )));
                    }
                }
            })
            .response
            .named(eda_label);
        ui.end_row();

        for (field, label, value) in [
            (
                FileField::Placements,
                tr!("variant-builder-input-placements"),
                &file.placements,
            ),
            (FileField::Parts, tr!("variant-builder-input-parts"), &file.parts),
            (
                FileField::PartMappings,
                tr!("variant-builder-input-part-mappings"),
                &file.part_mappings,
            ),
            (
                FileField::LoadOut,
                tr!("variant-builder-input-load-out"),
                &file.load_out,
            ),
            (
                FileField::AssemblyRules,
                tr!("variant-builder-input-assembly-rules"),
                &file.assembly_rules,
            ),
        ] {
            let value = value
                .as_ref()
                .map_or("".to_string(), |path| path.display().to_string());
            self.show_file(ui, field, label, value, tr!("form-common-button-choose"));
        }

        let substitutions = file
            .substitutions
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        self.show_file(
            ui,
            FileField::Substitutions,
            tr!("variant-builder-input-substitutions"),
            substitutions,
            tr!("variant-builder-button-add"),
        );

        self.show_list(
            ui,
            file,
            ListField::RefDesExcludeList,
            tr!("variant-builder-input-ref-des-exclude-list"),
        );
        self.show_list(
            ui,
            file,
            ListField::RefDesDisableList,
            tr!("variant-builder-input-ref-des-disable-list"),
        );

        for (label, value, change) in [
            (
                tr!("variant-builder-input-output-placements"),
                &file.output_placements,
                VariantBuilderChange::OutputPlacements as fn(Option<PathBuf>) -> VariantBuilderChange,
            ),
            (
                tr!("variant-builder-input-output-bom"),
                &file.output_bom,
                VariantBuilderChange::OutputBom,
            ),
        ] {
            ui.label(&label);
            let mut text = value
                .as_ref()
                .map_or("".to_string(), |path| path.display().to_string());
            if TextEdit::singleline(&mut text)
                .hint_text(tr!("variant-builder-output-default"))
                .ui(ui)
                .named(label)
                .changed()
            {
                let path = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
                send(VariantBuilderTabUiCommand::Changed(change(path)));
            }
            ui.end_row();
        }
    }

    /// A file field: the file, or files, shown, and buttons named after the field to clear and to choose them.
    fn show_file(&self, ui: &mut Ui, field: FileField, label: String, value: String, choose: String) {
        ui.label(&label);
        ui.horizontal(|ui| {
            let mut value = value;
            TextEdit::singleline(&mut value)
                .interactive(false)
                .ui(ui)
                .named(label.clone());
            if Button::new("x")
                .ui(ui)
                .named(format!("{}: {}", label, tr!("form-common-button-clear")))
                .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::ClearClicked(field));
            }
            if Button::new("...")
                .ui(ui)
                .named(format!("{}: {}", label, choose))
                .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::PickClicked(field));
            }
        });
        ui.end_row();
    }

    /// A ref-des list, typed separated by commas.
    fn show_list(&self, ui: &mut Ui, file: &VariantBuilderFile, field: ListField, label: String) {
        ui.label(&label);
        let list = match field {
            ListField::RefDesList => &file.variant.ref_des_list,
            ListField::RefDesExcludeList => &file.ref_des_exclude_list,
            ListField::RefDesDisableList => &file.ref_des_disable_list,
        };
        let mut text = self
            .list_texts
            .get(&field)
            .cloned()
            .unwrap_or_else(|| list.join(", "));
        if TextEdit::singleline(&mut text)
            .ui(ui)
            .named(label)
            .changed()
        {
            self.component
                .send(VariantBuilderTabUiCommand::ListChanged(field, text));
        }
        ui.end_row();
    }
}

#[derive(Clone, Debug, Default, Validate, Serialize, Deserialize)]
struct NewVariantBuilderFields {
    #[validate(length(min = 1, code = "form-input-error-length"))]
    name: String,

    #[validate(required(code = "form-option-error-required"))]
    directory: Option<PathBuf>,
}
