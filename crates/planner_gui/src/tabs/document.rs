use vizia::prelude::{Color, Data, Element, Label, Percentage, ScrollView, TabPair};
use vizia::context::EventContext;
use vizia::events::Event;
use vizia::modifiers::{AbilityModifiers, LayoutModifiers, StyleModifiers};
use crate::document::{Document, DocumentContainer, DocumentRouteEvent};
use crate::route::Route;

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