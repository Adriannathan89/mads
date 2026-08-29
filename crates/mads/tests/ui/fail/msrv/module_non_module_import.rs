use mads::module;

struct NotAModule;

#[module(imports = [NotAModule])]
struct AppModule;

fn main() {}
