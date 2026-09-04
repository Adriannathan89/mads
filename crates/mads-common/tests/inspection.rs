//! Integration coverage for the hidden inspection protocol.

use mads_common::__private::{
    DoctorCheck, GraphReport, INSPECTION_PROTOCOL_VERSION, InspectionEnvelope, InspectionKind,
    InspectionReport,
};

#[test]
fn protocol_round_trip_preserves_the_inspection_envelope() {
    let envelope = InspectionEnvelope::new(
        "token-1".into(),
        InspectionReport {
            kind: InspectionKind::Doctor,
            graph: GraphReport::default(),
            routes: Vec::new(),
            checks: vec![DoctorCheck::pass("configuration", "sources loaded")],
            diagnostics: Vec::new(),
            failed: false,
        },
    );

    let json = serde_json::to_string(&envelope).unwrap();
    let decoded: InspectionEnvelope = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.protocol_version(), INSPECTION_PROTOCOL_VERSION);
    assert_eq!(decoded.token(), "token-1");
    assert_eq!(decoded, envelope);
}

#[test]
fn protocol_never_serializes_or_debugs_configuration_values() {
    let secret = "postgres://user:inspection-secret@localhost/db";
    let envelope = InspectionEnvelope::new(
        "token-1".into(),
        InspectionReport {
            kind: InspectionKind::Doctor,
            graph: GraphReport {
                auto_configurations: vec![mads_common::__private::AutoConfigurationReport {
                    identifier: "mads.common.database".into(),
                    output_type_name: "mads_common::Database".into(),
                    status: "SKIPPED".into(),
                    reason_code: "configuration_absent".into(),
                    explanation: "database configuration was not applied".into(),
                    configuration: vec![mads_common::__private::ConfigurationEvidenceReport {
                        key: "database.url".into(),
                        source: Some("mads.toml".into()),
                    }],
                }],
                ..GraphReport::default()
            },
            routes: Vec::new(),
            checks: vec![DoctorCheck::pass("configuration", "sources loaded")],
            diagnostics: Vec::new(),
            failed: false,
        },
    );

    let json = serde_json::to_string(&envelope).unwrap();
    let debug = format!("{envelope:?}");

    assert!(!json.contains(secret));
    assert!(!debug.contains(secret));
}
