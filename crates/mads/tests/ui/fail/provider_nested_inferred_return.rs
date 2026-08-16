//! Confirms provider attributes reject inferred types nested in result output.

#[mads::provider]
fn value() -> mads::core::Result<_> {
    Ok(String::new())
}

fn main() {}
