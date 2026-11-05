//! The device API model against TRMNL's published document.
//!
//! The excerpt in `model/trmnl/device-api.json` is the oracle for names, paths,
//! requirements and response types; `tools/trmnl-excerpt` refreshes it. Header
//! types are the one deliberate difference: the model takes every header as text
//! (see `model/smithy/device.smithy`).

use std::path::Path;

use afterimage_conformance::{Contract, Excerpt, extract_all};

fn trmnl() -> Contract {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../trmnl/device-api.json");
    let text = std::fs::read_to_string(&path).expect("model/trmnl/device-api.json exists");
    serde_json::from_str::<Excerpt>(&text)
        .expect("the excerpt parses")
        .contract
}

fn model() -> Contract {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gen/device-api.openapi.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} exists: run tools/generate", path.display()));
    let document = serde_json::from_str(&text).expect("the generated document parses");
    extract_all(&document).expect("the generated document has a device contract")
}

#[test]
fn models_every_operation_with_its_name_and_success_code() {
    let model = model();
    for (key, theirs) in &trmnl().operations {
        let ours = model
            .operations
            .get(key)
            .unwrap_or_else(|| panic!("{key} is not modelled"));
        assert!(
            ours.name.eq_ignore_ascii_case(&theirs.name),
            "{key} is {} in the model and {} at TRMNL",
            ours.name,
            theirs.name
        );
        assert_eq!(ours.success, theirs.success, "{key}: success code");
    }
}

#[test]
fn accepts_every_header_as_text_with_the_same_requirement() {
    let model = model();
    for (key, theirs) in &trmnl().operations {
        let ours = &model.operations[key];
        for (name, header) in &theirs.headers {
            let modelled = ours
                .headers
                .get(name)
                .unwrap_or_else(|| panic!("{key}: header {name} is not modelled"));
            assert_eq!(
                modelled.required, header.required,
                "{key}: header {name} required"
            );
            assert_eq!(
                modelled.kind, "string",
                "{key}: header {name} is text on the wire"
            );
        }
    }
}

#[test]
fn accepts_every_request_body_field() {
    let model = model();
    for (key, theirs) in &trmnl().operations {
        assert_eq!(
            model.operations[key].body, theirs.body,
            "{key}: request body"
        );
    }
}

#[test]
fn answers_every_response_field_with_the_same_type() {
    let model = model();
    for (key, theirs) in &trmnl().operations {
        let ours = &model.operations[key];
        for (name, field) in &theirs.fields {
            let modelled = ours
                .fields
                .get(name)
                .unwrap_or_else(|| panic!("{key}: response field {name} is not modelled"));
            assert_eq!(
                modelled.kind, field.kind,
                "{key}: response field {name} type"
            );
        }
    }
}

/// Members the firmware uses and TRMNL's document does not list, each with the
/// firmware 1.8.18 source that establishes it.
const FIRMWARE_ADDITIONS: [(&str, &str, &str); 2] = [
    (
        "GET /api/display",
        "ID",
        "lib/trmnl/src/api-client/request_headers.cpp: buildDisplayHeaders sends the MAC",
    ),
    (
        "GET /api/display",
        "special_function",
        "lib/trmnl/src/api-client/request_headers.cpp: buildDisplayHeaders spells it so",
    ),
];

/// Operations of Afterimage's own in the device API, each with why TRMNL's document
/// has no place for it.
const AFTERIMAGE_OPERATIONS: [(&str, &str); 1] = [(
    "GET /api/images/{name}",
    "TRMNL's document leaves `image_url` to the server; firmware downloads it with buildImageHeaders",
)];

#[test]
fn adds_nothing_but_listed_firmware_additions() {
    let mut model = model();
    let trmnl = trmnl();
    for (operation, _) in AFTERIMAGE_OPERATIONS {
        assert!(
            model.operations.remove(operation).is_some(),
            "listed operation {operation} is not in the model"
        );
    }
    let operations = |contract: &Contract| contract.operations.keys().cloned().collect::<Vec<_>>();
    assert_eq!(operations(&model), operations(&trmnl), "operations");
    let listed = |key: &str, name: &str| {
        FIRMWARE_ADDITIONS
            .iter()
            .any(|(operation, member, _)| *operation == key && *member == name)
    };
    for (key, ours) in &model.operations {
        let theirs = &trmnl.operations[key];
        let extra_headers: Vec<_> = ours
            .headers
            .keys()
            .filter(|name| !theirs.headers.contains_key(*name) && !listed(key, name))
            .collect();
        let extra_fields: Vec<_> = ours
            .fields
            .keys()
            .filter(|name| !theirs.fields.contains_key(*name) && !listed(key, name))
            .collect();
        assert!(
            extra_headers.is_empty(),
            "{key}: headers TRMNL does not document: {extra_headers:?}"
        );
        assert!(
            extra_fields.is_empty(),
            "{key}: response fields TRMNL does not document: {extra_fields:?}"
        );
    }
    for (key, name, _) in FIRMWARE_ADDITIONS {
        let ours = &model.operations[key];
        assert!(
            ours.headers.contains_key(name) || ours.fields.contains_key(name),
            "{key}: listed addition {name} is not in the model"
        );
    }
}
