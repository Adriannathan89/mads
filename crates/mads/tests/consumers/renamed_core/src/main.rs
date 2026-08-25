//! Verifies attribute expansion through a renamed direct-core dependency.

use runtime::AutoConfigurationStatus;

#[runtime::module]
struct AppModule;

#[runtime::repository]
struct Repository;

fn framework_result() -> runtime::Result<()> {
    Ok(())
}

fn status_name(status: AutoConfigurationStatus) -> &'static str {
    match status {
        AutoConfigurationStatus::Active => "active",
        AutoConfigurationStatus::Skipped => "skipped",
        AutoConfigurationStatus::Overridden => "overridden",
        AutoConfigurationStatus::Failed => "failed",
    }
}

fn main() {
    let _ = framework_result;
    let _ = status_name;
}
