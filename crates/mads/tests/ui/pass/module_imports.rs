use mads::prelude::*;

#[module]
struct FeatureModule;

#[module(imports = [FeatureModule])]
struct AppModule;

fn assert_module<T: Module>() {}

fn main() {
    assert_module::<FeatureModule>();
    assert_module::<AppModule>();
}
