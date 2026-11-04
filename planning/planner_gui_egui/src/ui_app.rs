use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::epaint::Color32;
use egui::{CentralPanel, Frame, ThemePreference};
use egui_i18n::tr;
use slotmap::SlotMap;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::file_picker::{FileDialogs, NativeFileDialogs, PickError, Picker};
use crate::pcb::tabs::PcbTabs;
use crate::pcb::{Pcb, PcbKey, PcbUiCommand};
use crate::process::{Process, ProcessKey, ProcessUiCommand};
use crate::project::tabs::ProjectTabs;
use crate::project::{Project, ProjectKey, ProjectUiCommand};
use crate::sync::{Enqueue, Queue, Value, ValueGuard};
use crate::tabs::TabKey;
use crate::toolbar::{Toolbar, ToolbarContext, ToolbarUiCommand};
use crate::ui_app::app_tabs::new_pcb::NewPcbArgs;
use crate::ui_app::app_tabs::new_process::NewProcessArgs;
use crate::ui_app::app_tabs::new_project::NewProjectArgs;
use crate::ui_app::app_tabs::pcb::{PcbTab, PcbTabUiCommand};
use crate::ui_app::app_tabs::process::{ProcessTab, ProcessTabUiCommand};
use crate::ui_app::app_tabs::project::{ProjectTab, ProjectTabUiCommand};
use crate::ui_app::app_tabs::{AppTabs, TabKind, TabKindContext, TabKindUiCommand, TabUiCommand};
use crate::ui_commands::{UiCommand, handle_command};
use crate::ui_component::{ComponentState, UiComponent};
use crate::{fonts, pcb, project, task};

pub mod app_tabs;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct UiApp {
    app_tabs: Value<AppTabs>,

    config: Value<Config>,

    // state contains fields that cannot be initialized using 'Default'
    #[serde(skip)]
    state: MaybeUninit<Value<AppState>>,

    /// the commands to handle, in the order they were sent (`GARCH-11`).
    #[serde(skip)]
    commands: Option<Queue<UiCommand>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickReason {
    PcbFile,
    ProcessFile,
    ProjectFile,
}

impl PickReason {
    pub fn file_filter(self) -> &'static str {
        match self {
            PickReason::ProjectFile => "*.project.json",
            PickReason::PcbFile => "*.pcb.json",
            PickReason::ProcessFile => "*.process.json",
        }
    }
}

pub struct AppState {
    // TODO find a better way of doing this that doesn't require this boolean
    startup_done: bool,
    file_picker: Option<(
        PickReason,
        Picker,
        Box<dyn Fn(PathBuf) -> UiCommand + Send + Sync + 'static>,
    )>,
    file_dialogs: Arc<dyn FileDialogs>,

    command_sender: Enqueue<UiCommand>,

    pub projects: Value<SlotMap<ProjectKey, Project>>,

    pub toolbar: Toolbar,
    pub pcbs: Value<SlotMap<PcbKey, Pcb>>,
    pub processes: Value<SlotMap<ProcessKey, Process>>,
}

impl AppState {
    pub fn init(sender: Enqueue<UiCommand>, file_dialogs: Arc<dyn FileDialogs>) -> Self {
        let mut toolbar = Toolbar::new();
        toolbar
            .component
            .configure_mapper(sender.clone(), |command: ToolbarUiCommand| {
                UiCommand::ToolbarCommand(command)
            });

        Self {
            startup_done: false,
            file_picker: None,
            file_dialogs,

            command_sender: sender,
            projects: Value::new(SlotMap::default()),
            pcbs: Value::new(SlotMap::default()),
            processes: Value::new(SlotMap::default()),
            toolbar,
        }
    }

