//! The stores core's parts data sources (`STORE-1` to `STORE-6`).
//!
//! A view's data source is the canonical one (`STORE-6`), so the views are compared with `canonical_parts_source`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crux_core::Core;
use stores_app::{
    canonical_parts_source, Effect, Event, Part, PartsSource, PartsView, PartsViewItem, PartsViewRendererOperation,
    Stores,
};

fn csv_file(folder: &Path, name: &str, content: &str) -> PartsSource {
    let path = folder.join(name);
    fs::write(&path, content).unwrap();
    PartsSource::File(path)
}

/// The views the event's effects carry, and the core's error after the event, if any.
fn process(core: &Core<Stores>, event: Event) -> (Vec<PartsView>, Option<String>) {
    let mut views = vec![];
    for effect in core.process_event(event) {
        match effect {
            Effect::Render(_) => {}
            Effect::PartsView(request) => {
                let PartsViewRendererOperation::View {
                    view,
                } = request.operation;
                views.push(view);
            }
        }
    }
    (views, core.view().error)
}

fn item(manufacturer: &str, mpn: &str, fields: &[(&str, &str)]) -> PartsViewItem {
    PartsViewItem {
        part: Part::new(manufacturer.to_string(), mpn.to_string()),
        fields: fields
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// `STORE-1`, `STORE-2`
#[test]
fn loads_the_parts_from_a_csv_file_in_order_with_their_other_values() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\",\"LCSC\",\"Notes\"\n\
         \"RESCO\",\"RES1\",\"C1234\",\"\"\n\
         \"CAPCO\",\"CAP1\",\"C5678\",\"0402\"\n",
    );
    let core: Core<Stores> = Core::new();

    let expected_view = PartsView {
        source: canonical_parts_source(&source),
        field_names: vec!["LCSC".to_string(), "Notes".to_string()],
        parts: vec![
            item("RESCO", "RES1", &[("LCSC", "C1234"), ("Notes", "")]),
            item("CAPCO", "CAP1", &[("LCSC", "C5678"), ("Notes", "0402")]),
        ],
    };

    // `STORE-5`, loading sends the view, so that every view of the data source can be updated.
    let (views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views, vec![expected_view.clone()]);

    let (views, error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views, vec![expected_view]);
}

/// `STORE-1`
#[test]
fn a_file_with_only_the_headers_has_no_parts() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let core: Core<Stores> = Core::new();

    process(&core, Event::LoadParts {
        source: source.clone(),
    });
    let (views, error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });

    assert_eq!(error, None);
    assert_eq!(views, vec![PartsView {
        source: canonical_parts_source(&source),
        field_names: vec![],
        parts: vec![],
    }]);
}

/// `STORE-1`, loading again shows the data source's current parts.
#[test]
fn loading_again_replaces_the_parts() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let core: Core<Stores> = Core::new();
    process(&core, Event::LoadParts {
        source: source.clone(),
    });

    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"CAPCO\",\"CAP1\"\n",
    );
    process(&core, Event::LoadParts {
        source: source.clone(),
    });
    let (views, _error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });

    assert_eq!(views[0].parts, vec![item("CAPCO", "CAP1", &[])]);
}

/// `STORE-3`, the error names the missing header, so the user can fix the file.
#[test]
fn refuses_a_file_without_the_headers() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Maker\",\"Mpn\"\n\"RESCO\",\"RES1\"\n");
    let core: Core<Stores> = Core::new();

    let (_views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });

    let error = error.expect("an error");
    assert!(
        error.starts_with(&format!(
            "Unable to load the parts. source: '{}', cause: ",
            canonical_parts_source(&source)
        )),
        "{error}"
    );
    assert!(error.contains("Missing header. header: 'Manufacturer'"), "{error}");
}

/// `STORE-3`
#[test]
fn refuses_a_file_that_does_not_exist() {
    let folder = tempfile::tempdir().unwrap();
    let source = PartsSource::File(folder.path().join("missing.csv"));
    let core: Core<Stores> = Core::new();

    let (_views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });

    assert!(error.is_some());
}

