//! Shared take/apply for `[[version_overrides]]` / `[[campaigns]]` arrays.

use serde::de::DeserializeOwned;

use crate::deep_merge_toml;

pub type PatchPath = &'static [&'static str];

#[derive(Debug, Clone)]
pub struct ConfigOverrideEntry<M> {
    pub meta: M,
    pub patch: toml::Table,
}

/// Strip `key` from the root table; each element is `M` + remaining keys as patch.
pub fn take_patch_array<M>(
    config: &mut toml::Value,
    key: &str,
) -> Result<Vec<ConfigOverrideEntry<M>>, toml::de::Error>
where
    M: DeserializeOwned,
{
    let Some(table) = config.as_table_mut() else {
        return Ok(Vec::new());
    };
    let Some(array_value) = table.remove(key) else {
        return Ok(Vec::new());
    };

    #[derive(serde::Deserialize)]
    struct FlatEntry<M> {
        #[serde(flatten)]
        meta: M,
        #[serde(flatten)]
        patch: toml::Table,
    }

    let entries: Vec<FlatEntry<M>> = array_value.try_into()?;
    Ok(entries
        .into_iter()
        .map(|e| ConfigOverrideEntry {
            meta: e.meta,
            patch: e.patch,
        })
        .collect())
}

/// Whether `patch` affects the value at `path`: it sets a value there (any leaf
/// under it counts), **or** it sets a non-table ancestor — deep-merge replaces
/// the whole subtree in that case, so every leaf beneath is touched (a patch
/// like `models = "oops"` wipes `models.default` and must still be dismissable
/// / flagged as driving it).
pub fn patch_touches_path(patch: &toml::Table, path: PatchPath) -> bool {
    let Some(first) = path.first() else {
        return false;
    };
    let Some(mut cur) = patch.get(*first) else {
        return false;
    };
    for seg in path.iter().skip(1) {
        match cur.as_table() {
            Some(t) => match t.get(*seg) {
                Some(v) => cur = v,
                None => return false,
            },
            // Non-table ancestor: the merge replaces this subtree wholesale.
            None => return true,
        }
    }
    true
}

/// Whether `patch` touches any of `paths`.
pub fn patch_touches_any(patch: &toml::Table, paths: &[PatchPath]) -> bool {
    paths.iter().any(|p| patch_touches_path(patch, p))
}

/// Keys stripped from every applied patch: an override cannot re-inject nested
/// `version_overrides`/`campaigns` or define `[auth_provider.*]` /
/// `[model_providers.*]` command tables, nor switch retrieval route tables
/// (`embedding_models` / `reranker_models` / `retrieval_profiles` / `prime`).
///
/// Memory route-bearing selection (`memory.retrieval_profile`) is fail-closed
/// via [`strip_memory_retrieval_route`] after deep-merge rather than a top-level
/// strip alone (because `memory` also carries non-route fields).
pub const PATCH_STRIP_KEYS: &[&str] = &[
    "version_overrides",
    "campaigns",
    "auth_provider",
    "model_providers",
    "embedding_models",
    "reranker_models",
    "retrieval_profiles",
    "prime",
];

/// Remove route-bearing `memory.retrieval_profile` from a remote/campaign patch
/// **before** merge so untrusted patches cannot switch memory retrieval routing
/// while ordinary trusted disk/managed `[memory]` config remains writable.
pub fn strip_memory_retrieval_route(patch: &mut toml::Table) {
    if let Some(mem) = patch.get_mut("memory").and_then(|v| v.as_table_mut()) {
        mem.remove("retrieval_profile");
        // Nested tables under memory that only existed for the route key can stay;
        // empty memory table is fine.
    }
}

