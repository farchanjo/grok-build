//! Resilient parse of named retrieval config sections.
//!
//! Mirrors `[model_providers.<id>]` isolation:
//! - non-table section disables only that section and emits a structured warning;
//! - non-table/malformed entry drops only that entry; valid siblings survive;
//! - unknown fields are ignored (forward compatibility) and preserved by management writes.

use crate::agent::config_model_override_parse::{ConfigWarning, ConfigWarningKind};
use indexmap::IndexMap;
use xai_grok_config_types::{
    AgentPrimeConfig, DEFAULT_DEADLINE_MS, DEFAULT_EMBEDDING_BATCH_SIZE, DEFAULT_MAX_ATTEMPTS,
    DEFAULT_MAX_CANDIDATES, DEFAULT_MAX_INPUT_TOKENS, DEFAULT_MAX_OUTPUT_TOKENS,
    DEFAULT_MAX_RESULTS, EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol, MAX_ATTEMPTS,
    MAX_BATCH_SIZE, MAX_DEADLINE_MS, MAX_EMBEDDING_DIMENSIONS, MAX_INPUT_TOKENS_BOUND,
    MAX_RESULT_LIMIT, MIN_EMBEDDING_DIMENSIONS, PrimeConfig, RerankerModelConfig, RerankerProtocol,
    RetrievalFallbackStrategy, RetrievalGraphConfig, RetrievalProfileConfig, SkillPrimeConfig,
    clamp_context_fraction, clamp_unit_score, normalize_retrieval_id, validate_relative_endpoint,
};

/// Result of resilient retrieval-graph parse (no cross-ref / provider checks).
#[derive(Debug, Clone, Default)]
pub struct ParsedRetrievalGraph {
    pub graph: RetrievalGraphConfig,
    pub warnings: Vec<ConfigWarning>,
}

/// Parse all retrieval-related sections from a raw TOML config value.
pub fn parse_retrieval_graph(raw_config: &toml::Value) -> ParsedRetrievalGraph {
    let mut warnings = Vec::new();
    let embedding_models = parse_embedding_models(raw_config, &mut warnings);
    let reranker_models = parse_reranker_models(raw_config, &mut warnings);
    let retrieval_profiles = parse_retrieval_profiles(raw_config, &mut warnings);
    let prime = parse_prime(raw_config, &mut warnings);
    let memory_retrieval_profile = parse_memory_retrieval_profile(raw_config, &mut warnings);
    let memory_mode = parse_memory_mode(raw_config);
    let memory_vector_store = parse_memory_vector_store(raw_config);
    ParsedRetrievalGraph {
        graph: RetrievalGraphConfig {
            embedding_models,
            reranker_models,
            retrieval_profiles,
            prime,
            memory_retrieval_profile,
            memory_mode,
            memory_vector_store,
        },
        warnings,
    }
}

fn parse_embedding_models(
    raw: &toml::Value,
    warnings: &mut Vec<ConfigWarning>,
) -> IndexMap<String, EmbeddingModelConfig> {
    let mut out = IndexMap::new();
    let Some(section) = raw.get("embedding_models") else {
        return out;
    };
    let Some(table) = section.as_table() else {
        warnings.push(ConfigWarning::embedding_models_section(
            ConfigWarningKind::NotATable,
            format!(
                "`embedding_models` must be a table of [embedding_models.<id>] entries, got {}; \
                 all embedding models ignored",
                section.type_str()
            ),
        ));
        return out;
    };
    for (raw_id, value) in table {
        let id = match normalize_retrieval_id(raw_id) {
            Ok(id) => id,
            Err(err) => {
                warnings.push(ConfigWarning::embedding_model(
                    raw_id,
                    None,
                    ConfigWarningKind::InvalidValue,
                    format!("invalid embedding model id ({err}); entry dropped"),
                ));
                continue;
            }
        };
        let Some(entry) = value.as_table() else {
            warnings.push(ConfigWarning::embedding_model(
                &id,
                None,
                ConfigWarningKind::NotATable,
                format!(
                    "expected a table like [embedding_models.\"{id}\"], got {}; entry dropped",
                    value.type_str()
                ),
            ));
            continue;
        };
        match parse_embedding_entry(&id, entry, warnings) {
            Some(cfg) => {
                if out.contains_key(&id) {
                    warnings.push(ConfigWarning::embedding_model(
                        &id,
                        None,
                        ConfigWarningKind::ConflictingFields,
                        format!(
                            "duplicate embedding model id after normalize (`{id}`); later entry \
                             dropped to avoid silent overwrite"
                        ),
                    ));
                } else {
                    out.insert(id, cfg);
                }
            }
            None => {
                // warning already emitted
            }
        }
    }
    out
}

