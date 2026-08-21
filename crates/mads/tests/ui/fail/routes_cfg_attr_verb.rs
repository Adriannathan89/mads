//! Rejects route verbs nested inside conditional attribute expansion.

#[mads::routes]
trait Routes {
    #[cfg_attr(feature = "conditional-route", mads::get("/conditional"))]
    async fn conditional(&self) -> &'static str;
}

fn main() {}
