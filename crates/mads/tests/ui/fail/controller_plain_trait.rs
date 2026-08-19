trait PlainRoute {
    fn index(&self);
}

#[mads::controller(routes = [PlainRoute])]
struct Controller;

impl PlainRoute for Controller {
    fn index(&self) {}
}

fn main() {}
