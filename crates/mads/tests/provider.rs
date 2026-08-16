//! Integration tests for explicit provider-function construction.

use mads::core::{Catalog, Config, ConfigBuilder, MADS003, Mads, MapSource};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConfiguredValue(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct CombinedValue {
    configured: String,
    entries: usize,
}

#[mads::provider]
fn configured_value(config: Config) -> ConfiguredValue {
    ConfiguredValue(
        config
            .get("application.name")
            .expect("the test configuration contains an application name")
            .to_owned(),
    )
}

#[mads::provider]
async fn combined_value(
    config: Config,
    configured: ConfiguredValue,
) -> mads::core::Result<CombinedValue> {
    Ok(CombinedValue {
        configured: configured.0,
        entries: config.len(),
    })
}

fn test_config() -> Config {
    ConfigBuilder::new()
        .source(MapSource::new(
            "test",
            [("application.name", "provider-test")],
        ))
        .build()
        .expect("the fixed test configuration should build")
}

#[test]
fn provider_dependencies_follow_parameter_order() {
    let descriptor = Catalog::provider_for::<CombinedValue>()
        .expect("the combined-value provider descriptor should be registered");
    let dependency_names: Vec<_> = descriptor
        .dependencies()
        .iter()
        .map(|dependency| dependency.type_name())
        .collect();

    assert_eq!(dependency_names, ["Config", "ConfiguredValue"]);
}

#[tokio::test]
async fn explicit_order_stores_direct_and_fallible_provider_outputs() {
    let mut builder = Mads::builder_with_config(test_config());
    builder
        .construct::<ConfiguredValue>()
        .await
        .expect("the direct provider should construct first");
    builder
        .construct::<CombinedValue>()
        .await
        .expect("the fallible provider should construct after its dependency");
    let application = builder.build();

    let configured = application
        .context()
        .resolve::<ConfiguredValue>()
        .expect("the direct output should be stored");
    let combined = application
        .context()
        .resolve::<CombinedValue>()
        .expect("the fallible output should be stored");

    assert_eq!(
        configured.as_ref(),
        &ConfiguredValue("provider-test".into())
    );
    assert_eq!(
        combined.as_ref(),
        &CombinedValue {
            configured: "provider-test".into(),
            entries: 1,
        }
    );
}

#[tokio::test]
async fn construct_does_not_recursively_build_a_missing_provider_dependency() {
    let mut builder = Mads::builder_with_config(test_config());

    let Err(error) = builder.construct::<CombinedValue>().await else {
        panic!("combined construction should require explicit configured-value construction");
    };

    assert_eq!(error.code(), MADS003);
}
