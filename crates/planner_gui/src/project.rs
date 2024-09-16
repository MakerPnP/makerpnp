use std::thread::sleep;
use vizia::prelude::*;
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
    pub sections: Vec<String>,
}

impl ProjectContent {
    pub fn load(&mut self, cx: &mut EventContext, id: &String) {
        // Simulate loading a file, slowly.
        let id = id.clone();
        cx.spawn(move |cp|{
            sleep(Duration::from_millis(250));

            let content = ProjectContent {
                content: Some(format!("Dummy content for {}", id)),
                sections: vec![
                    "Section 1".to_string(),
                    "Section 2".to_string(),
                ],
            };
            let result = cp.emit(ProjectEvent::Loaded { content });
            match result {
                Ok(_) => println!("emitted content, id: {}", id),
                Err(e) => println!("failed to emit content, id: {}, error: {}", id, e),
            }
        });
    }
}

#[derive(Clone, Lens)]
pub struct ProjectContainer {
    pub project: Project,
    pub content: ProjectContent,
    pub active_section: Option<usize>,
}

impl View for ProjectContainer {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.take(|event, _meta| {
            println!("section event: {:?}", &event);
            match event {
                SectionEvent::Change { index } => {

                    self.active_section.replace(index);

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
        })
    }
}

impl ProjectContainer {
    pub fn new(cx: &mut Context, project: Project, active_section: Option<usize>) -> Handle<Self> {
        let id = project.id.clone();

        Self {
            project,
            content: ProjectContent::default(),
            active_section,
        }.build(cx, |cx| {

            HStack::new(cx, | cx | {
                //
                // Left
                //
                VStack::new(cx, |cx| {
                    let sections_lens = ProjectContainer::content.then(ProjectContent::sections);

                    List::new(cx, sections_lens, |cx, index, item| {

                        let foo = ProjectContainer::active_section.map(move |selection|{
                            let selected = match selection {
                                Some(active_index) if *active_index == index => true,
                                _ => false
                            };

                            println!("index: {}, selected: {}", index, selected);
                            selected
                        });

                        Label::new(cx, item).hoverable(false)
                            .background_color(foo.map(|foobar| match *foobar {
                                true => Color::rgb(0x00, 0x00, 0xff),
                                false => Color::rgb(0xdd, 0xdd, 0xdd),
                            }))
                            .width(Stretch(1.0))
                            .height(Pixels(30.0))
                            .checked(foo)
                            .on_press(move |ecx|ecx.emit(SectionEvent::Change { index }));
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
enum SectionEvent {
    Change { index: usize }
}
