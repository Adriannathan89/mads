//! Verifies attribute expansion through a direct core dependency.

#[mads_core::module]
struct AppModule;

#[mads_core::repository]
struct Repository;

fn framework_result() -> mads_core::Result<()> {
    Ok(())
}

fn main() {
    let _ = framework_result;
}
