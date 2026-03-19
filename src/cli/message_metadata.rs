use serde_json::{Map, Value};

use crate::error::{PlugboardError, Result};

pub fn parse_meta_args(entries: &[String]) -> Result<Vec<(String, Value)>> {
    entries.iter().map(|entry| parse_meta_arg(entry)).collect()
}

fn parse_meta_arg(entry: &str) -> Result<(String, Value)> {
    let Some((key, raw_value)) = entry.split_once('=') else {
        return Err(PlugboardError::InvalidMetadataArgument {
            input: entry.to_string(),
        });
    };
    if key.is_empty() {
        return Err(PlugboardError::InvalidMetadataArgument {
            input: entry.to_string(),
        });
    }

    let value = serde_json::from_str::<Value>(raw_value)
        .unwrap_or_else(|_| Value::String(raw_value.to_string()));
    Ok((key.to_string(), value))
}

pub fn merge_meta_into_metadata_json(
    existing: Option<&str>,
    meta: &[(String, Value)],
) -> Result<Option<String>> {
    if meta.is_empty() {
        return Ok(existing.map(str::to_string));
    }

    let mut root = match existing {
        Some(existing) => serde_json::from_str::<Value>(existing)?,
        None => Value::Object(Map::new()),
    };
    let Some(root_object) = root.as_object_mut() else {
        return Err(PlugboardError::InvalidMetadataJsonObject);
    };

    let mut meta_object = Map::new();
    for (key, value) in meta {
        meta_object.insert(key.clone(), value.clone());
    }
    root_object.insert("meta".into(), Value::Object(meta_object));
    Ok(Some(serde_json::to_string(&root)?))
}

#[cfg(test)]
mod tests {
    use super::{merge_meta_into_metadata_json, parse_meta_args};
    use serde_json::{Value, json};

    #[test]
    fn parses_multiple_meta_args_with_json_values() {
        let parsed = parse_meta_args(&[
            "model=llama3.2:3b".into(),
            "temperature=0.7".into(),
            "debug=true".into(),
        ])
        .unwrap();

        assert_eq!(parsed[0].0, "model");
        assert_eq!(parsed[0].1, json!("llama3.2:3b"));
        assert_eq!(parsed[1].1, json!(0.7));
        assert_eq!(parsed[2].1, json!(true));
    }

    #[test]
    fn rejects_invalid_meta_args() {
        assert!(parse_meta_args(&["missing_equals".into()]).is_err());
        assert!(parse_meta_args(&["=value".into()]).is_err());
    }

    #[test]
    fn merges_meta_under_top_level_field_without_overwriting_other_fields() {
        let merged = merge_meta_into_metadata_json(
            Some(r#"{"exit_code":0,"stdout":"ok"}"#),
            &[
                ("model".into(), json!("llama3.2:3b")),
                ("temperature".into(), json!(0.7)),
            ],
        )
        .unwrap()
        .unwrap();

        let parsed: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed["exit_code"], json!(0));
        assert_eq!(parsed["stdout"], json!("ok"));
        assert_eq!(parsed["meta"]["model"], json!("llama3.2:3b"));
        assert_eq!(parsed["meta"]["temperature"], json!(0.7));
    }
}
