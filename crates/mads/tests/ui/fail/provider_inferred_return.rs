//! Confirms provider attributes require a concrete return type.

#[mads::provider]
fn value() -> _ {
    String::new()
}

fn main() {}