/// Deep-merge each patch in iteration order (later wins on a leaf), stripping
/// `strip_keys` (top level) first, then stripping memory retrieval-route
/// selection so campaign/version patches cannot switch named memory profiles.
pub fn apply_patches(
    config: &mut toml::Value,
    patches: impl IntoIterator<Item = toml::Table>,
    strip_keys: &[&str],
) {
    for mut patch in patches {
        for key in strip_keys {
            patch.remove(*key);
        }
        strip_memory_retrieval_route(&mut patch);
        deep_merge_toml(config, &toml::Value::Table(patch));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(s: &str) -> toml::Table {
        toml::from_str(s).unwrap()
    }

    /// A patch that replaces a parent table with a scalar (`models = "oops"`)
    /// wipes every leaf beneath it on merge, so it must count as touching those
    /// leaves — otherwise the campaign that destroyed `models.default` would be
    /// neither dismissable nor flagged as driving the field.
    #[test]
    fn non_table_ancestor_counts_as_touching_leaves_beneath() {
        let patch = table("models = \"oops\"\n");
        assert!(patch_touches_path(&patch, &["models", "default"]));
        assert!(patch_touches_path(&patch, &["models"]));
        // Sibling sections are unaffected.
        assert!(!patch_touches_path(&patch, &["features", "campaigns"]));
        // A well-formed table patch still requires the leaf to be present.
        let tbl = table("[models]\ndefault = \"m\"\n");
        assert!(patch_touches_path(&tbl, &["models", "default"]));
        assert!(!patch_touches_path(&tbl, &["models", "other"]));
    }

    #[test]
    fn apply_patches_strips_requested_keys() {
        let mut cfg = toml::Value::Table(table("[models]\ndefault = \"old\"\n"));
        let patch = table("[models]\ndefault = \"new\"\n");
        apply_patches(&mut cfg, std::iter::once(patch), PATCH_STRIP_KEYS);
        assert_eq!(cfg["models"]["default"].as_str(), Some("new"));

        // Top-level strip keys are removed before merge.
        let mut cfg2 = toml::Value::Table(toml::Table::new());
        let mut p = toml::Table::new();
        p.insert("version_overrides".into(), toml::Value::Array(vec![]));
        p.insert("campaigns".into(), toml::Value::Array(vec![]));
        p.insert(
            "auth_provider".into(),
            toml::Value::Table(toml::Table::new()),
        );
        p.insert(
            "model_providers".into(),
            toml::Value::Table(toml::Table::new()),
        );
        p.insert("keep".into(), toml::Value::Boolean(true));
        apply_patches(&mut cfg2, std::iter::once(p), PATCH_STRIP_KEYS);
        assert!(cfg2.get("version_overrides").is_none());
        assert!(cfg2.get("campaigns").is_none());
        assert!(cfg2.get("auth_provider").is_none());
        assert!(cfg2.get("model_providers").is_none());
        assert_eq!(cfg2["keep"].as_bool(), Some(true));

        // Top-level strip only: a model may still reference a local provider by name.
        let mut cfg3 = toml::Value::Table(toml::Table::new());
        let p = table(
            "[auth_provider.injected]\ncommand = \"evil\"\n\
             [model_providers.injected]\nbase_url = \"https://evil.example/v1\"\n\
             [model.x]\nauth_provider = \"local-name\"\nmodel_provider = \"local-provider\"\n",
        );
        apply_patches(&mut cfg3, std::iter::once(p), PATCH_STRIP_KEYS);
        assert!(cfg3.get("auth_provider").is_none());
        assert!(cfg3.get("model_providers").is_none());
        assert_eq!(
            cfg3["model"]["x"]["auth_provider"].as_str(),
            Some("local-name")
        );
        assert_eq!(
            cfg3["model"]["x"]["model_provider"].as_str(),
            Some("local-provider")
        );
    }

    #[test]
    fn apply_patches_strips_retrieval_route_tables() {
        let mut cfg = toml::Value::Table(table(
            r#"
[embedding_models.e1]
provider = "openai"
model = "m"
"#,
        ));
        let p = table(
            r#"
[embedding_models.evil]
provider = "attacker"
model = "x"
[reranker_models.evil]
provider = "attacker"
model = "x"
[retrieval_profiles.evil]
embedding_models = ["evil"]
[prime.skills]
enabled = true
retrieval_profile = "evil"
"#,
        );
        apply_patches(&mut cfg, std::iter::once(p), PATCH_STRIP_KEYS);
        // Original trusted embedding survives; injection stripped.
        assert!(
            cfg.get("embedding_models")
                .and_then(|e| e.get("e1"))
                .is_some()
        );
        assert!(
            cfg.get("embedding_models")
                .and_then(|e| e.get("evil"))
                .is_none()
        );
        assert!(cfg.get("reranker_models").is_none());
        assert!(cfg.get("retrieval_profiles").is_none());
        assert!(cfg.get("prime").is_none());
    }

    #[test]
    fn apply_patches_strips_memory_retrieval_profile_route() {
        let mut cfg = toml::Value::Table(table(
            r#"
[memory]
enabled = true
retrieval_profile = "trusted"
[memory.search]
max_results = 6
"#,
        ));
        let p = table(
            r#"
[memory]
retrieval_profile = "evil"
enabled = false
"#,
        );
        apply_patches(&mut cfg, std::iter::once(p), PATCH_STRIP_KEYS);
        // Route selection cannot switch via remote patch.
        assert_eq!(cfg["memory"]["retrieval_profile"].as_str(), Some("trusted"));
        // Non-route memory fields from the patch may still apply (enabled).
        assert_eq!(cfg["memory"]["enabled"].as_bool(), Some(false));
    }
}
