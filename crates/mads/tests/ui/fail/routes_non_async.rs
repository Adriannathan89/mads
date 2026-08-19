#[mads::routes]
trait Routes {
    #[mads::get("/")]
    fn index(&self);
}

fn main() {}
