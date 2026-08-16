//! Confirms provider attributes reject unsafe functions.

#[mads::provider]
unsafe fn value() -> String {
    String::new()
}

fn main() {}
