//! Confirms `Self` paths in field-type const expressions target the public handle.

#[mads::service]
struct BufferedService {
    bytes: [u8; Self::CAPACITY],
}

impl BufferedService {
    const CAPACITY: usize = 4;

    fn capacity(&self) -> usize {
        self.bytes.len()
    }
}

fn main() {
    let _method: fn(&BufferedService) -> usize = BufferedService::capacity;
}
