#[mads::routes]
trait Routes<T> {
    #[mads::get("/")]
    async fn index(&self, value: T);
}

fn main() {}