    pub fn file_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.file_dialogs.clone()
    }

    // FUTURE consider returning a result to indicate if the picker was busy
    fn pick_file(&mut self, reason: PickReason, command_fn: Box<dyn Fn(PathBuf) -> UiCommand + Send + Sync + 'static>) {
        // TODO use the filter, picker API needs updating
        let _filter = reason.file_filter();

        match &mut self.file_picker {
            Some(_) => {
                error!("file picker busy, not picking a {:?}", reason);
            }
            None => {
                let mut picker = Picker::new(self.file_dialogs.clone());
                picker.pick_file();
                self.file_picker = Some((reason, picker, command_fn));
            }
        }
    }

    pub fn pick_project_file(&mut self) {
        let open_project_file_command_fn = |path: PathBuf| UiCommand::OpenProjectFile(path);

        self.pick_file(PickReason::ProjectFile, Box::new(open_project_file_command_fn));
    }

    pub fn pick_pcb_file(&mut self) {
        let open_pcb_file_command_fn = |path: PathBuf| UiCommand::OpenPcbFile(path);
        self.pick_file(PickReason::PcbFile, Box::new(open_pcb_file_command_fn));
    }

    pub fn pick_process_file(&mut self) {
        let open_process_file_command_fn = |path: PathBuf| UiCommand::OpenProcessFile(path);
        self.pick_file(PickReason::ProcessFile, Box::new(open_process_file_command_fn));
    }

    pub fn make_project_tab(
        &mut self,
        path: PathBuf,
        project_key: ProjectKey,
        project_tabs: Value<ProjectTabs>,
    ) -> (TabKind, ProjectKey) {
        info!("Open project. path: {:?}", path);

        let label = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let tab_kind_component = ComponentState::default();
        let tab_kind_sender = tab_kind_component.sender.clone();

        let mut project_tab = ProjectTab::new(label, path, project_key, project_tabs);
        project_tab
            .component
            .configure_mapper(tab_kind_sender, |command| TabKindUiCommand::ProjectTabCommand {
                command,
            });

        let tab_kind = TabKind::Project(project_tab, tab_kind_component);

        (tab_kind, project_key)
    }

    pub fn send_project_command(
        command_sender: &mut Enqueue<UiCommand>,
        project_command: ProjectUiCommand,
        project_key: ProjectKey,
        tab_key: TabKey,
    ) {
        command_sender
            .send(UiCommand::TabCommand {
                tab_key,
                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                    command: ProjectTabUiCommand::ProjectCommand {
                        key: project_key,
                        command: project_command,
                    },
                }),
            })
            .expect("sent");
    }

    pub fn configure_project_tab(&mut self, project_key: ProjectKey, tab_key: TabKey, commands: Vec<ProjectUiCommand>) {
        let mut projects = self.projects.lock().unwrap();
        let mut project = projects.get_mut(project_key).unwrap();

        let app_command_sender = self.command_sender.clone();
        configure_project_component(app_command_sender, tab_key.clone(), &mut project);

        for command in commands {
            Self::send_project_command(&mut self.command_sender, command, project_key, tab_key);
        }
    }

    pub fn open_project_file(&mut self, path: PathBuf, app_tabs: Value<AppTabs>) {
        let (commands, project_key, project_tabs) = {
            let mut projects = self.projects.lock().unwrap();
            project_from_path(path.clone(), &mut projects, None, self.file_dialogs.clone())
        };

        let (tab_kind, project_key) = self.make_project_tab(path, project_key, project_tabs);

        let tab_key = app_tabs
            .lock()
            .unwrap()
            .add_tab(tab_kind);

        self.configure_project_tab(project_key, tab_key, commands);
    }

    /// `tab_key` - the tab key of the tab to replace, e.g. the 'NewProjectTab' instance's key.
    pub fn create_project(&mut self, tab_key: TabKey, args: NewProjectArgs, app_tabs: Value<AppTabs>) {
        debug!("Creating project. tab_key: {:?}, args: {:?}", tab_key, args);

        let (commands, project_key, project_tabs, path) = {
            let mut projects = self.projects.lock().unwrap();
            project_from_args(args, &mut projects, self.file_dialogs.clone())
        };

        let (tab_kind, project_key) = self.make_project_tab(path, project_key, project_tabs);

        app_tabs
            .lock()
            .unwrap()
            .replace(&tab_key, tab_kind)
            .expect("replaced");

        self.configure_project_tab(project_key, tab_key, commands);
    }

    //
    // pcb tab
    //

    pub fn send_pcb_command(
        command_sender: &mut Enqueue<UiCommand>,
        pcb_command: PcbUiCommand,
        pcb_key: PcbKey,
        tab_key: TabKey,
    ) {
        command_sender
            .send(UiCommand::TabCommand {
                tab_key,
                command: TabUiCommand::TabKindCommand(TabKindUiCommand::PcbTabCommand {
                    command: PcbTabUiCommand::PcbCommand {
                        key: pcb_key,
                        command: pcb_command,
                    },
                }),
            })
            .expect("sent");
    }

    pub fn make_pcb_tab(&mut self, path: PathBuf, pcb_key: PcbKey, pcb_tabs: Value<PcbTabs>) -> (TabKind, PcbKey) {
        info!("Open pcb. path: {:?}", path);

        let label = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let tab_kind_component = ComponentState::default();
        let tab_kind_sender = tab_kind_component.sender.clone();

        let mut pcb_tab = PcbTab::new(label, path, pcb_key, pcb_tabs);
        pcb_tab
            .component
            .configure_mapper(tab_kind_sender, |command| TabKindUiCommand::PcbTabCommand {
                command,
            });

        let tab_kind = TabKind::Pcb(pcb_tab, tab_kind_component);

        (tab_kind, pcb_key)
    }

    pub fn configure_pcb_tab(&mut self, pcb_key: PcbKey, tab_key: TabKey, commands: Vec<PcbUiCommand>) {
        let mut pcbs = self.pcbs.lock().unwrap();
        let mut pcb = pcbs.get_mut(pcb_key).unwrap();

        let app_command_sender = self.command_sender.clone();
        configure_pcb_component(app_command_sender, tab_key.clone(), &mut pcb);

        for command in commands {
            Self::send_pcb_command(&mut self.command_sender, command, pcb_key, tab_key);
        }
    }

    pub fn open_pcb_file(&mut self, path: PathBuf, app_tabs: Value<AppTabs>) {
        let (commands, pcb_key, pcb_tabs) = {
            let mut pcbs = self.pcbs.lock().unwrap();
            pcb_from_path(path.clone(), &mut pcbs, None)
        };

        let (tab_kind, pcb_key) = self.make_pcb_tab(path, pcb_key, pcb_tabs);

        let tab_key = app_tabs
            .lock()
            .unwrap()
            .add_tab(tab_kind);

        self.configure_pcb_tab(pcb_key, tab_key, commands);
    }

    pub fn create_pcb(&mut self, tab_key: TabKey, args: NewPcbArgs, app_tabs: Value<AppTabs>) {
        debug!("Creating pcb. tab_key: {:?}, args: {:?}", tab_key, args);

        let (commands, pcb_key, pcb_tabs, path) = {
            let mut pcbs = self.pcbs.lock().unwrap();
            pcb_from_args(args, &mut pcbs)
        };

        let (tab_kind, pcb_key) = self.make_pcb_tab(path, pcb_key, pcb_tabs);

        app_tabs
            .lock()
            .unwrap()
            .replace(&tab_key, tab_kind)
            .expect("replaced");

        self.configure_pcb_tab(pcb_key, tab_key, commands);
    }

    //
    // process tab
    //

    pub fn send_process_command(
        command_sender: &mut Enqueue<UiCommand>,
        process_command: ProcessUiCommand,
        process_key: ProcessKey,
        tab_key: TabKey,
    ) {
        command_sender
            .send(UiCommand::TabCommand {
                tab_key,
                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProcessTabCommand {
                    command: ProcessTabUiCommand::ProcessCommand {
                        key: process_key,
                        command: process_command,
                    },
                }),
            })
            .expect("sent");
    }

    pub fn make_process_tab(&mut self, path: PathBuf, process_key: ProcessKey) -> TabKind {
        info!("Open process. path: {:?}", path);

        let label = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let tab_kind_component = ComponentState::default();
        let tab_kind_sender = tab_kind_component.sender.clone();

        let mut process_tab = ProcessTab::new(label, path, process_key);
        process_tab
            .component
            .configure_mapper(tab_kind_sender, |command| TabKindUiCommand::ProcessTabCommand {
                command,
            });

        TabKind::Process(process_tab, tab_kind_component)
    }

    pub fn configure_process_tab(&mut self, process_key: ProcessKey, tab_key: TabKey, commands: Vec<ProcessUiCommand>) {
        let mut processes = self.processes.lock().unwrap();
        let process = processes.get_mut(process_key).unwrap();

        let app_command_sender = self.command_sender.clone();
        configure_process_component(app_command_sender, tab_key, process);

        for command in commands {
            Self::send_process_command(&mut self.command_sender, command, process_key, tab_key);
        }
    }

    pub fn open_process_file(&mut self, path: PathBuf, app_tabs: Value<AppTabs>) {
        let (commands, process_key) = {
            let mut processes = self.processes.lock().unwrap();
            process_from_path(path.clone(), &mut processes)
        };

        let tab_kind = self.make_process_tab(path, process_key);

        let tab_key = app_tabs
            .lock()
            .unwrap()
            .add_tab(tab_kind);

        self.configure_process_tab(process_key, tab_key, commands);
    }

    /// `tab_key` - the tab key of the tab to replace, e.g. the 'NewProcessTab' instance's key.
    pub fn create_process(&mut self, tab_key: TabKey, args: NewProcessArgs, app_tabs: Value<AppTabs>) {
        debug!("Creating process. tab_key: {:?}, args: {:?}", tab_key, args);

        let path = args.build_path();
        let (commands, process_key) = {
            let mut processes = self.processes.lock().unwrap();
            let mut process_commands = None;
            let process_key = processes.insert_with_key(|key| {
                let (process, commands) = Process::new(path.clone(), key, args.reference, args.preset);
                process_commands.replace(commands);
                process
            });
            (process_commands.unwrap(), process_key)
        };

        let tab_kind = self.make_process_tab(path, process_key);

        app_tabs
            .lock()
            .unwrap()
            .replace(&tab_key, tab_kind)
            .expect("replaced");

        self.configure_process_tab(process_key, tab_key, commands);
    }
}

