//! Verifies attribute expansion through a renamed direct-core dependency.

#[runtime::module]
struct AppModule;

#[runtime::repository]
struct Repository;

fn framework_result() -> runtime::Result<()> {
    Ok(())
}

fn main() {
    let _ = framework_result;
}
