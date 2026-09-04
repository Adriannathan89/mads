use mads::prelude::*;

#[derive(Clone)]
struct MissingProvider;

#[mads::service]
struct NeedsMissing {
    _missing: MissingProvider,
}

#[mads::module]
struct InvalidGraphModule;

#[mads::main]
async fn main() -> Result<(), HttpRuntimeError> {
    Mads::run::<InvalidGraphModule>().await
}
