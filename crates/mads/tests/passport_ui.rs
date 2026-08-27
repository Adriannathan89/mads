//! Compile-time contracts for typed Passport principals.

#![cfg(all(feature = "http", feature = "jwt"))]

#[test]
fn passport_principal_accepts_supported_shapes() {
    trybuild::TestCases::new().pass("tests/ui-passport/pass/*.rs");
}

#[test]
fn passport_principal_rejects_unsupported_shapes() {
    trybuild::TestCases::new().compile_fail("tests/ui-passport/fail/*.rs");
}
