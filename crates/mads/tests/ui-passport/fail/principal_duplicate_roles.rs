use mads::common::PassportPrincipal;

#[derive(PassportPrincipal)]
struct Principal {
    #[roles]
    primary_roles: Vec<String>,
    #[roles]
    inherited_roles: Vec<String>,
}

fn main() {}
