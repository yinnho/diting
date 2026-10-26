//! Workflow-directory resolution: `name`-addressed flows on disk and the
//! flow package's bundled recipes. Split from flow/mod.rs at v0.8.0
//! (god-file ratchet); the executor stays in mod.rs.

use serde_json::{Map, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------

/// Where on-disk `name`-addressed flows live: server-side assets, sibling
/// of the binary's working directory by default — drop a directory in to
/// deploy, no rebuild.
pub fn workflow_dir() -> PathBuf {
    std::env::var_os("DITING_WORKFLOW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("workflow"))
}

/// A workflow name is a single path segment of lowercase/digits/dashes —
/// anything else (dots, slashes, ..) is rejected before it ever reaches the
/// filesystem.
pub fn is_workflow_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Names of flows available to `name=`: every `workflow/<name>/flow.json`
/// under the workflow dir (discovery-by-error: an unknown name in runFlow
/// lists these).
pub fn available_workflows() -> Vec<String> {
    let names: Vec<String> = std::fs::read_dir(workflow_dir())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("flow.json").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_workflow_name(n))
        .collect();
    let mut names = names;
    names.sort();
    names
}

/// Resolve the flow document to run: inline, or by name from the workflow
/// directory.
pub fn resolve_flow_doc(flow: Option<Value>, name: Option<&str>) -> Result<Value, String> {
    if let Some(doc) = flow {
        if doc.get("steps").and_then(|s| s.as_array()).is_some() {
            return Ok(doc);
        }
        return Err("flow document has no steps array".into());
    }
    let Some(name) = name else {
        return Err(
            "pass either \"flow\" (inline document) or \"name\" (server-side workflow/<name>/flow.json)"
                .into(),
        );
    };
    if !is_workflow_name(name) {
        return Err(format!(
            "invalid workflow name {name:?} (lowercase/digits/dashes only)"
        ));
    }
    let path = workflow_dir().join(name).join("flow.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        let mut doc: Value = serde_json::from_str(&text)
            .map_err(|e| format!("workflow {name:?} is not valid JSON: {e}"))?;
        // Flow-bundled recipes: `recipes/<name>.js` siblings join the run
        // (the preload step resolves them when the builtin registry has no
        // such name). Local disk is the user's own package — reading it is
        // the same trust level as reading flow.json itself.
        let bundled = load_bundled_recipes(&workflow_dir().join(name).join("recipes"));
        if !bundled.is_empty() {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("__bundled_recipes".into(), Value::Object(bundled));
            }
        }
        return Ok(doc);
    }
    Err(format!(
        "workflow {name:?} not found at {} (available: {})",
        path.display(),
        available_workflows().join(", ")
    ))
}

/// Read a flow package's `recipes/` directory into the bundled-recipe map
/// (`<file stem> → source`). Missing dir → empty map; non-.js entries are
/// ignored so a stray README doesn't become a recipe.
fn load_bundled_recipes(dir: &std::path::Path) -> Map<String, Value> {
    let mut map = Map::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return map;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("js") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_workflow_name(stem) {
            continue;
        }
        if let Ok(src) = std::fs::read_to_string(&path) {
            map.insert(stem.to_string(), Value::String(src));
        }
    }
    map
}
