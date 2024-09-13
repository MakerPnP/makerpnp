use vizia::prelude::{Color, Data, Element, Label, Percentage, ScrollView, Stretch, TabPair, TextAlign, VStack};
use vizia::context::EventContext;
use vizia::events::Event;
use vizia::modifiers::{AbilityModifiers, LayoutModifiers, StyleModifiers, TextModifiers};
use crate::route::Route;

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