#[mads::common::guard(strategy = "jwt", principal = UserPrincipal)]
async fn profile() {}

struct UserPrincipal;

fn main() {}
