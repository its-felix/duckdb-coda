use serde_json::Value;

use crate::constants::ATTACH_RESOURCE_PREFIXES;

pub(crate) fn is_browser_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

pub(crate) fn strip_attach_resource_prefix(value: &str) -> &str {
    ATTACH_RESOURCE_PREFIXES
        .iter()
        .find_map(|prefix| value.strip_prefix(prefix))
        .unwrap_or(value)
}

pub(crate) fn doc_id_from_browser_url(value: &str) -> Option<String> {
    let path_start = value.find("://").map(|index| index + 3)?;
    let path = &value[path_start..];
    let path = path.find('/').map(|index| &path[index..])?;
    let doc_segment = path.strip_prefix("/d/")?.split(['/', '?', '#']).next()?;
    let marker = doc_segment.rfind("_d")?;
    let doc_id = &doc_segment[marker + 2..];
    (!doc_id.is_empty()).then(|| doc_id.to_string())
}

pub(crate) fn doc_id_from_resolved_link(body: &str) -> Result<String, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("invalid ResolveBrowserLink response: {error}"))?;
    let resource = root
        .get("resource")
        .and_then(Value::as_object)
        .ok_or("ResolveBrowserLink response did not contain a resource")?;
    let resource_type = resource
        .get("type")
        .and_then(Value::as_str)
        .ok_or("ResolveBrowserLink resource did not contain a type")?;
    if resource_type.eq_ignore_ascii_case("doc") {
        return resource
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| "resolved doc resource did not contain an id".to_string());
    }

    let href = resource
        .get("href")
        .and_then(Value::as_str)
        .ok_or("resolved resource did not contain an API link")?;
    let marker = "/docs/";
    let start = href
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| {
            format!("resolved {resource_type} resource is not contained by a document")
        })?;
    let doc_id = href[start..].split(['/', '?', '#']).next().unwrap_or("");
    if doc_id.is_empty() {
        return Err(format!(
            "resolved {resource_type} resource API link did not contain a document id"
        ));
    }
    Ok(doc_id.to_string())
}
