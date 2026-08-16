//! Compile tests for the public MADS attribute macros.

#[test]
fn core_attributes_accept_supported_shapes() {
    trybuild::TestCases::new().pass("tests/ui/pass/*.rs");
}

#[test]
fn core_attributes_reject_unsupported_shapes() {
    trybuild::TestCases::new().compile_fail("tests/ui/fail/*.rs");
}
