//! Confirms generic services receive a focused diagnostic.

#[mads::service]
struct GenericService<T> {
    value: T,
}

fn main() {}