impl Default for UiApp {
    fn default() -> Self {
        Self {
            app_tabs: Default::default(),
            config: Default::default(),
            state: MaybeUninit::uninit(),
            commands: None,
        }
    }
}

impl UiApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_file_dialogs(cc, Arc::new(NativeFileDialogs))
    }

    /// As [`Self::new`], with the file dialogs to use, for example in tests.
    pub fn with_file_dialogs(cc: &eframe::CreationContext<'_>, file_dialogs: Arc<dyn FileDialogs>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        fonts::initialize(&cc.egui_ctx);

        // Set solid scrollbars for the entire app
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.scroll = egui::style::ScrollStyle::solid();
        });

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut instance = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        {
            let config = instance.config.lock().unwrap();
            egui_i18n::set_language(&config.language_identifier);

            // Safety: now safe to use i18n translation system (e.g. [`egui_i18n::tr!`])
        }

        let (app_message_sender, commands) = crate::sync::queue::<UiCommand>();
        instance.commands = Some(commands);

        let state = Value::new(AppState::init(app_message_sender.clone(), file_dialogs));

        instance.state.write(state.clone());
        // Safety: `Self::state()` is now safe to call.

        let app_tabs = instance.app_tabs.clone();

        let mut app_tabs = app_tabs.lock().unwrap();
        app_tabs
            .component
            .configure_mapper(app_message_sender, |(tab_key, command)| UiCommand::TabCommand {
                tab_key,
                command,
            });

        instance
    }

    /// The most commands handled in one frame, so that a command which always leads to another can't stop frames being
    /// drawn. Any left are handled in the next frame.
    const MAX_COMMANDS_PER_FRAME: usize = 1000;

    /// Handles the commands sent so far, one at a time, in the order they were sent, along with the commands that
    /// handling them leads to (`GARCH-11`).
    fn handle_commands(&mut self, context: &egui::Context) {
        let Some(commands) = self.commands.as_ref() else {
            return;
        };
        // Safety: it's always safe, because `new` calls `state.write()`
        let state = unsafe { self.state.assume_init_ref() }.clone();
        let sender = state
            .lock()
            .unwrap()
            .command_sender
            .clone();

        for _ in 0..Self::MAX_COMMANDS_PER_FRAME {
            let Some(command) = commands.pop() else {
                return;
            };

            let task = handle_command(
                command,
                state.clone(),
                self.app_tabs.clone(),
                self.config.clone(),
                context.clone(),
            );

            // follow-up commands are handled after those already sent.
            for command in task::run_to_completion(task) {
                sender.send(command).expect("sent");
            }
        }

        if !commands.is_empty() {
            context.request_repaint();
        }
    }

    /// Whether a file dialog is open, or its answer hasn't been handled yet.  Another can't be opened until it has.
    pub fn is_picking_file(&self) -> bool {
        // Safety: it's always safe, because `new` calls `state.write()`
        let state = unsafe { self.state.assume_init_ref() };

        state
            .lock()
            .unwrap()
            .file_picker
            .is_some()
    }

    /// provide mutable access to the state.
    ///
    /// Safety: it's always safe, because `new` calls `state.write()`
    fn app_state(&mut self) -> ValueGuard<'_, AppState> {
        unsafe {
            self.state
                .assume_init_mut()
                .lock()
                .unwrap()
        }
    }

    /// When the app starts up, the new project tab components won't be wired up
    /// so we just remove them for now until we have a component restoration system.
    fn remove_new_project_tabs_on_startup(&mut self) {
        let mut app_tabs = self.app_tabs.lock().unwrap();

        app_tabs.retain(|tab_key, tab_kind| {
            let should_retain = !matches!(tab_kind, TabKind::NewProject(_, _));
            if !should_retain {
                debug!("removing 'new project' tab, tab_key: {:?}", tab_key);
            }
            should_retain
        });
    }

    /// When the app starts up, the pcb tabs won't be wired up, and we cannot restore pcb tabs without a path
    /// so we just remove them for now until we have a component restoration system.
    fn remove_new_pcb_tabs_on_startup(&mut self) {
        let mut app_tabs = self.app_tabs.lock().unwrap();

        app_tabs.retain(|tab_key, tab_kind| {
            let should_retain = !matches!(tab_kind, TabKind::NewPcb(_, _));
            if !should_retain {
                debug!("removing 'new pcb' tab, tab_key: {:?}", tab_key);
            }
            should_retain
        });
    }

    /// when the app starts up, the projects and pcbs will be empty, and the project and pcbs tabs will have keys that
    /// don't exist in the projects and pcbs lists (because they are empty now).
    /// we have to find these tabs, create projects/pcbs instances, store them in the maps and replace the corresponding
    /// tab's map key with the new key generated when adding the key to the map
    ///
    /// Safety: call only once on startup, before the tabs are shown.
    fn restore_documents_on_startup(&mut self) {
        // we have to do this as a two-step process to above borrow-checker issues
        // we also have to limit the scope of the access to app_tabs and app_state

        #[derive(Debug)]
        enum Kind {
            Project(Value<ProjectTabs>),
            Pcb(Value<PcbTabs>),
            Process,
        }

        // step 1 - find the document tabs, return the tab keys and paths.
        let tab_keys_paths_and_tabs = {
            let app_tabs = self.app_tabs.lock().unwrap();

            app_tabs.filter_map(|(tab_key, tab_kind)| match tab_kind {
                TabKind::Project(project_tab, _) => Some((
                    Kind::Project(project_tab.project_tabs.clone()),
                    *tab_key,
                    project_tab.path.clone(),
                )),
                TabKind::Pcb(pcb_tab, _) => Some((Kind::Pcb(pcb_tab.pcb_tabs.clone()), *tab_key, pcb_tab.path.clone())),
                TabKind::Process(process_tab, _) => Some((Kind::Process, *tab_key, process_tab.path.clone())),
                _ => None,
            })
        };

        // step 2 - store the documents and update the document key for the tab.
        for (kind, tab_key, path) in tab_keys_paths_and_tabs {
            debug!(
                "Restoring document. kind: {:?}, tab_key: {:?}, path: {:?}",
                kind, tab_key, path
            );
            let app_command_sender = self.app_state().command_sender.clone();

            match kind {
                Kind::Project(persisted_tabs) => {
                    let (project_key, commands) = {
                        let app_state = self.app_state();
                        let mut projects = app_state.projects.lock().unwrap();

                        let (project_command, project_key, _new_project_tabs) =
                            project_from_path(path, &mut projects, Some(persisted_tabs), app_state.file_dialogs());

                        let project = projects.get_mut(project_key).unwrap();
                        configure_project_component(app_command_sender.clone(), tab_key, project);

                        (project_key, project_command)
                    };

                    {
                        let app_tabs = self.app_tabs.lock().unwrap();
                        app_tabs.with_tab_mut(&tab_key, |tab| {
                            if let TabKind::Project(project_tab, _) = tab {
                                project_tab.project_key = project_key;
                            } else {
                                unreachable!()
                            }
                        });
                    }

                    {
                        for command in commands {
                            app_command_sender
                                .send(UiCommand::TabCommand {
                                    tab_key,
                                    command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                                        command: ProjectTabUiCommand::ProjectCommand {
                                            key: project_key,
                                            command,
                                        },
                                    }),
                                })
                                .expect("sent");
                        }
                    }
                }
                Kind::Pcb(persisted_tabs) => {
                    let (pcb_key, commands) = {
                        let app_state = self.app_state();
                        let mut pcbs = app_state.pcbs.lock().unwrap();

                        let (pcb_command, pcb_key, _new_pcb_tabs) =
                            pcb_from_path(path, &mut pcbs, Some(persisted_tabs));

                        let pcb = pcbs.get_mut(pcb_key).unwrap();
                        configure_pcb_component(app_command_sender.clone(), tab_key, pcb);

                        (pcb_key, pcb_command)
                    };

                    {
                        let app_tabs = self.app_tabs.lock().unwrap();
                        app_tabs.with_tab_mut(&tab_key, |tab| {
                            if let TabKind::Pcb(pcb_tab, _) = tab {
                                pcb_tab.pcb_key = pcb_key;
                            } else {
                                unreachable!()
                            }
                        });
                    }

                    {
                        for command in commands {
                            app_command_sender
                                .send(UiCommand::TabCommand {
                                    tab_key,
                                    command: TabUiCommand::TabKindCommand(TabKindUiCommand::PcbTabCommand {
                                        command: PcbTabUiCommand::PcbCommand {
                                            key: pcb_key,
                                            command,
                                        },
                                    }),
                                })
                                .expect("sent");
                        }
                    }
                }
                Kind::Process => {
                    let (process_key, commands) = {
                        let app_state = self.app_state();
                        let mut processes = app_state.processes.lock().unwrap();

                        let (commands, process_key) = process_from_path(path, &mut processes);

                        let process = processes.get_mut(process_key).unwrap();
                        configure_process_component(app_command_sender.clone(), tab_key, process);

                        (process_key, commands)
                    };

                    {
                        let app_tabs = self.app_tabs.lock().unwrap();
                        app_tabs.with_tab_mut(&tab_key, |tab| {
                            if let TabKind::Process(process_tab, _) = tab {
                                process_tab.process_key = process_key;
                            } else {
                                unreachable!()
                            }
                        });
                    }

                    for command in commands {
                        app_command_sender
                            .send(UiCommand::TabCommand {
                                tab_key,
                                command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProcessTabCommand {
                                    command: ProcessTabUiCommand::ProcessCommand {
                                        key: process_key,
                                        command,
                                    },
                                }),
                            })
                            .expect("sent");
                    }
                }
            }
        }
    }
}