fn parse_embedding_entry(
    id: &str,
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<EmbeddingModelConfig> {
    let mut unknown = Vec::new();
    let mut cfg = EmbeddingModelConfig::default();
    // Known keys; unknown collected for forward-compat warning.
    let known = [
        "provider",
        "model",
        "protocol",
        "dimensions",
        "encoding",
        "batch_size",
        "max_input_tokens",
    ];
    for (k, v) in table {
        if !known.contains(&k.as_str()) {
            unknown.push(k.clone());
            continue;
        }
        match k.as_str() {
            "provider" => match v.as_str() {
                Some(s) if !s.trim().is_empty() => cfg.provider = s.trim().to_owned(),
                Some(_) => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("provider"),
                    ConfigWarningKind::InvalidValue,
                    "provider must be non-empty; field ignored".into(),
                )),
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("provider"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "provider must be a string, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "model" => match v.as_str() {
                Some(s) if !s.trim().is_empty() => cfg.model = s.trim().to_owned(),
                Some(_) => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("model"),
                    ConfigWarningKind::InvalidValue,
                    "model must be non-empty; field ignored".into(),
                )),
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("model"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "model must be a string, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "protocol" => match v.as_str().and_then(parse_embedding_protocol) {
                Some(p) => cfg.protocol = p,
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("protocol"),
                    ConfigWarningKind::InvalidValue,
                    "unrecognized embedding protocol; only registered typed protocols are \
                     accepted (openai_compatible); field ignored"
                        .into(),
                )),
            },
            "dimensions" => match as_u32(v) {
                Some(d) if (MIN_EMBEDDING_DIMENSIONS..=MAX_EMBEDDING_DIMENSIONS).contains(&d) => {
                    cfg.dimensions = Some(d);
                }
                Some(d) => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("dimensions"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "dimensions {d} out of bounds \
                         ({MIN_EMBEDDING_DIMENSIONS}..={MAX_EMBEDDING_DIMENSIONS}); field ignored"
                    ),
                )),
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("dimensions"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "dimensions must be an integer, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "encoding" => match v.as_str().and_then(parse_embedding_encoding) {
                Some(e) => cfg.encoding = e,
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("encoding"),
                    ConfigWarningKind::InvalidValue,
                    "unrecognized encoding; expected float or base64; field ignored".into(),
                )),
            },
            "batch_size" => match as_u32(v) {
                Some(b) if (1..=MAX_BATCH_SIZE).contains(&b) => cfg.batch_size = b,
                Some(b) => {
                    warnings.push(ConfigWarning::embedding_model(
                        id,
                        Some("batch_size"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "batch_size {b} out of bounds (1..={MAX_BATCH_SIZE}); using default"
                        ),
                    ));
                    cfg.batch_size = DEFAULT_EMBEDDING_BATCH_SIZE;
                }
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("batch_size"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "batch_size must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_input_tokens" => match as_u32(v) {
                Some(t) if (1..=MAX_INPUT_TOKENS_BOUND).contains(&t) => cfg.max_input_tokens = t,
                Some(t) => {
                    warnings.push(ConfigWarning::embedding_model(
                        id,
                        Some("max_input_tokens"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_input_tokens {t} out of bounds (1..={MAX_INPUT_TOKENS_BOUND}); \
                             using default"
                        ),
                    ));
                    cfg.max_input_tokens = DEFAULT_MAX_INPUT_TOKENS;
                }
                None => warnings.push(ConfigWarning::embedding_model(
                    id,
                    Some("max_input_tokens"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_input_tokens must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            _ => {}
        }
    }
    for key in unknown {
        warnings.push(ConfigWarning::embedding_model(
            id,
            Some(key.as_str()),
            ConfigWarningKind::UnknownField,
            "unrecognized key; field ignored".into(),
        ));
    }
    // Required fields for a usable entry.
    if cfg.provider.is_empty() || cfg.model.is_empty() {
        warnings.push(ConfigWarning::embedding_model(
            id,
            None,
            ConfigWarningKind::InvalidValue,
            "provider and model are required and must be non-empty; entry dropped".into(),
        ));
        return None;
    }
    Some(cfg)
}

fn parse_reranker_models(
    raw: &toml::Value,
    warnings: &mut Vec<ConfigWarning>,
) -> IndexMap<String, RerankerModelConfig> {
    let mut out = IndexMap::new();
    let Some(section) = raw.get("reranker_models") else {
        return out;
    };
    let Some(table) = section.as_table() else {
        warnings.push(ConfigWarning::reranker_models_section(
            ConfigWarningKind::NotATable,
            format!(
                "`reranker_models` must be a table of [reranker_models.<id>] entries, got {}; \
                 all reranker models ignored",
                section.type_str()
            ),
        ));
        return out;
    };
    for (raw_id, value) in table {
        let id = match normalize_retrieval_id(raw_id) {
            Ok(id) => id,
            Err(err) => {
                warnings.push(ConfigWarning::reranker_model(
                    raw_id,
                    None,
                    ConfigWarningKind::InvalidValue,
                    format!("invalid reranker model id ({err}); entry dropped"),
                ));
                continue;
            }
        };
        let Some(entry) = value.as_table() else {
            warnings.push(ConfigWarning::reranker_model(
                &id,
                None,
                ConfigWarningKind::NotATable,
                format!(
                    "expected a table like [reranker_models.\"{id}\"], got {}; entry dropped",
                    value.type_str()
                ),
            ));
            continue;
        };
        if let Some(cfg) = parse_reranker_entry(&id, entry, warnings) {
            if out.contains_key(&id) {
                warnings.push(ConfigWarning::reranker_model(
                    &id,
                    None,
                    ConfigWarningKind::ConflictingFields,
                    format!(
                        "duplicate reranker model id after normalize (`{id}`); later entry \
                         dropped to avoid silent overwrite"
                    ),
                ));
            } else {
                out.insert(id, cfg);
            }
        }
    }
    out
}

fn parse_reranker_entry(
    id: &str,
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<RerankerModelConfig> {
    let mut unknown = Vec::new();
    let mut cfg = RerankerModelConfig::default();
    let known = [
        "provider",
        "model",
        "protocol",
        "endpoint",
        "batch_size",
        "max_input_tokens",
    ];
    for (k, v) in table {
        if !known.contains(&k.as_str()) {
            unknown.push(k.clone());
            continue;
        }
        match k.as_str() {
            "provider" => match v.as_str() {
                Some(s) if !s.trim().is_empty() => cfg.provider = s.trim().to_owned(),
                _ => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("provider"),
                    ConfigWarningKind::InvalidValue,
                    "provider must be a non-empty string; field ignored".into(),
                )),
            },
            "model" => match v.as_str() {
                Some(s) if !s.trim().is_empty() => cfg.model = s.trim().to_owned(),
                _ => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("model"),
                    ConfigWarningKind::InvalidValue,
                    "model must be a non-empty string; field ignored".into(),
                )),
            },
            "protocol" => match v.as_str().and_then(parse_reranker_protocol) {
                Some(p) => cfg.protocol = p,
                None => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("protocol"),
                    ConfigWarningKind::InvalidValue,
                    "unrecognized reranker protocol; only registered typed protocols are \
                     accepted (openai_compatible, cohere_compatible); field ignored"
                        .into(),
                )),
            },
            "endpoint" => match v.as_str() {
                Some(s) => match validate_relative_endpoint(s) {
                    Ok(()) => cfg.endpoint = Some(s.trim().to_owned()),
                    Err(err) => warnings.push(ConfigWarning::reranker_model(
                        id,
                        Some("endpoint"),
                        ConfigWarningKind::InvalidValue,
                        format!("{err}; field ignored"),
                    )),
                },
                None => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("endpoint"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "endpoint must be a string, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "batch_size" => match as_u32(v) {
                Some(b) if (1..=MAX_BATCH_SIZE).contains(&b) => cfg.batch_size = b,
                Some(b) => {
                    warnings.push(ConfigWarning::reranker_model(
                        id,
                        Some("batch_size"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "batch_size {b} out of bounds (1..={MAX_BATCH_SIZE}); using default"
                        ),
                    ));
                    cfg.batch_size = DEFAULT_EMBEDDING_BATCH_SIZE;
                }
                None => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("batch_size"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "batch_size must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_input_tokens" => match as_u32(v) {
                Some(t) if (1..=MAX_INPUT_TOKENS_BOUND).contains(&t) => cfg.max_input_tokens = t,
                Some(t) => {
                    warnings.push(ConfigWarning::reranker_model(
                        id,
                        Some("max_input_tokens"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_input_tokens {t} out of bounds (1..={MAX_INPUT_TOKENS_BOUND}); \
                             using default"
                        ),
                    ));
                    cfg.max_input_tokens = DEFAULT_MAX_INPUT_TOKENS;
                }
                None => warnings.push(ConfigWarning::reranker_model(
                    id,
                    Some("max_input_tokens"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_input_tokens must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            _ => {}
        }
    }
    for key in unknown {
        warnings.push(ConfigWarning::reranker_model(
            id,
            Some(key.as_str()),
            ConfigWarningKind::UnknownField,
            "unrecognized key; field ignored".into(),
        ));
    }
    if cfg.provider.is_empty() || cfg.model.is_empty() {
        warnings.push(ConfigWarning::reranker_model(
            id,
            None,
            ConfigWarningKind::InvalidValue,
            "provider and model are required and must be non-empty; entry dropped".into(),
        ));
        return None;
    }
    Some(cfg)
}

fn parse_retrieval_profiles(
    raw: &toml::Value,
    warnings: &mut Vec<ConfigWarning>,
) -> IndexMap<String, RetrievalProfileConfig> {
    let mut out = IndexMap::new();
    let Some(section) = raw.get("retrieval_profiles") else {
        return out;
    };
    let Some(table) = section.as_table() else {
        warnings.push(ConfigWarning::retrieval_profiles_section(
            ConfigWarningKind::NotATable,
            format!(
                "`retrieval_profiles` must be a table of [retrieval_profiles.<id>] entries, got \
                 {}; all profiles ignored",
                section.type_str()
            ),
        ));
        return out;
    };
    for (raw_id, value) in table {
        let id = match normalize_retrieval_id(raw_id) {
            Ok(id) => id,
            Err(err) => {
                warnings.push(ConfigWarning::retrieval_profile(
                    raw_id,
                    None,
                    ConfigWarningKind::InvalidValue,
                    format!("invalid retrieval profile id ({err}); entry dropped"),
                ));
                continue;
            }
        };
        let Some(entry) = value.as_table() else {
            warnings.push(ConfigWarning::retrieval_profile(
                &id,
                None,
                ConfigWarningKind::NotATable,
                format!(
                    "expected a table like [retrieval_profiles.\"{id}\"], got {}; entry dropped",
                    value.type_str()
                ),
            ));
            continue;
        };
        if let Some(cfg) = parse_profile_entry(&id, entry, warnings) {
            if out.contains_key(&id) {
                warnings.push(ConfigWarning::retrieval_profile(
                    &id,
                    None,
                    ConfigWarningKind::ConflictingFields,
                    format!(
                        "duplicate retrieval profile id after normalize (`{id}`); later entry \
                         dropped to avoid silent overwrite"
                    ),
                ));
            } else {
                out.insert(id, cfg);
            }
        }
    }
    out
}

fn parse_profile_entry(
    id: &str,
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<RetrievalProfileConfig> {
    let mut unknown = Vec::new();
    let mut cfg = RetrievalProfileConfig::default();
    let known = [
        "embedding_models",
        "reranker_models",
        "fallback_strategy",
        "max_candidates",
        "max_results",
        "min_score",
        "deadline_ms",
        "max_attempts",
        "max_input_tokens",
        "max_output_tokens",
    ];
    for (k, v) in table {
        if !known.contains(&k.as_str()) {
            unknown.push(k.clone());
            continue;
        }
        match k.as_str() {
            "embedding_models" => match as_string_array(v) {
                Some(ids) => cfg.embedding_models = ids,
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("embedding_models"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "embedding_models must be an array of strings, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "reranker_models" => match as_string_array(v) {
                Some(ids) => cfg.reranker_models = ids,
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("reranker_models"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "reranker_models must be an array of strings, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "fallback_strategy" => match v.as_str() {
                Some("deterministic") => {
                    cfg.fallback_strategy = RetrievalFallbackStrategy::Deterministic;
                }
                Some(other) => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("fallback_strategy"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "unrecognized fallback_strategy `{other}`; only `deterministic` is \
                         supported in v1; field ignored"
                    ),
                )),
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("fallback_strategy"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "fallback_strategy must be a string, got {}; field ignored",
                        v.type_str()
                    ),
                )),
            },
            "max_candidates" => match as_u32(v) {
                Some(n) if (1..=MAX_RESULT_LIMIT).contains(&n) => cfg.max_candidates = n,
                Some(n) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("max_candidates"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_candidates {n} out of bounds (1..={MAX_RESULT_LIMIT}); using default"
                        ),
                    ));
                    cfg.max_candidates = DEFAULT_MAX_CANDIDATES;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("max_candidates"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_candidates must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_results" => match as_u32(v) {
                Some(n) if (1..=MAX_RESULT_LIMIT).contains(&n) => cfg.max_results = n,
                Some(n) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("max_results"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_results {n} out of bounds (1..={MAX_RESULT_LIMIT}); using default"
                        ),
                    ));
                    cfg.max_results = DEFAULT_MAX_RESULTS;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("max_results"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_results must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "min_score" => match as_f32(v) {
                Some(s) => cfg.min_score = clamp_unit_score(s),
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("min_score"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "min_score must be a number, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "deadline_ms" => match as_u64(v) {
                Some(n) if (1..=MAX_DEADLINE_MS).contains(&n) => cfg.deadline_ms = n,
                Some(n) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("deadline_ms"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "deadline_ms {n} out of bounds (1..={MAX_DEADLINE_MS}); using default"
                        ),
                    ));
                    cfg.deadline_ms = DEFAULT_DEADLINE_MS;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("deadline_ms"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "deadline_ms must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_attempts" => match as_u32(v) {
                Some(n) if (1..=MAX_ATTEMPTS).contains(&n) => cfg.max_attempts = n,
                Some(n) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("max_attempts"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_attempts {n} out of bounds (1..={MAX_ATTEMPTS}); using default"
                        ),
                    ));
                    cfg.max_attempts = DEFAULT_MAX_ATTEMPTS;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("max_attempts"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_attempts must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_input_tokens" => match as_u32(v) {
                Some(t) if (1..=MAX_INPUT_TOKENS_BOUND).contains(&t) => cfg.max_input_tokens = t,
                Some(t) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("max_input_tokens"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_input_tokens {t} out of bounds (1..={MAX_INPUT_TOKENS_BOUND}); \
                             using default"
                        ),
                    ));
                    cfg.max_input_tokens = DEFAULT_MAX_INPUT_TOKENS;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("max_input_tokens"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_input_tokens must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            "max_output_tokens" => match as_u32(v) {
                Some(t) if (1..=MAX_INPUT_TOKENS_BOUND).contains(&t) => cfg.max_output_tokens = t,
                Some(t) => {
                    warnings.push(ConfigWarning::retrieval_profile(
                        id,
                        Some("max_output_tokens"),
                        ConfigWarningKind::InvalidValue,
                        format!(
                            "max_output_tokens {t} out of bounds (1..={MAX_INPUT_TOKENS_BOUND}); \
                             using default"
                        ),
                    ));
                    cfg.max_output_tokens = DEFAULT_MAX_OUTPUT_TOKENS;
                }
                None => warnings.push(ConfigWarning::retrieval_profile(
                    id,
                    Some("max_output_tokens"),
                    ConfigWarningKind::InvalidValue,
                    format!(
                        "max_output_tokens must be an integer, got {}; using default",
                        v.type_str()
                    ),
                )),
            },
            _ => {}
        }
    }
    for key in unknown {
        warnings.push(ConfigWarning::retrieval_profile(
            id,
            Some(key.as_str()),
            ConfigWarningKind::UnknownField,
            "unrecognized key; field ignored".into(),
        ));
    }
    Some(cfg)
}

fn parse_prime(raw: &toml::Value, warnings: &mut Vec<ConfigWarning>) -> PrimeConfig {
    let mut prime = PrimeConfig::default();
    let Some(section) = raw.get("prime") else {
        return prime;
    };
    let Some(table) = section.as_table() else {
        warnings.push(ConfigWarning::prime_section(
            ConfigWarningKind::NotATable,
            format!(
                "`prime` must be a table with [prime.skills] / [prime.agents], got {}; prime \
                 disabled",
                section.type_str()
            ),
        ));
        return prime;
    };
    if let Some(skills) = table.get("skills") {
        if let Some(t) = skills.as_table() {
            prime.skills = parse_skill_prime(t, warnings);
        } else {
            warnings.push(ConfigWarning::prime(
                "skills",
                None,
                ConfigWarningKind::NotATable,
                format!(
                    "prime.skills must be a table, got {}; skills prime disabled",
                    skills.type_str()
                ),
            ));
        }
    }
    if let Some(agents) = table.get("agents") {
        if let Some(t) = agents.as_table() {
            prime.agents = parse_agent_prime(t, warnings);
        } else {
            warnings.push(ConfigWarning::prime(
                "agents",
                None,
                ConfigWarningKind::NotATable,
                format!(
                    "prime.agents must be a table, got {}; agents prime disabled",
                    agents.type_str()
                ),
            ));
        }
    }
    for (k, _) in table {
        if k != "skills" && k != "agents" {
            warnings.push(ConfigWarning::prime(
                k,
                None,
                ConfigWarningKind::UnknownField,
                "unrecognized prime key; field ignored".into(),
            ));
        }
    }
    prime
}

fn parse_skill_prime(
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
) -> SkillPrimeConfig {
    let mut cfg = SkillPrimeConfig::default();
    fill_prime_common("skills", table, warnings, |f, v| match f {
        "enabled" => {
            if let Some(b) = v.as_bool() {
                cfg.enabled = b;
            }
        }
        "retrieval_profile" => {
            if let Some(s) = v.as_str() {
                let t = s.trim();
                cfg.retrieval_profile = if t.is_empty() {
                    None
                } else {
                    Some(t.to_owned())
                };
            }
        }
        "max_results" => {
            if let Some(n) = as_u32(v) {
                cfg.max_results = n.clamp(1, MAX_RESULT_LIMIT);
            }
        }
        "max_body_chars" => {
            if let Some(n) = as_u32(v) {
                cfg.max_body_chars = n.max(1);
            }
        }
        "max_total_chars" => {
            if let Some(n) = as_u32(v) {
                cfg.max_total_chars = n.max(1);
            }
        }
        "max_tokens" => {
            if let Some(n) = as_u32(v) {
                cfg.max_tokens = n.clamp(1, MAX_INPUT_TOKENS_BOUND);
            }
        }
        "max_context_fraction" => {
            if let Some(n) = as_f32(v) {
                cfg.max_context_fraction = clamp_context_fraction(n);
            }
        }
        "deadline_ms" => {
            if let Some(n) = as_u64(v) {
                cfg.deadline_ms = n.clamp(1, MAX_DEADLINE_MS);
            }
        }
        "degrade_on_error" => {
            if let Some(b) = v.as_bool() {
                cfg.degrade_on_error = b;
            }
        }
        "min_score" => {
            if let Some(n) = as_f32(v) {
                cfg.min_score = n.clamp(0.0, 1.0);
            }
        }
        _ => {}
    });
    cfg
}

fn parse_agent_prime(
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
) -> AgentPrimeConfig {
    let mut cfg = AgentPrimeConfig::default();
    fill_prime_common("agents", table, warnings, |f, v| match f {
        "enabled" => {
            if let Some(b) = v.as_bool() {
                cfg.enabled = b;
            }
        }
        "retrieval_profile" => {
            if let Some(s) = v.as_str() {
                let t = s.trim();
                cfg.retrieval_profile = if t.is_empty() {
                    None
                } else {
                    Some(t.to_owned())
                };
            }
        }
        "max_results" => {
            if let Some(n) = as_u32(v) {
                cfg.max_results = n.clamp(1, MAX_RESULT_LIMIT);
            }
        }
        "max_body_chars" => {
            if let Some(n) = as_u32(v) {
                cfg.max_body_chars = n.max(1);
            }
        }
        "max_total_chars" => {
            if let Some(n) = as_u32(v) {
                cfg.max_total_chars = n.max(1);
            }
        }
        "max_tokens" => {
            if let Some(n) = as_u32(v) {
                cfg.max_tokens = n.clamp(1, MAX_INPUT_TOKENS_BOUND);
            }
        }
        "max_context_fraction" => {
            if let Some(n) = as_f32(v) {
                cfg.max_context_fraction = clamp_context_fraction(n);
            }
        }
        "deadline_ms" => {
            if let Some(n) = as_u64(v) {
                cfg.deadline_ms = n.clamp(1, MAX_DEADLINE_MS);
            }
        }
        "degrade_on_error" => {
            if let Some(b) = v.as_bool() {
                cfg.degrade_on_error = b;
            }
        }
        _ => {}
    });
    cfg
}

fn fill_prime_common(
    consumer: &str,
    table: &toml::map::Map<String, toml::Value>,
    warnings: &mut Vec<ConfigWarning>,
    mut apply: impl FnMut(&str, &toml::Value),
) {
    let known = [
        "enabled",
        "retrieval_profile",
        "max_results",
        "max_body_chars",
        "max_total_chars",
        "max_tokens",
        "max_context_fraction",
        "deadline_ms",
        "degrade_on_error",
    ];
    for (k, v) in table {
        if known.contains(&k.as_str()) {
            apply(k, v);
        } else {
            warnings.push(ConfigWarning::prime(
                consumer,
                Some(k.as_str()),
                ConfigWarningKind::UnknownField,
                "unrecognized key; field ignored".into(),
            ));
        }
    }
}

fn parse_memory_retrieval_profile(
    raw: &toml::Value,
    warnings: &mut Vec<ConfigWarning>,
) -> Option<String> {
    let mem = raw.get("memory")?;
    let Some(table) = mem.as_table() else {
        return None;
    };
    let Some(v) = table.get("retrieval_profile") else {
        return None;
    };
    match v.as_str() {
        Some(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_owned())
            }
        }
        None => {
            warnings.push(ConfigWarning::memory_retrieval(
                Some("retrieval_profile"),
                ConfigWarningKind::InvalidValue,
                format!(
                    "memory.retrieval_profile must be a string, got {}; ignored",
                    v.type_str()
                ),
            ));
            None
        }
    }
}

fn parse_memory_mode(raw: &toml::Value) -> Option<xai_grok_config_types::MemoryMode> {
    let mem = raw.get("memory")?;
    let table = mem.as_table()?;
    let v = table.get("mode")?;
    v.as_str().and_then(|s| match s.trim() {
        "milvus" => Some(xai_grok_config_types::MemoryMode::Milvus),
        "local" => Some(xai_grok_config_types::MemoryMode::Local),
        _ => None,
    })
}

fn parse_memory_vector_store(raw: &toml::Value) -> Option<String> {
    let mem = raw.get("memory")?;
    let table = mem.as_table()?;
    let v = table.get("vector_store")?;
    v.as_str().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn parse_embedding_protocol(s: &str) -> Option<EmbeddingProtocol> {
    match s.trim().to_ascii_lowercase().as_str() {
        "openai_compatible" | "openai-compatible" => Some(EmbeddingProtocol::OpenaiCompatible),
        _ => None,
    }
}

fn parse_embedding_encoding(s: &str) -> Option<EmbeddingEncoding> {
    match s.trim().to_ascii_lowercase().as_str() {
        "float" => Some(EmbeddingEncoding::Float),
        "base64" => Some(EmbeddingEncoding::Base64),
        _ => None,
    }
}

fn parse_reranker_protocol(s: &str) -> Option<RerankerProtocol> {
    match s.trim().to_ascii_lowercase().as_str() {
        "openai_compatible" | "openai-compatible" => Some(RerankerProtocol::OpenaiCompatible),
        "cohere_compatible" | "cohere-compatible" => Some(RerankerProtocol::CohereCompatible),
        _ => None,
    }
}

fn as_u32(v: &toml::Value) -> Option<u32> {
    v.as_integer()
        .and_then(|i| u32::try_from(i).ok())
        .or_else(|| {
            v.as_float().and_then(|f| {
                if f.fract() == 0.0 && f >= 0.0 {
                    Some(f as u32)
                } else {
                    None
                }
            })
        })
}

fn as_u64(v: &toml::Value) -> Option<u64> {
    v.as_integer()
        .and_then(|i| u64::try_from(i).ok())
        .or_else(|| {
            v.as_float().and_then(|f| {
                if f.fract() == 0.0 && f >= 0.0 {
                    Some(f as u64)
                } else {
                    None
                }
            })
        })
}

fn as_f32(v: &toml::Value) -> Option<f32> {
    v.as_float()
        .map(|f| f as f32)
        .or_else(|| v.as_integer().map(|i| i as f32))
}

fn as_string_array(v: &toml::Value) -> Option<Vec<String>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(item.as_str()?.trim().to_owned());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config_model_override_parse::{ConfigWarningKind, WarningTarget};

    fn parse(toml: &str) -> ParsedRetrievalGraph {
        let v: toml::Value = toml::from_str(toml).unwrap();
        parse_retrieval_graph(&v)
    }

    #[test]
    fn happy_path_parses_all_sections() {
        let p = parse(
            r#"
[embedding_models.e1]
provider = "openai"
model = "text-embedding-3-small"
dimensions = 1536
encoding = "float"
batch_size = 16

[reranker_models.r1]
provider = "openai"
model = "rerank-1"
endpoint = "v1/rerank"

[retrieval_profiles.default]
embedding_models = ["e1"]
reranker_models = ["r1"]
max_candidates = 40
max_results = 8
min_score = 0.2

[prime.skills]
enabled = true
retrieval_profile = "default"
max_results = 2

[memory]
retrieval_profile = "default"
"#,
        );
        assert_eq!(p.graph.embedding_models.len(), 1);
        assert_eq!(p.graph.reranker_models.len(), 1);
        assert_eq!(p.graph.retrieval_profiles.len(), 1);
        assert!(p.graph.prime.skills.enabled);
        assert_eq!(p.graph.memory_retrieval_profile.as_deref(), Some("default"));
        assert!(p.warnings.is_empty());
    }

    /// Both config readers parse the same `[memory] retrieval_profile` key.
    #[test]
    fn memory_retrieval_profile_key_parity_with_memory_config() {
        let toml_src = r#"
[memory]
retrieval_profile = "team-default"
"#;

        // Retrieval-graph parser used by validation and management.
        let p = parse(toml_src);
        assert_eq!(
            p.graph.memory_retrieval_profile.as_deref(),
            Some("team-default")
        );

        // Runtime `MemoryConfig` used by session spawn.
        let v: toml::Value = toml::from_str(toml_src).unwrap();
        let mem_value = v.get("memory").cloned().unwrap();
        let memory_config: crate::config::MemoryConfig = toml::Value::try_into(mem_value).unwrap();

        assert_eq!(
            memory_config.retrieval_profile.as_deref(),
            p.graph.memory_retrieval_profile.as_deref(),
            "both readers must parse the same [memory] retrieval_profile key \
             to the same value; a management write only ever touches this \
             single key"
        );
    }

    #[test]
    fn non_table_section_disables_only_that_section() {
        let p = parse(
            r#"
embedding_models = "oops"
[reranker_models.r1]
provider = "openai"
model = "r"
"#,
        );
        assert!(p.graph.embedding_models.is_empty());
        assert_eq!(p.graph.reranker_models.len(), 1);
        assert!(p.warnings.iter().any(|w| {
            matches!(w.target, WarningTarget::EmbeddingModelsSection)
                && w.kind == ConfigWarningKind::NotATable
        }));
    }

    #[test]
    fn malformed_entry_drops_only_that_entry() {
        let p = parse(
            r#"
[embedding_models.good]
provider = "openai"
model = "m"

[embedding_models.bad]
provider = "openai"
"#,
        );
        assert!(p.graph.embedding_models.contains_key("good"));
        assert!(!p.graph.embedding_models.contains_key("bad"));
        assert!(p.warnings.iter().any(|w| {
            matches!(
                &w.target,
                WarningTarget::EmbeddingModel { id, .. } if id == "bad"
            )
        }));
    }

    #[test]
    fn normalized_id_collision_keeps_first_drops_later() {
        let p = parse(
            r#"
[embedding_models.E1]
provider = "openai"
model = "first"

[embedding_models.e1]
provider = "openai"
model = "second"
"#,
        );
        assert_eq!(p.graph.embedding_models.len(), 1);
        assert_eq!(p.graph.embedding_models["e1"].model, "first");
        assert!(p.warnings.iter().any(|w| {
            w.kind == ConfigWarningKind::ConflictingFields && w.reason.contains("duplicate")
        }));
    }

    #[test]
    fn unknown_fields_warn_and_are_ignored() {
        let p = parse(
            r#"
[embedding_models.e1]
provider = "openai"
model = "m"
future_knob = 42
"#,
        );
        assert!(p.graph.embedding_models.contains_key("e1"));
        assert!(p.warnings.iter().any(|w| {
            w.kind == ConfigWarningKind::UnknownField && w.field() == Some("future_knob")
        }));
    }

    #[test]
    fn endpoint_path_attacks_rejected() {
        let p = parse(
            r#"
[reranker_models.r1]
provider = "openai"
model = "m"
endpoint = "https://evil.example/x"
"#,
        );
        assert!(p.graph.reranker_models.contains_key("r1"));
        assert!(p.graph.reranker_models["r1"].endpoint.is_none());
        assert!(p.warnings.iter().any(|w| w.field() == Some("endpoint")));
    }

    #[test]
    fn legacy_memory_embedding_unaffected() {
        let p = parse(
            r#"
[memory.embedding]
provider = "api"
model = "legacy"
dimensions = 1024
"#,
        );
        assert!(p.graph.memory_retrieval_profile.is_none());
        assert!(p.warnings.is_empty());
    }
}
