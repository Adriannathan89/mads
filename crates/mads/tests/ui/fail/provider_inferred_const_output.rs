//! Confirms provider attributes reject inferred output array lengths.

#[mads::provider]
fn value() -> [u8; _] {
    [0; 4]
}

fn main() {}