impl eframe::App for UiApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    #[profiling::function]
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_commands(&ctx);

        egui::Panel::top("top_panel").show(ui, |ui| {
            profiling::scope!("ui::top_panel");
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        // NOTE: no File->Quit on web pages!
                        let is_web = cfg!(target_arch = "wasm32");
                        if !is_web {
                            ui.menu_button(tr!("menu-top-level-file"), |ui| {
                                if ui
                                    .button(tr!("menu-item-quit"))
                                    .clicked()
                                {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                            ui.add_space(16.0);
                        }
                    },
                    |ui| {
                        let theme_preference = ctx.options(|opt| opt.theme_preference);

                        egui::ComboBox::from_id_salt(ui.id().with("theme"))
                            .selected_text({
                                match theme_preference {
                                    ThemePreference::Dark => tr!("theme-button-dark"),
                                    ThemePreference::Light => tr!("theme-button-light"),
                                    ThemePreference::System => tr!("theme-button-system"),
                                }
                            })
                            .show_ui(ui, |ui| {
                                let sender = self.app_state().command_sender.clone();

                                if ui
                                    .add(egui::Button::selectable(
                                        theme_preference.eq(&ThemePreference::Dark),
                                        tr!("theme-button-dark"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::Dark))
                                        .expect("sent");
                                }
                                if ui
                                    .add(egui::Button::selectable(
                                        theme_preference.eq(&ThemePreference::Light),
                                        tr!("theme-button-light"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::Light))
                                        .expect("sent");
                                }
                                if ui
                                    .add(egui::Button::selectable(
                                        theme_preference.eq(&ThemePreference::System),
                                        tr!("theme-button-system"),
                                    ))
                                    .clicked()
                                {
                                    sender
                                        .send(UiCommand::ThemeChanged(ThemePreference::System))
                                        .expect("sent");
                                }
                            });

                        let language = egui_i18n::get_language();
                        fn format_language_key(language_identifier: &String) -> String {
                            format!("language-{}", &language_identifier).to_string()
                        }

                        egui::ComboBox::from_id_salt(ui.id().with("language"))
                            .selected_text(tr!(&format_language_key(&language)))
                            .show_ui(ui, |ui| {
                                for other_language in egui_i18n::languages() {
                                    let sender = self.app_state().command_sender.clone();
                                    if ui
                                        .add(egui::Button::selectable(
                                            other_language.eq(&language),
                                            tr!(&format_language_key(&other_language)),
                                        ))
                                        .clicked()
                                    {
                                        sender
                                            .send(UiCommand::LangageChanged(other_language.clone()))
                                            .expect("sent");
                                    }
                                }
                            });
                    },
                );
            });

            {
                let mut context = build_toolbar_context(&self.app_tabs);

                self.app_state()
                    .toolbar
                    .ui(ui, &mut context);
            }
        });

        if !self.app_state().startup_done {
            self.app_state().startup_done = true;

            {
                let mut app_tabs = self.app_tabs.lock().unwrap();
                app_tabs.show_home_tab_on_startup(
                    self.config
                        .lock()
                        .unwrap()
                        .show_home_tab_on_startup,
                );
            }
            // FIXME due to a bug in egui_dock with `Tree::retain_tabs, we can't do this right now
            //       it's not ideal, but it's better to have new project/pcb tabs instead of a panic or broken UI.
            //       See: https://github.com/Adanos020/egui_dock/issues/264#issuecomment-2916873095
            if false {
                self.remove_new_project_tabs_on_startup();
                self.remove_new_pcb_tabs_on_startup();
            }
            self.restore_documents_on_startup();
        }

        // in a block to limit the scope of the `app_tabs` borrow/guard
        {
            profiling::scope!("ui::central_panel");
            // block required limit the scope of the `app_state` guard
            let (projects, pcbs, processes) = {
                let app_state = self.app_state();
                let projects = app_state.projects.clone();
                let pcbs = app_state.pcbs.clone();
                let processes = app_state.processes.clone();
                drop(app_state);
                (projects, pcbs, processes)
            };

            let config = self.config.clone();

            let mut tab_context = TabKindContext {
                config,
                projects,
                pcbs,
                processes,
            };

            let mut app_tabs = self.app_tabs.lock().unwrap();

            // FIXME remove this when `on_close` bugs in egui_dock are fixed.
            app_tabs.cleanup_tabs(&mut tab_context);

            CentralPanel::default()
                .frame(
                    Frame::central_panel(&ctx.global_style())
                        .inner_margin(0.)
                        .fill(Color32::TRANSPARENT),
                )
                .show(ui, |ui| {
                    app_tabs.ui(ui, &mut tab_context);
                });
        }

        let mut app_state = self.app_state();

        if let Some((_reason, picker, command_fn)) = app_state.file_picker.as_mut() {
            profiling::scope!("ui::file_picker");
            // FIXME this `update` method does not get called immediately after picking a file, instead update gets
            //       called when the user moves the mouse or interacts with the window again.
            match picker.picked() {
                Ok(picked_file) => {
                    let command = command_fn(picked_file);
                    app_state
                        .command_sender
                        .send(command)
                        .ok();

                    app_state.file_picker = None;
                }
                Err(PickError::Cancelled) => {
                    app_state.file_picker = None;
                }
                _ => {}
            }
        }

        // commands sent while drawing this frame are handled at the start of the next one.
        drop(app_state);
        if self
            .commands
            .as_ref()
            .is_some_and(|commands| !commands.is_empty())
        {
            ctx.request_repaint();
        }
    }
}

