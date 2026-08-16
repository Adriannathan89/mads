//! Confirms nested `Self` field types refer to the generated public handle.

#[mads::service]
struct RecursiveService {
    parent: Box<Self>,
}

impl RecursiveService {
    fn parent(&self) -> &Self {
        &self.parent
    }
}

fn main() {
    let _method: fn(&RecursiveService) -> &RecursiveService = RecursiveService::parent;
}
