use mads::common::PassportPrincipal;

#[derive(PassportPrincipal)]
struct Principal {
    #[roles]
    roles: Vec<u64>,
}

fn main() {}
