//! Confirms non-unit modules receive a focused diagnostic.

#[mads::module]
struct InvalidModule {
    enabled: bool,
}

fn main() {}
