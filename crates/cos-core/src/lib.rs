//! CoS runtime contracts and durable foundation. UI and provider hosts are separate owners.
pub const UPSTREAM_COMMIT: &str = "024731d8083d92c46e8248d64e7e0b36ab0a470f";
pub const APP_VERSION: &str = "2.1.39";
pub const BRIDGE_PROTOCOL: u32 = 14;
pub mod browser_protocol;
pub mod browser_tools;
pub mod config;
pub mod logging;
pub mod native_browser_bridge;
pub mod persistence;
pub mod runtime;
pub mod sandbox;
pub mod secrets;

pub fn fresh_config(platform: &str) -> serde_json::Value {
    let defaults: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/config-defaults.json"
    ))
    .expect("frozen upstream defaults");
    defaults.get(platform).unwrap_or(&defaults["linux"]).clone()
}

pub fn parse_config(value: serde_json::Value) -> Result<serde_json::Value, String> {
    config::parse(value).map_err(|issues| {
        issues
            .iter()
            .map(|i| i.path.clone())
            .collect::<Vec<_>>()
            .join(", ")
    })
}
