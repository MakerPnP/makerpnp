use std::path::PathBuf;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use regex::Regex;
use tracing::info;
use vizia::prelude::*;
use planner_app::{ProjectTree, ProjectView};
use crate::app_core::{CoreEvent, CoreService};
use crate::route::Route;

#[derive(Debug)]
enum ProjectEvent {
    Load { },
    Loaded { content: ProjectContent },
    LoadOrCreate {},
}

#[derive(Debug)]
pub enum ProjectRouteEvent {
    RouteChanged { route: Route }
}

#[derive(Clone, Data, Lens, Debug)]
pub struct Project {
    pub name: String,
    pub file_path: PathBuf,
}

#[derive(Clone, Lens, Default, Debug)]
pub struct ProjectContent {
    pub content: Option<String>,
    pub project_tree: ProjectTree,
}

impl ProjectContent {
    pub fn load(&mut self, ecx: &mut EventContext) {
        let content = ProjectContent {
            content: Some(format!("Dummy content")),
            project_tree: ProjectTree::default(),
        };

        ecx.emit(ProjectEvent::Loaded { content });
    }
}

#[derive(Debug)]
enum NewProjectFormEvent {
    SetName { text: String },
    SetPath { text: String },
    Ok,
}

#[derive(Default, Lens, Debug, Data, Clone)]
pub struct ProjectForm {
    pub name: String,
    // TODO consider renaming to directory_path for clarity
    pub path: String,
}

#[derive(Debug, Data, Clone)]
pub enum ProjectOrForm {
    Project(Project),
    Form(ProjectForm),
}

#[derive(Debug)]
enum ProjectContainerEvent {
    OnBuild {}
}

#[derive(Lens)]
pub struct ProjectState {
    pub project_or_form: ProjectOrForm,
    pub content: ProjectContent,
    
    #[lens(ignore)]
    pub core_service: CoreService,
}

impl Model for ProjectState {
    fn event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        event.take(|event, _meta| {
            println!("project event: {:?}", &event);
            match event {
                ProjectEvent::LoadOrCreate { } => {
                    match self.project_or_form {
                        ProjectOrForm::Project(ref _project) => {
                            ecx.emit(ProjectEvent::Load {})
                        }
                        ProjectOrForm::Form(ref _form) => {
                            // Nothing to do
                        }
                    }
                }
                ProjectEvent::Load { } => {
                    self.content.load(ecx);
                }
                ProjectEvent::Loaded { content } => {
                    self.content = content;
                    self.core_service.update(planner_app::Event::ProjectTree {}, ecx);
                }
            }
        });
        event.take(|event, _meta| {
            println!("project form event: {:?}", &event);
            if let ProjectOrForm::Form(ref mut form) = self.project_or_form {
                match event {
                    NewProjectFormEvent::SetName { text } => form.name = text,
                    NewProjectFormEvent::SetPath { text } => form.path = text,
                    NewProjectFormEvent::Ok => {
                        self.core_service.update(planner_app::Event::CreateProject {
                            project_name: form.name.to_string(),
                            directory_path: PathBuf::from(&form.path),
                        }, ecx)
                    }
                }
            }
        });

        event.take(|event, meta| {
            println!("core event: {:?}, project_or_form: {:?}", &event, &self.project_or_form);
            match event {
                CoreEvent::RenderView { view } => {
                    match view {
                        ProjectView::ProjectTree(project_tree) => {
                            self.content.project_tree = project_tree;
                        }
                        _ => todo!()
                    }
                },
                CoreEvent::Navigate { path } => {
                    let pattern = Regex::new(r"/project/load/(?<path>.*)").unwrap();
                    if let Some(captures) = pattern.captures(&path) {
                        let encoded_path = captures.name("path").unwrap().as_str();
                        let path = String::from_utf8(BASE64_STANDARD.decode(encoded_path).unwrap()).unwrap();
                        let path = PathBuf::from(path);

                        let name = Localized::new("spinner-loading").to_string_local(ecx);

                        let project = Project {
                            name,
                            file_path: path,
                        };

                        self.project_or_form = ProjectOrForm::Project(project);
                        ecx.emit(ProjectEvent::Load {});
                    } else {
                        todo!()
                    }
                }
            }
        });
    }
}

#[derive(Lens)]
pub struct ProjectContainer {
    pub active_tree_item: Option<usize>,
    
    // TODO maybe not needed after all...
    container_entity: Option<Entity>,
}

impl View for ProjectContainer {
    fn event(&mut self, ecx: &mut EventContext, event: &mut Event) {
        event.take(|event, _meta| {
            println!("project tree event: {:?}", &event);
            match event {
                ProjectTreeEvent::Change { index } => {

                    self.active_tree_item.replace(index);

                    ecx.emit(ProjectRouteEvent::RouteChanged { route: Route(Some(index)) })
                }
            }
        });

        event.take(|event, meta| {
            println!("project container event: {:?}", &event);
            match event {
               ProjectContainerEvent::OnBuild {} => {
                   self.container_entity.replace(meta.origin.clone());
                   ecx.emit(ProjectEvent::LoadOrCreate {});
               }
            } 
        });
    }
}

