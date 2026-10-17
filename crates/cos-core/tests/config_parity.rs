use serde_json::Value;

#[test]
fn fresh_defaults_match_each_upstream_platform() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/config-defaults.json"
    ))
    .unwrap();
    for (platform, expected) in cases.as_object().unwrap() {
        assert_eq!(&cos_core::fresh_config(platform), expected, "{platform}");
    }
}

#[test]
fn upstream_config_normalization_and_refusals_match() {
    let cases: Value =
        serde_json::from_str(include_str!("../../../fixtures/upstream/config-cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let result = cos_core::parse_config(case["input"].clone());
        assert_eq!(
            result.is_ok(),
            case["valid"].as_bool().unwrap(),
            "{}: {:?}",
            case["name"],
            result.as_ref().err()
        );
        if let Ok(actual) = result {
            assert_eq!(actual, case["expected"], "{}", case["name"]);
            assert_eq!(
                cos_core::config::migrate_loaded(actual),
                case["loaded"],
                "loaded {}",
                case["name"]
            );
        }
    }
}
