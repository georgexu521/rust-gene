//! Workflow contract sanitation support.
//!
//! Keeps workflow feedback safe to render and feed back into the model.

pub(super) fn sanitize_workflow_judgment_value(value: &mut serde_json::Value) {
    let Some(triggers) = value
        .get_mut("guided_reasoning_triggers")
        .and_then(|value| value.as_array_mut())
    else {
        return;
    };

    let mut normalized = Vec::new();
    for trigger in triggers.iter() {
        let raw = trigger.as_str().unwrap_or_default();
        let mapped = normalize_guided_reasoning_trigger(raw);
        if !normalized.iter().any(|existing| existing == mapped) {
            normalized.push(mapped.to_string());
        }
    }

    *triggers = normalized
        .into_iter()
        .map(serde_json::Value::String)
        .collect();
}

pub(super) fn sanitize_acceptance_review_value(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    if !object
        .get("accepted")
        .is_some_and(|value| value.is_boolean())
    {
        object.insert("accepted".to_string(), serde_json::Value::Bool(false));
    }
    normalize_enum_field(
        object,
        "confidence",
        &["low", "medium", "high"],
        "medium",
        &["confidence", "level", "status"],
    );
    normalize_enum_field(
        object,
        "next_action",
        &["finish", "continue_repair", "ask_user", "stop"],
        "continue_repair",
        &["next_action", "action", "recommendation"],
    );
    sanitize_acceptance_criteria(object);
    sanitize_string_array_field(
        object,
        "unresolved_items",
        &["item", "issue", "reason", "criterion", "message", "summary"],
    );
    sanitize_string_array_field(
        object,
        "residual_risks",
        &["risk", "reason", "item", "message", "summary"],
    );
}

fn sanitize_acceptance_criteria(object: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(criteria) = object.get_mut("criteria") else {
        object.insert("criteria".to_string(), serde_json::Value::Array(Vec::new()));
        return;
    };

    if criteria.is_object() {
        *criteria = serde_json::Value::Array(vec![criteria.take()]);
    }

    let Some(items) = criteria.as_array_mut() else {
        *criteria = serde_json::Value::Array(Vec::new());
        return;
    };

    for item in items.iter_mut() {
        if !item.is_object() {
            let criterion = json_value_to_text(
                item,
                &["criterion", "text", "description", "message", "summary"],
            );
            *item = serde_json::json!({
                "criterion": criterion,
                "status": "not_verified",
                "evidence": null
            });
        }

        if let Some(criterion) = item.as_object_mut() {
            normalize_string_field(
                criterion,
                "criterion",
                "Unspecified acceptance criterion",
                &["criterion", "text", "description", "message", "summary"],
            );
            normalize_enum_field(
                criterion,
                "status",
                &["pending", "passed", "failed", "not_verified"],
                "not_verified",
                &["status", "state", "result"],
            );
            if let Some(evidence) = criterion.get_mut("evidence") {
                if evidence.is_null() {
                    continue;
                }
                let normalized = json_value_to_text(
                    evidence,
                    &[
                        "evidence", "message", "reason", "command", "output", "summary",
                    ],
                );
                *evidence = if normalized.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String(normalized)
                };
            }
        }
    }
}

fn normalize_string_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    fallback: &str,
    preferred_fields: &[&str],
) {
    let normalized = object
        .get(key)
        .map(|value| json_value_to_text(value, preferred_fields))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string());
    object.insert(key.to_string(), serde_json::Value::String(normalized));
}

fn normalize_enum_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    allowed: &[&str],
    fallback: &str,
    preferred_fields: &[&str],
) {
    let normalized = object
        .get(key)
        .map(|value| normalize_enum_token(&json_value_to_text(value, preferred_fields)))
        .filter(|value| allowed.iter().any(|allowed| allowed == value))
        .unwrap_or_else(|| fallback.to_string());
    object.insert(key.to_string(), serde_json::Value::String(normalized));
}

fn sanitize_string_array_field(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    preferred_fields: &[&str],
) {
    let values = match object.get_mut(key) {
        Some(value) if value.is_array() => value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| json_value_to_text(item, preferred_fields))
                    .filter(|item| !item.trim().is_empty())
                    .map(serde_json::Value::String)
                    .collect()
            })
            .unwrap_or_default(),
        Some(value) if value.is_null() => Vec::new(),
        Some(value) => {
            let item = json_value_to_text(value, preferred_fields);
            if item.trim().is_empty() {
                Vec::new()
            } else {
                vec![serde_json::Value::String(item)]
            }
        }
        None => Vec::new(),
    };
    object.insert(key.to_string(), serde_json::Value::Array(values));
}

fn json_value_to_text(value: &serde_json::Value, preferred_fields: &[&str]) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(value) => value.trim().to_string(),
        serde_json::Value::Array(items) => {
            let joined = items
                .iter()
                .map(|item| json_value_to_text(item, preferred_fields))
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            if joined.is_empty() {
                value.to_string()
            } else {
                joined
            }
        }
        serde_json::Value::Object(object) => {
            for field in preferred_fields {
                if let Some(nested) = object.get(*field) {
                    let text = json_value_to_text(nested, preferred_fields);
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
            }
            value.to_string()
        }
        _ => value.to_string(),
    }
}

fn normalize_enum_token(raw: &str) -> String {
    raw.trim()
        .trim_matches('"')
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
}

fn normalize_guided_reasoning_trigger(raw: &str) -> &'static str {
    let normalized = raw.trim().to_ascii_lowercase().replace([' ', '-'], "_");

    match normalized.as_str() {
        "ambiguous_requirement" => "ambiguous_requirement",
        "competing_approaches" => "competing_approaches",
        "high_risk_area" => "high_risk_area",
        "unfamiliar_code_path" => "unfamiliar_code_path",
        "tool_failure" => "tool_failure",
        "test_failure" => "test_failure",
        "unexpected_diff" => "unexpected_diff",
        "repeated_repair" => "repeated_repair",
        "goal_drift" => "goal_drift",
        "context_conflict" => "context_conflict",
        "broad_product_request" => "broad_product_request",
        _ => {
            if normalized.contains("test") {
                "test_failure"
            } else if normalized.contains("tool") {
                "tool_failure"
            } else if normalized.contains("risk") || normalized.contains("danger") {
                "high_risk_area"
            } else if normalized.contains("approach") || normalized.contains("alternative") {
                "competing_approaches"
            } else if normalized.contains("ambiguous")
                || normalized.contains("unclear")
                || normalized.contains("requirement")
            {
                "ambiguous_requirement"
            } else if normalized.contains("drift") {
                "goal_drift"
            } else if normalized.contains("context") || normalized.contains("conflict") {
                "context_conflict"
            } else {
                "unfamiliar_code_path"
            }
        }
    }
}