//
// project
//
fn project_from_path(
    path: PathBuf,
    projects: &mut ValueGuard<SlotMap<ProjectKey, Project>>,
    persisted_tabs: Option<Value<ProjectTabs>>,
    file_dialogs: Arc<dyn FileDialogs>,
) -> (Vec<ProjectUiCommand>, ProjectKey, Value<ProjectTabs>) {
    let mut project_commands = None;
    let mut project_tabs = None;
    let project_key = projects.insert_with_key(|key| {
        let new_project_tabs = persisted_tabs.unwrap_or_else(|| project::make_tabs(key));

        let (project, commands) = Project::from_path(path.clone(), key, new_project_tabs.clone(), file_dialogs.clone());

        project_commands.replace(commands);
        project_tabs = Some(new_project_tabs);

        project
    });
    (project_commands.unwrap(), project_key, project_tabs.unwrap())
}

fn project_from_args(
    args: NewProjectArgs,
    projects: &mut ValueGuard<SlotMap<ProjectKey, Project>>,
    file_dialogs: Arc<dyn FileDialogs>,
) -> (Vec<ProjectUiCommand>, ProjectKey, Value<ProjectTabs>, PathBuf) {
    let path = args.build_path();

    let mut project_commands = None;
    let mut project_tabs = None;
    let project_key = projects.insert_with_key(|key| {
        let new_project_tabs = project::make_tabs(key);
        let (project, commands) = Project::new(
            args.name,
            path.clone(),
            key,
            new_project_tabs.clone(),
            file_dialogs.clone(),
        );

        project_commands.replace(commands);
        project_tabs.replace(new_project_tabs);

        project
    });
    (project_commands.unwrap(), project_key, project_tabs.unwrap(), path)
}

