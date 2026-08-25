use mads::common::passport_strategy;

struct NotAStrategy;

#[passport_strategy(name = "jwt")]
impl NotAStrategy {
    fn validate(&self) {}
}

fn main() {}