/// `STORE-2`, parts can only be shown once they're loaded, and an event that succeeds clears the error.
#[test]
fn refuses_to_show_parts_that_are_not_loaded() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let core: Core<Stores> = Core::new();

    let (views, error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });
    assert_eq!(views, vec![]);
    assert_eq!(
        error,
        Some(format!(
            "The parts are not loaded. source: '{}'",
            canonical_parts_source(&source)
        ))
    );

    let (_views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
}

/// `STORE-1`, `ISSUE-23`, values that look like numbers are kept as they are in the file.
#[test]
fn keeps_values_that_look_like_numbers() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\",\"Package\"\n\"RESCO\",\"1234\",\"0402\"\n",
    );
    let core: Core<Stores> = Core::new();

    let (_views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    let (views, _error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });

    assert_eq!(views[0].parts, vec![item("RESCO", "1234", &[("Package", "0402")])]);
}

/// `STORE-3`, a refused load keeps the parts loaded before.
#[test]
fn a_refused_load_keeps_the_parts_loaded_before() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let core: Core<Stores> = Core::new();
    process(&core, Event::LoadParts {
        source: source.clone(),
    });

    csv_file(folder.path(), "parts.csv", "\"Maker\",\"Mpn\"\n");
    let (_views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });
    assert!(error.is_some());

    let (views, error) = process(&core, Event::RequestPartsView {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-5`, opening a data source that's open already uses the parts loaded, without loading them again.
#[test]
fn opening_again_uses_the_parts_loaded_before() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let core: Core<Stores> = Core::new();

    let (views, error) = process(&core, Event::OpenParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);

    // a change to the file isn't seen by opening it again, only by loading it again.
    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"CAPCO\",\"CAP1\"\n",
    );
    let (views, error) = process(&core, Event::OpenParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);

    let (views, error) = process(&core, Event::LoadParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("CAPCO", "CAP1", &[])]);
}

/// `STORE-3`, `STORE-5`, a refused open sends no view, and opening again tries to load the parts again.
#[test]
fn a_refused_open_sends_no_view() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Maker\",\"Mpn\"\n");
    let core: Core<Stores> = Core::new();

    let (views, error) = process(&core, Event::OpenParts {
        source: source.clone(),
    });
    assert!(error.is_some());
    assert_eq!(views, vec![]);

    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let (views, error) = process(&core, Event::OpenParts {
        source: source.clone(),
    });
    assert_eq!(error, None);
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-6`, one file is one data source, however its path is written.
#[test]
fn a_file_is_one_data_source_however_its_path_is_written() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("other")).unwrap();
    let source = csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"RESCO\",\"RES1\"\n",
    );
    let other_spelling = PartsSource::File(
        folder
            .path()
            .join("other")
            .join("..")
            .join("parts.csv"),
    );
    assert_ne!(source, other_spelling);
    let core: Core<Stores> = Core::new();

    process(&core, Event::OpenParts {
        source: source.clone(),
    });
    csv_file(
        folder.path(),
        "parts.csv",
        "\"Manufacturer\",\"Mpn\"\n\"CAPCO\",\"CAP1\"\n",
    );

    // the same data source, already loaded, so not loaded again.
    let (views, error) = process(&core, Event::OpenParts {
        source: other_spelling,
    });
    assert_eq!(error, None);
    assert_eq!(views[0].source, canonical_parts_source(&source));
    assert_eq!(views[0].parts, vec![item("RESCO", "RES1", &[])]);
}

/// `STORE-6`, on Windows, file names that differ only in case are the same file.
#[cfg(windows)]
#[test]
fn a_file_is_one_data_source_whatever_the_case_of_its_path() {
    let folder = tempfile::tempdir().unwrap();
    let source = csv_file(folder.path(), "parts.csv", "\"Manufacturer\",\"Mpn\"\n");
    let other_case = PartsSource::File(folder.path().join("PARTS.CSV"));

    assert_eq!(canonical_parts_source(&other_case), canonical_parts_source(&source));
}

/// `STORE-6`, a data source whose file doesn't exist is kept as it is, so an error names it as the user did.
#[test]
fn a_data_source_whose_file_does_not_exist_is_kept_as_it_is() {
    let folder = tempfile::tempdir().unwrap();
    let source = PartsSource::File(folder.path().join("missing.csv"));

    assert_eq!(canonical_parts_source(&source), source);
}
