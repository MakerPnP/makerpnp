use vizia::prelude::*;
use crate::document::{Document, DocumentContainer, DocumentRouteEvent};
use crate::route::Route;
use crate::tabbed_document_container::TabbedDocument;

#[derive(Clone, Data)]
pub enum TabKind {
    Home(HomeTab),
    Document(DocumentTab),
}

#[derive(Clone, Data)]
pub struct DocumentTab {
    pub document: Document,
    pub route: Route,
}

impl DocumentTab {
    pub fn build_tab(&self) -> TabPair {
        let document = self.document.clone();
        let name = document.name.clone();
        let active_section = self.route.0.clone();

        let tab = TabPair::new(
            move |cx| {
                Label::new(cx, name.clone()).hoverable(false);
                Element::new(cx).class("indicator");
            },
            move |cx| {
                let document_for_scrollview = document.clone();
                ScrollView::new(cx, 0.0, 0.0, false, true, move |cx| {
                    DocumentContainer::new(cx, document_for_scrollview.clone(), active_section);
                })
                    .background_color(Color::rgb(0xdd, 0xdd, 0xdd))
                    .height(Percentage(100.0))
                    .width(Percentage(100.0));
            },
        );

        tab
    }

    pub fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|event, _meta| {
            println!("section event: {:?}", &event);
            match event {
                DocumentRouteEvent::RouteChanged { document_id, route } => {
                    if document_id.eq(&self.document.id) {
                        self.route = route.clone()
                    }
                }
            }
        });
    }
}

#[derive(Clone, Data)]
pub struct HomeTab {
    pub route: Route,
}

impl HomeTab {
    pub fn build_tab(&self, name: String) -> TabPair {
        let tab = TabPair::new(
            move |cx| {
                Label::new(cx, name.clone()).hoverable(false);
                Element::new(cx).class("indicator");
            },
            |cx| {
                ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
                    VStack::new(cx, |cx|{
                        Label::new(cx, "🏠 Home")
                            .text_align(TextAlign::Center);
                    }).child_space(Stretch(1.0));
                })
                    .background_color(Color::rgb(0xbb, 0xbb, 0xbb))
                    .height(Percentage(100.0))
                    .width(Percentage(100.0));
            },
        );

        tab
    }

    pub fn event(&mut self, _cx: &mut EventContext, _event: &mut Event) {
        // nothing to do
    }
}

impl TabKind {
    pub fn name(&self) -> String {
        match self {
            TabKind::Home(_) => { "Home".to_string() }
            TabKind::Document(document_tab) => { document_tab.document.name.clone() }
        }
    }
}

impl TabbedDocument for TabKind {

    fn build_tab(&self) -> TabPair {
        match self {
            TabKind::Home(tab) => tab.build_tab(self.name()),
            TabKind::Document(tab) => tab.build_tab(),
        }
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        match self {
            TabKind::Home(tab) => tab.event(cx, event),
            TabKind::Document(tab) => tab.event(cx, event)
        }
    }
}
