//! Public extractor surface tests.

use mads_common::{Header, Json, Path, Query, Request, axum, headers};

#[test]
fn common_exports_standard_http_types() {
    fn accepts_path<T>(_: Option<Path<T>>) {}
    fn accepts_query<T>(_: Option<Query<T>>) {}
    fn accepts_json<T>(_: Option<Json<T>>) {}
    fn accepts_header<T: headers::Header>(_: Option<Header<T>>) {}
    fn accepts_request(_: Option<Request>) {}

    accepts_path::<u64>(None);
    accepts_query::<u64>(None);
    accepts_json::<u64>(None);
    accepts_header::<headers::UserAgent>(None);
    accepts_request(None);
    let _: axum::Router = axum::Router::new();
}
