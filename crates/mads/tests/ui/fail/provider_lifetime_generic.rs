//! Confirms provider attributes reject lifetime-generic functions.

#[mads::provider]
fn value<'value>() -> String {
    String::new()
}

fn main() {}
