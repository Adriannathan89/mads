//! Shared private inspection protocol fixture behavior.

use std::{env, fs, thread, time::Duration};

use mads_common::__private::{
    INSPECTION_ACK_ENV, INSPECTION_KIND_ENV, INSPECTION_PROTOCOL_VERSION, INSPECTION_RESPONSE_ENV,
    INSPECTION_TOKEN_ENV, INSPECTION_VERSION_ENV, InspectionKind,
};

/// Runs one platform-neutral private protocol fixture behavior.
pub fn run(mode: &str) {
    if mode == "early_exit" {
        return;
    }

    let token = env::var(INSPECTION_TOKEN_ENV).expect("token should be set");
    let ack_path = env::var(INSPECTION_ACK_ENV).expect("ack path should be set");
    let response_path = env::var(INSPECTION_RESPONSE_ENV).expect("response path should be set");
    let kind = inspection_kind(&env::var(INSPECTION_KIND_ENV).expect("kind should be set"));
    assert_eq!(
        env::var(INSPECTION_VERSION_ENV).expect("version should be set"),
        INSPECTION_PROTOCOL_VERSION.to_string()
    );

    let ack_token = if mode == "wrong_token" { "incorrect-token" } else { &token };
    fs::write(
        ack_path,
        format!(
            r#"{{"protocol_version":{},"token":"{}"}}"#,
            INSPECTION_PROTOCOL_VERSION, ack_token
        ),
    )
    .expect("ack should be written");

    if mode == "timeout" {
        thread::sleep(Duration::from_secs(30));
        return;
    }

    if mode == "malformed" {
        fs::write(response_path, "this is not JSON").expect("response should be written");
        return;
    }

    let response_token = if mode == "wrong_token" { "incorrect-token" } else { &token };
    let version = if mode == "wrong_version" {
        INSPECTION_PROTOCOL_VERSION + 1
    } else {
        INSPECTION_PROTOCOL_VERSION
    };
    let report = format!(
        r#"{{"kind":"{}","graph":{{"root_module":null,"modules":[],"imports":[],"providers":[],"dependencies":[],"construction_order":null,"auto_configurations":[]}},"routes":[],"checks":[],"diagnostics":[],"failed":false}}"#,
        inspection_kind_name(kind)
    );
    fs::write(
        response_path,
        format!(
            r#"{{"protocol_version":{version},"token":"{response_token}","report":{report}}}"#
        ),
    )
    .expect("response should be written");
}

const fn inspection_kind_name(kind: InspectionKind) -> &'static str {
    match kind {
        InspectionKind::Routes => "routes",
        InspectionKind::Graph => "graph",
        InspectionKind::Doctor => "doctor",
    }
}

fn inspection_kind(value: &str) -> InspectionKind {
    match value {
        "routes" => InspectionKind::Routes,
        "graph" => InspectionKind::Graph,
        "doctor" => InspectionKind::Doctor,
        _ => panic!("unexpected inspection kind"),
    }
}
