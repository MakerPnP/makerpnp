use std::path::PathBuf;
use std::thread::sleep;
use vizia::prelude::*;
use planner_app::{ProjectTree, ProjectView};
use crate::app_core::CoreEvent;
use crate::CORE_SERVICE;
use crate::route::Route;

#[derive(Debug)]
enum ProjectEvent {
    Load { id: String },
    Loaded { content: ProjectContent }
}

#[derive(Debug)]
pub enum ProjectRouteEvent {
    RouteChanged { project_id: String, route: Route }
}

#[derive(Clone, Data, Lens)]
pub struct Project {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Lens, Default, Debug)]
pub struct ProjectContent {
    pub content: Option<String>,
    pub project_tree: ProjectTree,
}

impl ProjectContent {
    pub fn load(&mut self, ecx: &mut EventContext, id: &String) {
        let id = id.clone();

        let content = ProjectContent {
            content: Some(format!("Dummy content for {}", id)),
            project_tree: ProjectTree::default(),
        };

        ecx.emit(ProjectEvent::Loaded { content });

        CORE_SERVICE.update(planner_app::Event::ProjectTree {
            // FIXME the id is the path and reference for now.
            reference: id.clone(),
            path: PathBuf::from(id),
        }, ecx);
    }
}

#[derive(Lens)]
pub struct ProjectContainer {
    pub project: Project,
    pub content: ProjectContent,
    pub active_tree_item: Option<usize>,
}

impl View for ProjectContainer {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.take(|event, _meta| {
            println!("project tree event: {:?}", &event);
            match event {
                ProjectTreeEvent::Change { index } => {

                    self.active_tree_item.replace(index);

                    cx.emit(ProjectRouteEvent::RouteChanged { project_id: self.project.id.clone(), route: Route(Some(index)) })
                }
            }
        });
        event.take(|event, _meta| {
            println!("project event: {:?}", &event);
            match event {
                ProjectEvent::Load { id } => {
                    self.content.load(cx, &id);
                }
                ProjectEvent::Loaded { content} => {
                    self.content = content;
                }
            }
        });
        event.take(|event, meta| {
            println!("core event: {:?}, project_id: {:?}", &event, &self.project.id);
            match event {
                CoreEvent::RenderView { reference, view } if reference.eq(&self.project.id) => {
                    match view {
                        ProjectView::ProjectTree(project_tree) => {
                            self.content.project_tree = project_tree;
                        }
                        _ => todo!()
                    }
                }
                _ => {}
            }
        })
    }
}

impl ProjectContainer {
    pub fn new(cx: &mut Context, project: Project, active_section: Option<usize>) -> Handle<Self> {
        let id = project.id.clone();

        Self {
            project,
            content: ProjectContent::default(),
            active_tree_item: active_section,
        }.build(cx, |cx| {

            HStack::new(cx, | cx | {
                //
                // Left
                //
                VStack::new(cx, |cx| {
                    let project_tree_lens = ProjectContainer::content.then(ProjectContent::project_tree).map_ref(|tree|&tree.items);

                    List::new(cx, project_tree_lens, |cx, index, item| {

                        let is_item_active_lens = ProjectContainer::active_tree_item.map(move |selection|{
                            let selected = match selection {
                                Some(active_index) if *active_index == index => true,
                                _ => false
                            };

                            println!("index: {}, selected: {}", index, selected);
                            selected
                        });

                        Label::new(cx, item.map(|item|{
                            item.name.clone()
                        })).hoverable(false)
                            .background_color(is_item_active_lens.map(|is_active| match *is_active {
                                true => Color::rgb(0x00, 0x00, 0xff),
                                false => Color::rgb(0xdd, 0xdd, 0xdd),
                            }))
                            .width(Stretch(1.0))
                            .height(Pixels(30.0))
                            .checked(is_item_active_lens)
                            .on_press(move |ecx|ecx.emit(ProjectTreeEvent::Change { index }));
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
                    Label::new(cx, ProjectContainer::content.map(move |content| {
                        // FIXME why do we need to clone things all the time
                        content.content.clone().unwrap_or(loading_message.clone())
                    }))
                        .text_align(TextAlign::Center);
                })
                    .child_space(Stretch(1.0));

            });

        }).on_build(move |ecx| {
            ecx.emit(ProjectEvent::Load { id: id.clone() })
        })
    }
}

#[derive(Debug)]
enum ProjectTreeEvent {
    Change { index: usize }
}
