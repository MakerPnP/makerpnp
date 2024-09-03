use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};
use dioxus::prelude::{Signal, use_signal};
use dioxus_sdk::i18n::{Language, use_i18, use_init_i18n};
use unic_langid::LanguageIdentifier;
// TODO make loading languages dynamic so that translators don't have to re-compile to test.
//      the code is prepared for this by using the two mutexes above instead of using
//      static constants.

static EN_US: &str = include_str!("../assets/i18n/en-US.json");
static ES_ES: &str = include_str!("../assets/i18n/es-ES.json");

static LANGUAGES: Mutex<Vec<LanguagePair>> = Mutex::new(Vec::new());
static SELECTED_LANGUAGE: Mutex<Option<LanguagePair>> = Mutex::new(None);

pub fn initialise() {

    let languages: Vec<LanguagePair> = vec![
        LanguagePair { code: "en-US".to_string(), name: "English (United-States)".to_string() },
        LanguagePair { code: "es-ES".to_string(), name: "Español (España)".to_string() },
    ];

    let first_language: &LanguagePair = languages.first().unwrap();
    let first_language_identifier: LanguageIdentifier = first_language.code.parse().unwrap();

    use_init_i18n(first_language_identifier.clone(), first_language_identifier, || {
        languages.iter().map(|LanguagePair { code, name: _name }|{
            match code.as_str() {
                "en-US" => Language::from_str(EN_US).unwrap(),
                "es-ES" => Language::from_str(ES_ES).unwrap(),
                _ => panic!()
            }
        }).collect()
    });

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(first_language.clone());

    let mut guard = LANGUAGES.lock().unwrap();
    (*guard).extend(languages);

}

pub fn change(language_pair: &LanguagePair) {

    let mut i18n = use_i18();

    let mut guard = SELECTED_LANGUAGE.lock().unwrap();
    (*guard).replace(language_pair.clone());

    i18n.set_language(language_pair.code.parse().unwrap());
}

pub fn languages() -> MutexGuard<'static, Vec<LanguagePair>> {

    let guard = LANGUAGES.lock().expect("not locked");

    guard
}

#[derive(Clone)]
pub struct LanguagePair {
    pub code: String,
    pub name: String,
}

// FIXME avoid cloning, return some reference instead, but how!
pub fn selected() -> LanguagePair {

    let guard = SELECTED_LANGUAGE.lock().expect("not locked");

    guard.as_ref().unwrap().clone()
}

pub fn use_change_language() -> Signal<Box<(impl FnMut(LanguagePair) + 'static)>> {
    let mut i18n = use_i18();

    let closure = move |language_pair: LanguagePair| {
        let mut guard = SELECTED_LANGUAGE.lock().unwrap();

        i18n.set_language(language_pair.code.parse().unwrap());
        (*guard).replace(language_pair);
    };

    use_signal(move || Box::new(closure))
}
