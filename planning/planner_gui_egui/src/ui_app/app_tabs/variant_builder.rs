//! A design variant's tab, which shows a variant builder: for a new one, first the form to choose its EDA tool and
//! placements file, and name its design and variant, then the variant builder, whose data sources are chosen, in
//! groups, then saved, and built (`VBGUI`).

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
use variantbuilder_app::variant_builders::{
    DEFAULT_VARIANT_NAME, design_name, file_name, placements_extensions, stored_path,
};
use variantbuilder_app::{
    EdaTool, Event, NewVariantBuilder, VariantBuilderChange, VariantBuilderFile, VariantBuilderStatus,
    VariantBuilderView,
};

use crate::file_picker::{FileDialogs, FileFilter, Picker, SaveFile};
use crate::forms::{Form, NamedResponse, PickerFieldAction, picker_field};
use crate::sync::Value;
use crate::tabs::{Tab, TabKey};
use crate::task::Task;
use crate::ui_component::{ComponentState, UiComponent};
use crate::variant_builder_core::VariantBuilderCoreService;

/// A field of a variant builder whose value is a file the user chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileField {
    Parts,
    PartMappings,
    AssemblyRules,
}

/// A field of a variant builder whose value is a list of files, to which those chosen are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FilesField {
    GlobalSubstitutions,
    DesignSubstitutions,
}

/// A field of a variant builder that's a list of ref-des, typed separated by commas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ListField {
    RefDesList,
    RefDesExclusionList,
    RefDesNoPlaceList,
}

/// What a file dialog the tab opened is choosing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Picking {
    /// The new variant builder's placements.
    Placements,
    File(FileField),
    Files(FilesField),
    /// A new assembly rules file, to create.
    NewAssemblyRules,
}

/// The tab is persisted, so that it can be shown again when the application restarts.
#[derive(Default, Deserialize, Serialize, Derivative)]
#[derivative(Debug)]
pub struct VariantBuilderTab {
    /// The new variant builder's EDA tool, placements, and names, until "Ok" creates it.
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

    /// What the file dialog that's open is choosing.
    #[serde(skip)]
    picking: Option<Picking>,

    #[serde(skip)]
    pub component: ComponentState<VariantBuilderTabUiCommand>,

    #[serde(skip)]
    file_picker: Value<Picker>,
}

#[derive(Debug, Clone)]
pub enum VariantBuilderTabUiCommand {
    EdaToolChanged(Option<EdaTool>),
    ClearPlacementsClicked,
    PickPlacementsClicked,
    PlacementsPicked(PathBuf),
    DesignNameChanged(String),
    VariantNameChanged(String),
    Submit,

    Changed(VariantBuilderChange),
    ListChanged(ListField, String),
    ClearClicked(FileField),
    PickClicked(FileField),
    FilePicked(FileField, PathBuf),
    AddClicked(FilesField),
    FilesPicked(FilesField, Vec<PathBuf>),
    RemoveClicked(FilesField, usize),
    /// Moves the file at the index one place up the list, before the one before it.
    MoveUpClicked(FilesField, usize),
    /// Moves the file at the index one place down the list, after the one after it.
    MoveDownClicked(FilesField, usize),
    NewAssemblyRulesClicked,
    NewAssemblyRulesPicked(PathBuf),
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

