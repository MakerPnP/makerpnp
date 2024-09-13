use vizia::prelude::*;

#[derive(Clone)]
pub struct LanguagePair {
    pub code: String,
    pub name: String,
}

pub fn load_languages(languages: &[LanguagePair], cx: &mut Context) {
    for pair in languages {
        match pair.code.as_str() {
            "en-US" => {
                cx.add_translation(
                    "en-US".parse().unwrap(),
                    include_str!("../resources/translations/en-US/planner.ftl").to_owned(),
                );
            },
            "es-ES" => {
                cx.add_translation(
                    "es-ES".parse().unwrap(),
                    include_str!("../resources/translations/es-ES/planner.ftl").to_owned(),
                );
            },
            _ => unreachable!()
        }
    }
}