fn configure_project_component(app_command_sender: Enqueue<UiCommand>, tab_key: TabKey, project: &mut Project) {
    project
        .component
        .configure_mapper(app_command_sender, move |(key, command)| UiCommand::TabCommand {
            tab_key,
            command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProjectTabCommand {
                command: ProjectTabUiCommand::ProjectCommand {
                    key,
                    command,
                },
            }),
        });
}

//
// pcb
//

fn pcb_from_path(
    path: PathBuf,
    pcbs: &mut ValueGuard<SlotMap<PcbKey, Pcb>>,
    persisted_tabs: Option<Value<PcbTabs>>,
) -> (Vec<PcbUiCommand>, PcbKey, Value<PcbTabs>) {
    let mut pcb_commands = None;
    let mut pcb_tabs = None;
    let pcb_key = pcbs.insert_with_key(|key| {
        let new_pcb_tabs = persisted_tabs.unwrap_or_else(|| pcb::make_tabs(key));

        let (pcb, commands) = Pcb::from_path(path.clone(), key, new_pcb_tabs.clone());
        pcb_commands.replace(commands);
        pcb_tabs = Some(new_pcb_tabs);

        pcb
    });
    (pcb_commands.unwrap(), pcb_key, pcb_tabs.unwrap())
}

fn pcb_from_args(
    args: NewPcbArgs,
    pcbs: &mut ValueGuard<SlotMap<PcbKey, Pcb>>,
) -> (Vec<PcbUiCommand>, PcbKey, Value<PcbTabs>, PathBuf) {
    let path = args.build_path();

    let mut pcb_commands = None;
    let mut pcb_tabs = None;
    let pcb_key = pcbs.insert_with_key(|key| {
        let new_pcb_tabs = pcb::make_tabs(key);
        let (pcb, commands) = Pcb::new(path.clone(), key, args.name, args.units, new_pcb_tabs.clone());

        pcb_commands.replace(commands);
        pcb_tabs.replace(new_pcb_tabs);

        pcb
    });
    (pcb_commands.unwrap(), pcb_key, pcb_tabs.unwrap(), path)
}

