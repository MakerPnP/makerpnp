use std::thread::sleep;
use vizia::prelude::*;
use crate::route::Route;

#[derive(Debug)]
enum DocumentEvent {
    Load { id: String },
    Loaded { content: DocumentContent }
}

#[derive(Debug)]
pub enum DocumentRouteEvent {
    RouteChanged { document_id: String, route: Route }
}

#[derive(Clone, Data, Lens)]
pub struct Document {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Lens, Default, Debug)]
pub struct DocumentContent {
    pub content: Option<String>,
    pub sections: Vec<String>,
}

impl DocumentContent {
    pub fn load(&mut self, cx: &mut EventContext, id: &String) {
        // Simulate loading a file, slowly.
        let id = id.clone();
        cx.spawn(move |cp|{
            sleep(Duration::from_millis(250));

            let content = match id.as_str() {
                "document_1" => DocumentContent {
                    content: Some("content for document 1".to_string()),
                    sections: vec![
                        "Section 4".to_string(),
                        "Section 2".to_string(),
                    ],
                },
                "document_2" => DocumentContent {
                    content: Some("content for document 1".to_string()),
                    sections: vec![
                        "Section 6".to_string(),
                        "Section 9".to_string(),
                    ],
                },
                _ => unreachable!()
            };
            let result = cp.emit(DocumentEvent::Loaded { content });
            match result {
                Ok(_) => println!("emitted content, id: {}", id),
                Err(e) => println!("failed to emit content, id: {}, error: {}", id, e),
            }
        });
    }
}

#[derive(Clone, Lens)]
pub struct DocumentContainer {
    pub document: Document,
    pub content: DocumentContent,
    pub active_section: Option<usize>,
}

impl View for DocumentContainer {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.take(|event, _meta| {
            println!("section event: {:?}", &event);
            match event {
                SectionEvent::Change { index } => {

                    self.active_section.replace(index);

                    cx.emit(DocumentRouteEvent::RouteChanged { document_id: self.document.id.clone(), route: Route(Some(index)) })
                }
            }
        });
        event.take(|event, _meta| {
            println!("document event: {:?}", &event);
            match event {
                DocumentEvent::Load { id } => {
                    self.content.load(cx, &id);
                }
                DocumentEvent::Loaded { content} => {
                    self.content = content;
                }
            }
        })
    }
}

impl DocumentContainer {
    pub fn new(cx: &mut Context, document: Document, active_section: Option<usize>) -> Handle<Self> {
        let id = document.id.clone();

        Self {
            document,
            content: DocumentContent::default(),
            active_section,
        }.build(cx, |cx| {

            HStack::new(cx, | cx | {
                //
                // Left
                //
                VStack::new(cx, |cx| {
                    let sections_lens = DocumentContainer::content.then(DocumentContent::sections);

                    List::new(cx, sections_lens, |cx, index, item| {

                        let foo = DocumentContainer::active_section.map(move |selection|{
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
                    Label::new(cx, DocumentContainer::content.map(move |content| {
                        // FIXME why do we need to clone things all the time
                        content.content.clone().unwrap_or(loading_message.clone())
                    }))
                        .text_align(TextAlign::Center);
                })
                    .child_space(Stretch(1.0));

            });

        }).on_build(move |ecx| {
            ecx.emit(DocumentEvent::Load { id: id.clone() })
        })
    }
}

#[derive(Debug)]
enum SectionEvent {
    Change { index: usize }
}