    /// The files of the field, as shown, with the changes to be saved.
    fn shown_files(&self, variant_builders: &Value<VariantBuilderCoreService>, field: FilesField) -> Vec<PathBuf> {
        let (view, _) = self.known(variant_builders);
        view.map(|view| files(&self.shown(&view), field).clone())
            .unwrap_or_default()
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
            .unwrap_or_else(|| tr!("tab-label-new-design-variant"));

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
        let command = match self.picking {
            None => None,
            Some(Picking::Files(field)) => file_picker
                .picked_multi()
                .ok()
                .map(|picked| VariantBuilderTabUiCommand::FilesPicked(field, picked)),
            Some(picking) => file_picker
                .picked()
                .ok()
                .map(|picked| match picking {
                    Picking::Placements => VariantBuilderTabUiCommand::PlacementsPicked(picked),
                    Picking::File(field) => VariantBuilderTabUiCommand::FilePicked(field, picked),
                    Picking::NewAssemblyRules => VariantBuilderTabUiCommand::NewAssemblyRulesPicked(picked),
                    Picking::Files(_) => unreachable!(),
                }),
        };
        drop(file_picker);
        if let Some(command) = command {
            self.component.send(command);
        }

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
            VariantBuilderTabUiCommand::EdaToolChanged(eda_tool) => {
                let mut fields = self.fields.lock().unwrap();
                // `VBGUI-1`, the placements are chosen from the EDA tool's files, so they're chosen again.
                if fields.eda_tool != eda_tool {
                    fields.placements = None;
                }
                fields.eda_tool = eda_tool;
                None
            }
            VariantBuilderTabUiCommand::ClearPlacementsClicked => {
                self.fields.lock().unwrap().placements = None;
                None
            }
            VariantBuilderTabUiCommand::PickPlacementsClicked => {
                let Some(eda_tool) = self.fields.lock().unwrap().eda_tool else {
                    // the button is disabled until there's an EDA tool.
                    return None;
                };
                self.picking = Some(Picking::Placements);
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file_of(vec![FileFilter {
                        name: tr!("variant-builder-filter-placements"),
                        extensions: placements_extensions(eda_tool)
                            .iter()
                            .map(|extension| extension.to_string())
                            .collect(),
                    }]);
                None
            }
            VariantBuilderTabUiCommand::PlacementsPicked(placements) => {
                self.picking = None;
                let mut fields = self.fields.lock().unwrap();
                // `VBGUI-1`, the design is named after the placements, unless the user has named it.
                let named = fields
                    .placements
                    .as_deref()
                    .map(design_name)
                    .unwrap_or_default();
                if fields.design_name == named {
                    fields.design_name = design_name(&placements);
                }
                fields.placements = Some(placements);
                None
            }
            VariantBuilderTabUiCommand::DesignNameChanged(name) => {
                self.fields.lock().unwrap().design_name = name;
                None
            }
            VariantBuilderTabUiCommand::VariantNameChanged(name) => {
                self.fields.lock().unwrap().variant_name = name;
                None
            }
            VariantBuilderTabUiCommand::Submit => {
                let fields = self.fields.lock().unwrap();
                let (Some(eda_tool), Some(placements)) = (fields.eda_tool, &fields.placements) else {
                    // "Ok" is disabled until the form is valid.
                    return None;
                };
                let (design_name, variant_name) = (fields.design_name.trim(), fields.variant_name.trim());
                // `VBGUI-1`, `VARB-2`, in the placements' folder.
                let path = placements
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
                    .join(file_name(design_name, variant_name));
                let new = NewVariantBuilder {
                    design_name: Some(design_name.to_string()),
                    variant_name: Some(variant_name.to_string()),
                    changes: vec![
                        VariantBuilderChange::EdaTool(Some(eda_tool)),
                        VariantBuilderChange::Placements(Some(stored_path(&path, placements))),
                    ],
                };
                drop(fields);

                self.path = Some(path);
                // `VBGUI-1`, an existing file isn't replaced.
                self.process(&context.variant_builders, |path| Event::Create {
                    path,
                    overwrite: false,
                    new,
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
                    ListField::RefDesExclusionList => VariantBuilderChange::RefDesExclusionList(list),
                    ListField::RefDesNoPlaceList => VariantBuilderChange::RefDesNoPlaceList(list),
                });
                None
            }
            VariantBuilderTabUiCommand::ClearClicked(field) => {
                self.change(file_change(field, None));
                None
            }
            VariantBuilderTabUiCommand::PickClicked(field) => {
                self.picking = Some(Picking::File(field));
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_file_of(csv_filters());
                None
            }
            VariantBuilderTabUiCommand::FilePicked(field, path) => {
                self.picking = None;
                self.change(file_change(field, Some(self.stored(&path))));
                None
            }
            VariantBuilderTabUiCommand::AddClicked(field) => {
                self.picking = Some(Picking::Files(field));
                self.file_picker
                    .lock()
                    .unwrap()
                    .pick_files_of(csv_filters());
                None
            }
            VariantBuilderTabUiCommand::FilesPicked(field, paths) => {
                self.picking = None;
                // added to those there are.
                let mut files = self.shown_files(&context.variant_builders, field);
                files.extend(
                    paths
                        .iter()
                        .map(|path| self.stored(path)),
                );
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::RemoveClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index < files.len() {
                    files.remove(index);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::MoveUpClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index > 0 && index < files.len() {
                    files.swap(index - 1, index);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::MoveDownClicked(field, index) => {
                let mut files = self.shown_files(&context.variant_builders, field);
                if index + 1 < files.len() {
                    files.swap(index, index + 1);
                }
                self.change(files_change(field, files));
                None
            }
            VariantBuilderTabUiCommand::NewAssemblyRulesClicked => {
                let (Some(path), (Some(view), _)) = (&self.path, self.known(&context.variant_builders)) else {
                    return None;
                };
                self.picking = Some(Picking::NewAssemblyRules);
                // `VBGUI-6`, named after the variant builder, in its folder.
                let file_name = format!(
                    "{}_{}_assembly_rules.csv",
                    view.file.design_name, view.file.variant_name
                );
                self.file_picker
                    .lock()
                    .unwrap()
                    .save_file(SaveFile {
                        directory: path.parent().map(Path::to_path_buf),
                        file_name,
                        filters: csv_filters(),
                    });
                None
            }
            VariantBuilderTabUiCommand::NewAssemblyRulesPicked(assembly_rules) => {
                self.picking = None;
                // `VBGUI-6`, the core sets the assembly rules, so a change to them not yet saved is replaced.
                self.pending
                    .retain(|change| !matches!(change, VariantBuilderChange::AssemblyRules(_)));
                let assembly_rules = self.stored(&assembly_rules);
                self.process(&context.variant_builders, |path| Event::CreateAssemblyRules {
                    path,
                    assembly_rules,
                });
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

/// The change that sets the field to the file, or none.
fn file_change(field: FileField, path: Option<PathBuf>) -> VariantBuilderChange {
    match field {
        FileField::Parts => VariantBuilderChange::Parts(path),
        FileField::PartMappings => VariantBuilderChange::PartMappings(path),
        FileField::AssemblyRules => VariantBuilderChange::AssemblyRules(path),
    }
}

/// The change that sets the field to the files.
fn files_change(field: FilesField, paths: Vec<PathBuf>) -> VariantBuilderChange {
    match field {
        FilesField::GlobalSubstitutions => VariantBuilderChange::GlobalSubstitutions(paths),
        FilesField::DesignSubstitutions => VariantBuilderChange::DesignSubstitutions(paths),
    }
}

/// The files of the field, as the variant builder's file says.
fn files(file: &VariantBuilderFile, field: FilesField) -> &Vec<PathBuf> {
    match field {
        FilesField::GlobalSubstitutions => &file.global_substitutions,
        FilesField::DesignSubstitutions => &file.design_substitutions,
    }
}

/// The data sources are CSV files.
fn csv_filters() -> Vec<FileFilter> {
    vec![FileFilter {
        name: tr!("variant-builder-filter-csv"),
        extensions: vec!["csv".to_string()],
    }]
}

impl VariantBuilderTab {
    fn show_form(&self, ui: &mut Ui, form: &Form<NewVariantBuilderFields, VariantBuilderTabUiCommand>) {
        let default_style = || Style {
            padding: length(2.),
            gap: length(2.),
            ..Default::default()
        };

        let has_eda_tool = self
            .fields
            .lock()
            .unwrap()
            .eda_tool
            .is_some();

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
                    form.add_field_ui("eda_tool", tr!("variant-builder-input-eda-tool"), tui, {
                        move |ui: &mut Ui, fields, sender| {
                            egui::ComboBox::from_id_salt(ui.id().with("eda_tool"))
                                .width(ui.available_width())
                                .selected_text(eda_tool_label(fields.eda_tool))
                                .show_ui(ui, |ui| {
                                    for eda_tool in [EdaTool::DipTrace, EdaTool::KiCad, EdaTool::EasyEda] {
                                        if ui
                                            .add(Button::selectable(
                                                fields.eda_tool == Some(eda_tool),
                                                eda_tool_label(Some(eda_tool)),
                                            ))
                                            .clicked()
                                        {
                                            sender
                                                .send(VariantBuilderTabUiCommand::EdaToolChanged(Some(eda_tool)))
                                                .expect("sent");
                                        }
                                    }
                                })
                                .response
                        }
                    });

                    // `VBGUI-1`, the placements are chosen from the EDA tool's files, so once it's chosen.
                    if has_eda_tool {
                        let placements_label = tr!("variant-builder-input-placements");
                        form.add_field_tui("placements", placements_label.clone(), tui, {
                            move |tui: &mut Tui, fields, sender| {
                                let value = fields
                                    .placements
                                    .as_ref()
                                    .map_or("".to_string(), |path| path.display().to_string());

                                match picker_field(tui, &placements_label, &value) {
                                    Some(PickerFieldAction::Clear) => sender
                                        .send(VariantBuilderTabUiCommand::ClearPlacementsClicked)
                                        .expect("sent"),
                                    Some(PickerFieldAction::Pick) => sender
                                        .send(VariantBuilderTabUiCommand::PickPlacementsClicked)
                                        .expect("sent"),
                                    None => {}
                                }
                            }
                        });
                    }

                    for (field_name, label, command) in [
                        (
                            "design_name",
                            tr!("variant-builder-input-design-name"),
                            VariantBuilderTabUiCommand::DesignNameChanged as fn(String) -> VariantBuilderTabUiCommand,
                        ),
                        (
                            "variant_name",
                            tr!("variant-builder-input-variant-name"),
                            VariantBuilderTabUiCommand::VariantNameChanged,
                        ),
                    ] {
                        form.add_field_ui(field_name, label, tui, {
                            move |ui: &mut Ui, fields, sender| {
                                let value = match field_name {
                                    "design_name" => &fields.design_name,
                                    _ => &fields.variant_name,
                                };
                                let mut text = value.clone();
                                let output = TextEdit::singleline(&mut text)
                                    .desired_width(ui.available_width())
                                    .show(ui);

                                if *value != text {
                                    sender
                                        .send(command(text))
                                        .expect("sent")
                                }

                                output.response.response
                            }
                        });
                    }
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
                self.show_fields(ui, &file);

                ui.separator();

                show_substitutions_applied(ui, &file);

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

    /// The variant builder's fields: what it's of, which can't be changed, then the rest in groups (`VBGUI-2`).
    fn show_fields(&self, ui: &mut Ui, file: &VariantBuilderFile) {
        let send = |command| self.component.send(command);

        let grid = |ui: &mut Ui, id: &str, add_contents: &mut dyn FnMut(&mut Ui)| {
            egui::Grid::new(ui.id().with(id))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| add_contents(ui));
        };
        let group = |ui: &mut Ui, id: &str, title: String, add_contents: &mut dyn FnMut(&mut Ui)| {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong(title);
                grid(ui, id, add_contents);
            });
        };

        // `VBGUI-1`, set as it's created, and kept as they are.
        grid(ui, "variant_builder_of", &mut |ui| {
            let placements = file
                .placements
                .as_ref()
                .map_or("".to_string(), |path| path.display().to_string());
            for (label, value) in [
                (tr!("variant-builder-input-eda-tool"), eda_tool_label(file.eda_tool)),
                (tr!("variant-builder-input-placements"), placements),
                (tr!("variant-builder-input-design-name"), file.design_name.clone()),
                (tr!("variant-builder-input-variant-name"), file.variant_name.clone()),
            ] {
                show_read_only(ui, label, value);
            }
        });

        group(
            ui,
            "part_matching",
            tr!("variant-builder-group-part-matching"),
            &mut |ui| {
                for (field, label, value) in [
                    (FileField::Parts, tr!("variant-builder-input-parts"), &file.parts),
                    (
                        FileField::PartMappings,
                        tr!("variant-builder-input-part-mappings"),
                        &file.part_mappings,
                    ),
                ] {
                    self.show_file(ui, field, label, value, false);
                }
                self.show_files(
                    ui,
                    file,
                    FilesField::GlobalSubstitutions,
                    tr!("variant-builder-input-global-substitutions"),
                );
            },
        );

        group(
            ui,
            "design_specific",
            tr!("variant-builder-group-design-specific"),
            &mut |ui| {
                self.show_file(
                    ui,
                    FileField::AssemblyRules,
                    tr!("variant-builder-input-assembly-rules"),
                    &file.assembly_rules,
                    true,
                );
                self.show_files(
                    ui,
                    file,
                    FilesField::DesignSubstitutions,
                    tr!("variant-builder-input-design-substitutions"),
                );
            },
        );

        group(
            ui,
            "reference_designators",
            tr!("variant-builder-group-reference-designators"),
            &mut |ui| {
                for (field, label) in [
                    (ListField::RefDesList, tr!("variant-builder-input-ref-des-list")),
                    (
                        ListField::RefDesExclusionList,
                        tr!("variant-builder-input-ref-des-exclusion-list"),
                    ),
                    (
                        ListField::RefDesNoPlaceList,
                        tr!("variant-builder-input-ref-des-no-place-list"),
                    ),
                ] {
                    self.show_list(ui, file, field, label);
                }
            },
        );

        group(ui, "output", tr!("variant-builder-group-output"), &mut |ui| {
            self.show_outputs(ui, file, &send)
        });
    }

    /// The outputs' paths, typed, relative to the variant builder's folder, or its defaults if they're empty.
    fn show_outputs(&self, ui: &mut Ui, file: &VariantBuilderFile, send: &dyn Fn(VariantBuilderTabUiCommand)) {
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

    /// A file field: the file shown, and buttons named after the field to clear it and to choose it, and, if `new`, to
    /// create a new one (`VBGUI-6`).
    fn show_file(&self, ui: &mut Ui, field: FileField, label: String, value: &Option<PathBuf>, new: bool) {
        ui.label(&label);
        ui.horizontal(|ui| {
            let mut value = value
                .as_ref()
                .map_or("".to_string(), |path| path.display().to_string());
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
                .named(format!("{}: {}", label, tr!("form-common-button-choose")))
                .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::PickClicked(field));
            }
            if new
                && Button::new(tr!("variant-builder-button-new"))
                    .ui(ui)
                    .named(format!("{}: {}", label, tr!("variant-builder-button-new")))
                    .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::NewAssemblyRulesClicked);
            }
        });
        ui.end_row();
    }

    /// A list of files: each file, with buttons to move it up and down the list, and to remove it, named after the field
    /// and the file, then a button to add files (`VBGUI-2`).
    fn show_files(&self, ui: &mut Ui, file: &VariantBuilderFile, field: FilesField, label: String) {
        ui.label(&label);
        ui.vertical(|ui| {
            let paths = files(file, field);
            for (index, path) in paths.iter().enumerate() {
                ui.horizontal(|ui| {
                    let path = path.display().to_string();
                    ui.label(&path);
                    for (enabled, text, action, command) in [
                        (
                            index > 0,
                            "⏶",
                            tr!("variant-builder-button-move-up"),
                            VariantBuilderTabUiCommand::MoveUpClicked(field, index),
                        ),
                        (
                            index + 1 < paths.len(),
                            "⏷",
                            tr!("variant-builder-button-move-down"),
                            VariantBuilderTabUiCommand::MoveDownClicked(field, index),
                        ),
                    ] {
                        if ui
                            .add_enabled(enabled, Button::new(text))
                            .named(format!("{}: {} {}", label, action, path))
                            .clicked()
                        {
                            self.component.send(command);
                        }
                    }
                    if Button::new("x")
                        .ui(ui)
                        .named(format!("{}: {} {}", label, tr!("variant-builder-button-remove"), path))
                        .clicked()
                    {
                        self.component
                            .send(VariantBuilderTabUiCommand::RemoveClicked(field, index));
                    }
                });
            }
            if Button::new(tr!("variant-builder-button-add"))
                .ui(ui)
                .named(format!("{}: {}", label, tr!("variant-builder-button-add")))
                .clicked()
            {
                self.component
                    .send(VariantBuilderTabUiCommand::AddClicked(field));
            }
        });
        ui.end_row();
    }

    /// A ref-des list, typed separated by commas.
    fn show_list(&self, ui: &mut Ui, file: &VariantBuilderFile, field: ListField, label: String) {
        ui.label(&label);
        let list = match field {
            ListField::RefDesList => &file.ref_des_list,
            ListField::RefDesExclusionList => &file.ref_des_exclusion_list,
            ListField::RefDesNoPlaceList => &file.ref_des_no_place_list,
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

/// The substitutions, numbered, in the order the core applies them, with the changes not yet saved, so the user sees
/// the effect of the order of the design's and the global ones (`VBGUI-7`, `VARB-5`).
fn show_substitutions_applied(ui: &mut Ui, file: &VariantBuilderFile) {
    ui.strong(tr!("variant-builder-substitutions-applied"));
    let mut substitutions = file.substitutions().peekable();
    if substitutions.peek().is_none() {
        ui.label(tr!("variant-builder-substitutions-applied-none"));
    }
    for (index, path) in substitutions.enumerate() {
        ui.label(format!("{}. {}", index + 1, path.display()));
    }
    ui.separator();
}

/// A read-only field, named for assistive technology and tests.
fn show_read_only(ui: &mut Ui, label: String, value: String) {
    ui.label(&label);
    let mut value = value;
    TextEdit::singleline(&mut value)
        .interactive(false)
        .ui(ui)
        .named(label);
    ui.end_row();
}

fn eda_tool_label(eda_tool: Option<EdaTool>) -> String {
    match eda_tool {
        None => tr!("form-common-combo-select"),
        Some(EdaTool::DipTrace) => "DipTrace".to_string(),
        Some(EdaTool::KiCad) => "KiCad".to_string(),
        Some(EdaTool::EasyEda) => "EasyEDA".to_string(),
    }
}

/// A new design variant: the EDA tool, then its placements, then the names of the design and the variant, which name
/// the variant builder's file (`VBGUI-1`).
#[derive(Clone, Debug, Validate, Serialize, Deserialize)]
struct NewVariantBuilderFields {
    #[validate(required(code = "form-option-error-required"))]
    eda_tool: Option<EdaTool>,

    #[validate(required(code = "form-option-error-required"))]
    placements: Option<PathBuf>,

    #[validate(length(min = 1, code = "form-input-error-length"))]
    design_name: String,

    #[validate(length(min = 1, code = "form-input-error-length"))]
    variant_name: String,
}

impl Default for NewVariantBuilderFields {
    fn default() -> Self {
        Self {
            eda_tool: None,
            placements: None,
            design_name: String::new(),
            variant_name: DEFAULT_VARIANT_NAME.to_string(),
        }
    }
}