fn configure_pcb_component(app_command_sender: Enqueue<UiCommand>, tab_key: TabKey, pcb: &mut Pcb) {
    pcb.component
        .configure_mapper(app_command_sender, move |(key, command)| UiCommand::TabCommand {
            tab_key,
            command: TabUiCommand::TabKindCommand(TabKindUiCommand::PcbTabCommand {
                command: PcbTabUiCommand::PcbCommand {
                    key,
                    command,
                },
            }),
        });
}

//
// process
//

fn process_from_path(
    path: PathBuf,
    processes: &mut ValueGuard<SlotMap<ProcessKey, Process>>,
) -> (Vec<ProcessUiCommand>, ProcessKey) {
    let mut process_commands = None;
    let process_key = processes.insert_with_key(|key| {
        let (process, commands) = Process::from_path(path.clone(), key);
        process_commands.replace(commands);
        process
    });
    (process_commands.unwrap(), process_key)
}

fn configure_process_component(app_command_sender: Enqueue<UiCommand>, tab_key: TabKey, process: &mut Process) {
    process
        .component
        .configure_mapper(app_command_sender, move |(key, command)| UiCommand::TabCommand {
            tab_key,
            command: TabUiCommand::TabKindCommand(TabKindUiCommand::ProcessTabCommand {
                command: ProcessTabUiCommand::ProcessCommand {
                    key,
                    command,
                },
            }),
        });
}

//
// toolbar
//

pub fn build_toolbar_context(app_tabs: &Value<AppTabs>) -> ToolbarContext {
    let app_tabs = app_tabs.lock().unwrap();
    let active_tab = app_tabs.active_tab();

    let can_save = active_tab.map_or(false, |tab_key| {
        app_tabs.with_tab_mut(&tab_key, |tab_kind| match tab_kind {
            TabKind::Home(_, _) => false,
            TabKind::NewProject(_, _) => false,
            TabKind::NewPcb(_, _) => false,
            TabKind::Project(project_tab, _) => project_tab.modified,
            TabKind::Pcb(pcb_tab, _) => pcb_tab.modified,
            TabKind::NewProcess(_, _) => false,
            TabKind::Process(process_tab, _) => process_tab.modified,
        })
    });

    let context = ToolbarContext {
        active_tab,
        can_save,
    };
    context
}
