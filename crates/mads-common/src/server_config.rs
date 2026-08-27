//! Conventional configuration loading for the HTTP runtime.

use std::path::Path;

use mads_core::{Config, ConfigBuilder, DotenvSource, EnvSource, Result, TomlSource};

#[allow(dead_code)]
pub(crate) fn load_standard_config_from(root: &Path) -> Result<Config> {
    load_standard_config_from_with_environment(root, EnvSource::new("MADS_"))
}

pub(crate) fn load_standard_config_from_with_environment(
    root: &Path,
    environment: EnvSource,
) -> Result<Config> {
    ConfigBuilder::new()
        .dotenv(DotenvSource::optional(root.join(".env")))
        .source(TomlSource::optional(root.join("mads.toml")))
        .source(environment)
        .build()
}
