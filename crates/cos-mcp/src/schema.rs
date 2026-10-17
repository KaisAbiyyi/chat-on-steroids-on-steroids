use serde_json::Value;
pub fn validate(schema: &Value, value: &Value) -> Result<(), String> {
    walk(schema, value, "")
}
fn received(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
fn walk(s: &Value, v: &Value, path: &str) -> Result<(), String> {
    let expected = s["type"].as_str().unwrap_or("");
    let fail = |detail: String| {
        if path.is_empty() {
            detail
        } else {
            format!("{path}: {detail}")
        }
    };
    let ok = match expected {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "number" => v.as_f64().is_some_and(f64::is_finite),
        "integer" => v.as_f64().is_some_and(|x| x.is_finite() && x.fract() == 0.),
        "boolean" => v.is_boolean(),
        _ => true,
    };
    if !ok {
        return Err(fail(format!(
            "Invalid input: expected {expected}, received {}",
            received(v)
        )));
    }
    if let Some(values) = s["enum"].as_array()
        && !values.contains(v)
    {
        return Err(fail("Invalid option".into()));
    }
    if let Some(object) = v.as_object() {
        if let Some(required) = s["required"].as_array() {
            for field in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(field) {
                    return Err(fail(format!(
                        "{field}: Invalid input: expected {}, received undefined",
                        s["properties"][field]["type"].as_str().unwrap_or("value")
                    )));
                }
            }
        }
        for (key, val) in object {
            if s["properties"].get(key).is_none() {
                if s["additionalProperties"] == false {
                    return Err(fail(format!("Unrecognized key: \"{key}\"")));
                }
            } else {
                walk(
                    &s["properties"][key],
                    val,
                    &if path.is_empty() {
                        key.to_owned()
                    } else {
                        format!("{path}.{key}")
                    },
                )?;
            }
        }
    }
    if let Some(array) = v.as_array() {
        if s["minItems"]
            .as_u64()
            .is_some_and(|n| array.len() < (n as usize))
            || s["maxItems"]
                .as_u64()
                .is_some_and(|n| array.len() > (n as usize))
        {
            return Err(fail("Array length is outside the allowed range".into()));
        }
        for (i, val) in array.iter().enumerate() {
            walk(&s["items"], val, &format!("{path}.{i}"))?;
        }
    }
    if let Some(text) = v.as_str() {
        let length = cos_core::config::js_len(text);
        if s["minLength"]
            .as_u64()
            .is_some_and(|n| length < (n as usize))
            || s["maxLength"]
                .as_u64()
                .is_some_and(|n| length > (n as usize))
        {
            return Err(fail("String length is outside the allowed range".into()));
        }
    }
    if let Some(n) = v.as_f64()
        && (s["minimum"].as_f64().is_some_and(|m| n < m)
            || s["maximum"].as_f64().is_some_and(|m| n > m))
    {
        return Err(fail("Number is outside the allowed range".into()));
    }
    Ok(())
}