impl ProjectContainer {
    pub fn new(cx: &mut Context, active_section: Option<usize>) -> Handle<Self> {
        info!("ProjectContainer::new");

        Self {
            active_tree_item: active_section,
            container_entity: None,
        }.build(cx, |cx| {

            Binding::new(cx, ProjectState::project_or_form, |cx, project_or_form_lens| {
                match project_or_form_lens.get(cx) {
                    ProjectOrForm::Project(project) => {
                        //
                        // Project
                        //

                        HStack::new(cx, |cx| {
                            //
                            // Left
                            //
                            VStack::new(cx, |cx| {
                                let project_tree_lens = ProjectState::content.then(ProjectContent::project_tree).map_ref(|tree| &tree.items);

                                List::new(cx, project_tree_lens, |cx, index, item| {
                                    let is_item_active_lens = ProjectContainer::active_tree_item.map(move |selection| {
                                        let selected = match selection {
                                            Some(active_index) if *active_index == index => true,
                                            _ => false
                                        };

                                        println!("index: {}, selected: {}", index, selected);
                                        selected
                                    });

                                    Label::new(cx, item.map(|item| {
                                        item.name.clone()
                                    })).hoverable(false)
                                        .background_color(is_item_active_lens.map(|is_active| match *is_active {
                                            true => Color::rgb(0x00, 0x00, 0xff),
                                            false => Color::rgb(0xdd, 0xdd, 0xdd),
                                        }))
                                        .width(Stretch(1.0))
                                        .height(Pixels(30.0))
                                        .checked(is_item_active_lens)
                                        .on_press(move |ecx| ecx.emit(ProjectTreeEvent::Change { index }));
                                })
                                    .child_space(Pixels(4.0));
                            })
                                .width(Pixels(200.0))
                                .height(Percentage(100.0));

                            //
                            // Divider
                            //
                            Element::new(cx)
                                .width(Pixels(2.0))
                                .height(Percentage(100.0))
                                .background_color(Color::gray());

                            //
                            // Right
                            //
                            VStack::new(cx, |cx| {
                                let loading_message = Localized::new("spinner-loading").to_string_local(cx);
                                Label::new(cx, ProjectState::content.map(move |content| {
                                    // FIXME why do we need to clone things all the time
                                    content.content.clone().unwrap_or(loading_message.clone())
                                }))
                                    .text_align(TextAlign::Center);
                            })
                                .child_space(Stretch(1.0));
                        });
                    },
                    ProjectOrForm::Form(project_form) => {
                        
                        //
                        // New project form
                        //
                        VStack::new(cx, |cx: &mut Context| {
                            
                            
                            let name_lens = project_or_form_lens.map_ref(|project_or_form|{ 
                                match project_or_form {
                                    ProjectOrForm::Form( form) => &form.name,
                                    _ => unreachable!()
                                }
                            });
                            let path_lens = project_or_form_lens.map_ref(|project_or_form|{ 
                                match project_or_form {
                                    ProjectOrForm::Form( form) => &form.path,
                                    _ => unreachable!()
                                }
                            });

                            HStack::new(cx, |cx| {
                                Label::new(cx, Localized::new("popup-new-project-name-label"))
                                    .width(Stretch(1.0));
                                Textbox::new(cx, name_lens)
                                    .width(Stretch(4.0))
                                    .on_edit(|ecx, text| ecx.emit(NewProjectFormEvent::SetName { text }));
                            })
                                .width(Stretch(1.0));

                            HStack::new(cx, |cx| {
                                Label::new(cx, Localized::new("popup-new-project-path-label"))
                                    .width(Stretch(1.0));
                                Textbox::new(cx, path_lens)
                                    .width(Stretch(4.0))
                                    .on_edit(|ecx, text| ecx.emit(NewProjectFormEvent::SetPath { text }));
                            })
                                .width(Stretch(1.0));


                            HStack::new(cx, |cx| {
                                Element::new(cx)
                                    .width(Stretch(0.1));
                                Button::new(cx, |cx| Label::new(cx, "Ok")) // TODO i18n
                                    .on_press(|ecx| ecx.emit(NewProjectFormEvent::Ok))
                                    .width(Stretch(0.95));
                            })
                                .width(Stretch(1.0));
                        })
                            .child_space(Pixels(20.0))
                            .child_top(Stretch(1.0))
                            .child_bottom(Stretch(1.0))
                            .row_between(Pixels(12.0));
                    }
                }
            });
        }).on_build(move |ecx| {
            ecx.emit(ProjectContainerEvent::OnBuild {}); 
        })
    }
}

#[derive(Debug)]
enum ProjectTreeEvent {
    Change { index: usize }
}
