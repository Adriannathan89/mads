//! Public extractor surface tests.

#![cfg(feature = "http")]

use mads_common::{Header, Json, Path, Query, Request, axum, headers};

#[test]
fn common_exports_standard_http_types() {
    fn accepts_path<T>(_: Path<T>) {}
    fn accepts_query<T>(_: Query<T>) {}
    fn accepts_json<T>(_: Json<T>) {}
    fn accepts_header<T: headers::Header>(_: Header<T>) {}
    fn accepts_request(_: Request) {}

    fn accepts_axum_path(path: axum::extract::Path<u64>) {
        accepts_path(path);
    }
    fn accepts_axum_query(query: axum::extract::Query<u64>) {
        accepts_query(query);
    }
    fn accepts_axum_json(json: axum::Json<u64>) {
        accepts_json(json);
    }
    fn accepts_axum_request(request: axum::extract::Request) {
        accepts_request(request);
    }

    let _: fn(axum::extract::Path<u64>) = accepts_axum_path;
    let _: fn(axum::extract::Query<u64>) = accepts_axum_query;
    let _: fn(axum::Json<u64>) = accepts_axum_json;
    let _: fn(axum::extract::Request) = accepts_axum_request;
    let _: fn(Header<headers::UserAgent>) = accepts_header::<headers::UserAgent>;
    let _: axum::Router = axum::Router::new();
}
