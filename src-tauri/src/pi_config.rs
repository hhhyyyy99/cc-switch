//! Pi Agent configuration read/write helpers.
//!
//! Pi stores user agent configuration under `~/.pi/agent/` by default:
//! - `models.json` contains custom/override providers under `providers`
//! - `settings.json` contains defaults such as `defaultProvider` / `defaultModel`

use crate::config::write_json_file;
use crate::error::AppError;
use crate::settings::get_pi_override_dir;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn pi_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn get_pi_dir() -> PathBuf {
    if let Some(override_dir) = get_pi_override_dir() {
        return override_dir;
    }

    crate::config::get_home_dir().join(".pi").join("agent")
}

pub fn get_pi_models_path() -> PathBuf {
    get_pi_dir().join("models.json")
}

pub fn get_pi_settings_path() -> PathBuf {
    get_pi_dir().join("settings.json")
}

pub fn get_pi_auth_path() -> PathBuf {
    get_pi_dir().join("auth.json")
}

pub fn get_pi_mcp_path() -> PathBuf {
    get_pi_dir().join("mcp.json")
}

pub fn get_pi_sessions_dir() -> PathBuf {
    get_pi_dir().join("sessions")
}

fn default_models_config() -> Value {
    json!({ "providers": {} })
}

pub fn read_pi_models_config() -> Result<Value, AppError> {
    let path = get_pi_models_path();
    if !path.exists() {
        return Ok(default_models_config());
    }

    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if content.trim().is_empty() {
        return Ok(default_models_config());
    }

    json5::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse Pi Agent models.json: {}: {e}",
            path.display()
        ))
    })
}

pub fn write_pi_models_config(config: &Value) -> Result<(), AppError> {
    write_json_file(&get_pi_models_path(), config)
}

pub fn read_pi_settings_config() -> Result<Value, AppError> {
    let path = get_pi_settings_path();
    if !path.exists() {
        return Ok(json!({}));
    }

    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if content.trim().is_empty() {
        return Ok(json!({}));
    }

    json5::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse Pi Agent settings.json: {}: {e}",
            path.display()
        ))
    })
}

pub fn write_pi_settings_config(config: &Value) -> Result<(), AppError> {
    write_json_file(&get_pi_settings_path(), config)
}

pub fn read_pi_auth_config() -> Result<Value, AppError> {
    let path = get_pi_auth_path();
    if !path.exists() {
        return Ok(json!({}));
    }

    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if content.trim().is_empty() {
        return Ok(json!({}));
    }

    json5::from_str(&content).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse Pi Agent auth.json: {}: {e}",
            path.display()
        ))
    })
}

pub fn write_pi_auth_config(config: &Value) -> Result<(), AppError> {
    write_json_file(&get_pi_auth_path(), config)
}

pub fn get_auth_provider_ids() -> Result<Vec<String>, AppError> {
    let auth = read_pi_auth_config()?;
    Ok(auth
        .as_object()
        .map(|obj| {
            obj.keys()
                .filter(|key| !key.trim().is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default())
}

pub fn get_providers() -> Result<Map<String, Value>, AppError> {
    let config = read_pi_models_config()?;
    Ok(config
        .get("providers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default())
}

pub fn get_provider(id: &str) -> Result<Option<Value>, AppError> {
    Ok(get_providers()?.get(id).cloned())
}

pub fn get_default_provider_and_model() -> Result<(Option<String>, Option<String>), AppError> {
    let settings = read_pi_settings_config()?;
    let default_provider = settings
        .get("defaultProvider")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let default_model = settings
        .get("defaultModel")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok((default_provider, default_model))
}

pub fn get_default_provider() -> Result<Option<String>, AppError> {
    get_default_provider_and_model().map(|(provider, _)| provider)
}

pub fn get_live_provider_ids() -> Result<Vec<String>, AppError> {
    let mut ids = BTreeSet::new();

    for id in get_providers()?.keys() {
        if !id.trim().is_empty() {
            ids.insert(id.clone());
        }
    }

    for id in get_auth_provider_ids()? {
        if !id.trim().is_empty() {
            ids.insert(id);
        }
    }

    if let (Some(default_provider), _) = get_default_provider_and_model()? {
        ids.insert(default_provider);
    }

    Ok(ids.into_iter().collect())
}

pub fn set_provider(id: &str, config: Value) -> Result<(), AppError> {
    let _guard = pi_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Pi Agent config lock poisoned".to_string()))?;
    let mut full_config = read_pi_models_config()?;

    if full_config.get("providers").is_none() {
        full_config["providers"] = json!({});
    }

    if let Some(providers) = full_config
        .get_mut("providers")
        .and_then(Value::as_object_mut)
    {
        providers.insert(id.to_string(), config);
    }

    write_pi_models_config(&full_config)
}

pub fn remove_provider(id: &str) -> Result<(), AppError> {
    let _guard = pi_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Pi Agent config lock poisoned".to_string()))?;
    let mut config = read_pi_models_config()?;

    if let Some(providers) = config.get_mut("providers").and_then(Value::as_object_mut) {
        providers.remove(id);
    }

    write_pi_models_config(&config)?;
    remove_auth_provider(id)?;
    clear_default_provider_if_matches(id)
}

fn remove_auth_provider(id: &str) -> Result<(), AppError> {
    let path = get_pi_auth_path();
    if !path.exists() {
        return Ok(());
    }

    let mut auth = read_pi_auth_config()?;
    let removed = auth
        .as_object_mut()
        .map(|obj| obj.remove(id).is_some())
        .unwrap_or(false);

    if removed {
        write_pi_auth_config(&auth)?;
    }

    Ok(())
}

fn clear_default_provider_if_matches(id: &str) -> Result<(), AppError> {
    let path = get_pi_settings_path();
    if !path.exists() {
        return Ok(());
    }

    let mut settings = read_pi_settings_config()?;
    let should_clear = settings
        .get("defaultProvider")
        .and_then(Value::as_str)
        .map(str::trim)
        == Some(id);

    if should_clear {
        if let Some(obj) = settings.as_object_mut() {
            obj.remove("defaultProvider");
            obj.remove("defaultModel");
        }
        write_pi_settings_config(&settings)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<PiModelEntry>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_overrides: HashMap<String, Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiModelEntry {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<PiModelCost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PiModelCost {
    pub input: f64,
    pub output: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct PiBuiltInProvider {
    pub id: &'static str,
    pub name: &'static str,
    pub env_key: Option<&'static str>,
    pub default_model: &'static str,
    pub base_url: &'static str,
    pub api: &'static str,
}

pub const PI_BUILT_IN_PROVIDERS: &[PiBuiltInProvider] = &[
    PiBuiltInProvider {
        id: "anthropic",
        name: "Anthropic",
        env_key: Some("ANTHROPIC_API_KEY"),
        default_model: "claude-opus-4-8",
        base_url: "https://api.anthropic.com",
        api: "anthropic-messages",
    },
    PiBuiltInProvider {
        id: "openai",
        name: "OpenAI",
        env_key: Some("OPENAI_API_KEY"),
        default_model: "gpt-5.5",
        base_url: "https://api.openai.com/v1",
        api: "openai-responses",
    },
    PiBuiltInProvider {
        id: "deepseek",
        name: "DeepSeek",
        env_key: Some("DEEPSEEK_API_KEY"),
        default_model: "deepseek-v4-pro",
        base_url: "https://api.deepseek.com",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "google",
        name: "Google Gemini",
        env_key: Some("GEMINI_API_KEY"),
        default_model: "gemini-3.1-pro-preview",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        api: "google-generative-ai",
    },
    PiBuiltInProvider {
        id: "openrouter",
        name: "OpenRouter",
        env_key: Some("OPENROUTER_API_KEY"),
        default_model: "moonshotai/kimi-k2.6",
        base_url: "https://openrouter.ai/api/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "vercel-ai-gateway",
        name: "Vercel AI Gateway",
        env_key: Some("AI_GATEWAY_API_KEY"),
        default_model: "zai/glm-5.1",
        base_url: "https://ai-gateway.vercel.sh",
        api: "anthropic-messages",
    },
    PiBuiltInProvider {
        id: "zai",
        name: "ZAI Coding Plan (Global)",
        env_key: Some("ZAI_API_KEY"),
        default_model: "glm-5.1",
        base_url: "https://api.z.ai/api/coding/paas/v4",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "zai-coding-cn",
        name: "ZAI Coding Plan (China)",
        env_key: Some("ZAI_CODING_CN_API_KEY"),
        default_model: "glm-5.1",
        base_url: "https://open.bigmodel.cn/api/coding/paas/v4",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "opencode",
        name: "OpenCode Zen",
        env_key: Some("OPENCODE_API_KEY"),
        default_model: "kimi-k2.6",
        base_url: "https://opencode.ai/zen/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "opencode-go",
        name: "OpenCode Go",
        env_key: Some("OPENCODE_API_KEY"),
        default_model: "kimi-k2.6",
        base_url: "https://opencode.ai/zen/go/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "kimi-coding",
        name: "Kimi For Coding",
        env_key: Some("KIMI_API_KEY"),
        default_model: "kimi-for-coding",
        base_url: "https://api.kimi.com/coding",
        api: "anthropic-messages",
    },
    PiBuiltInProvider {
        id: "minimax",
        name: "MiniMax",
        env_key: Some("MINIMAX_API_KEY"),
        default_model: "MiniMax-M2.7",
        base_url: "https://api.minimax.io/anthropic",
        api: "anthropic-messages",
    },
    PiBuiltInProvider {
        id: "minimax-cn",
        name: "MiniMax (China)",
        env_key: Some("MINIMAX_CN_API_KEY"),
        default_model: "MiniMax-M2.7",
        base_url: "https://api.minimaxi.com/anthropic",
        api: "anthropic-messages",
    },
    PiBuiltInProvider {
        id: "moonshotai",
        name: "Moonshot AI",
        env_key: Some("MOONSHOT_API_KEY"),
        default_model: "kimi-k2.6",
        base_url: "https://api.moonshot.ai/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "moonshotai-cn",
        name: "Moonshot AI (China)",
        env_key: Some("MOONSHOT_API_KEY"),
        default_model: "kimi-k2.6",
        base_url: "https://api.moonshot.cn/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "xiaomi",
        name: "Xiaomi MiMo",
        env_key: Some("XIAOMI_API_KEY"),
        default_model: "mimo-v2.5-pro",
        base_url: "https://api.xiaomimimo.com/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "xiaomi-token-plan-cn",
        name: "Xiaomi MiMo Token Plan (China)",
        env_key: Some("XIAOMI_TOKEN_PLAN_CN_API_KEY"),
        default_model: "mimo-v2.5-pro",
        base_url: "https://token-plan-cn.xiaomimimo.com/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "xiaomi-token-plan-ams",
        name: "Xiaomi MiMo Token Plan (Amsterdam)",
        env_key: Some("XIAOMI_TOKEN_PLAN_AMS_API_KEY"),
        default_model: "mimo-v2.5-pro",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        api: "openai-completions",
    },
    PiBuiltInProvider {
        id: "xiaomi-token-plan-sgp",
        name: "Xiaomi MiMo Token Plan (Singapore)",
        env_key: Some("XIAOMI_TOKEN_PLAN_SGP_API_KEY"),
        default_model: "mimo-v2.5-pro",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        api: "openai-completions",
    },
];

pub fn get_builtin_provider(id: &str) -> Option<&'static PiBuiltInProvider> {
    PI_BUILT_IN_PROVIDERS
        .iter()
        .find(|provider| provider.id == id)
}

pub fn build_builtin_provider_config(provider_id: &str, model_id: Option<&str>) -> Option<Value> {
    let provider = get_builtin_provider(provider_id)?;
    let model_id = model_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(provider.default_model);

    let mut config = json!({
        "name": provider.name,
        "baseUrl": provider.base_url,
        "api": provider.api,
        "modelOverrides": {
            model_id: {}
        }
    });

    if let Some(env_key) = provider.env_key {
        config["apiKey"] = Value::String(format!("${env_key}"));
    }

    Some(config)
}

pub fn get_typed_providers() -> Result<IndexMap<String, PiProviderConfig>, AppError> {
    let providers = get_providers()?;
    let mut result = IndexMap::new();

    for (id, value) in providers {
        match serde_json::from_value::<PiProviderConfig>(value.clone()) {
            Ok(config) => {
                result.insert(id, config);
            }
            Err(e) => {
                log::warn!("Failed to parse Pi Agent provider '{id}': {e}");
            }
        }
    }

    Ok(result)
}

pub fn set_typed_provider(id: &str, config: &PiProviderConfig) -> Result<(), AppError> {
    let value = serde_json::to_value(config).map_err(|e| AppError::JsonSerialize { source: e })?;
    set_provider(id, value)
}

pub fn hydrate_builtin_provider_defaults(id: &str, config: &mut PiProviderConfig) {
    let Some(provider) = get_builtin_provider(id) else {
        return;
    };

    if config
        .name
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.name = Some(provider.name.to_string());
    }
    if config
        .base_url
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.base_url = Some(provider.base_url.to_string());
    }
    if config
        .api
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.api = Some(provider.api.to_string());
    }
}

pub fn apply_switch_defaults(provider_id: &str, settings_config: &Value) -> Result<(), AppError> {
    let _guard = pi_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Pi Agent config lock poisoned".to_string()))?;
    let mut settings = read_pi_settings_config()?;
    if !settings.is_object() {
        settings = json!({});
    }

    let model_id = settings_config
        .get("models")
        .and_then(Value::as_array)
        .and_then(|models| models.first())
        .and_then(|model| model.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            settings_config
                .get("modelOverrides")
                .and_then(Value::as_object)
                .and_then(|models| models.keys().next())
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });

    if let Some(obj) = settings.as_object_mut() {
        obj.insert(
            "defaultProvider".to_string(),
            Value::String(provider_id.to_string()),
        );
        if let Some(model_id) = model_id {
            obj.insert("defaultModel".to_string(), Value::String(model_id));
        } else {
            obj.remove("defaultModel");
        }
    }

    write_pi_settings_config(&settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn builtin_provider_config_includes_endpoint_and_api() {
        let config = build_builtin_provider_config("opencode-go", Some("kimi-k2.6"))
            .expect("opencode-go should be a known Pi built-in provider");

        assert_eq!(config["baseUrl"], "https://opencode.ai/zen/go/v1");
        assert_eq!(config["api"], "openai-completions");
        assert_eq!(config["modelOverrides"]["kimi-k2.6"], json!({}));
    }

    #[test]
    fn hydrate_builtin_defaults_preserves_existing_model_overrides() {
        let mut config = PiProviderConfig {
            name: Some("Xiaomi MiMo Token Plan (China)".to_string()),
            base_url: None,
            api_key: Some("$XIAOMI_TOKEN_PLAN_CN_API_KEY".to_string()),
            api: None,
            headers: HashMap::new(),
            auth_header: None,
            models: Vec::new(),
            model_overrides: HashMap::from([("mimo-v2-pro".to_string(), json!({}))]),
            extra: HashMap::new(),
        };

        hydrate_builtin_provider_defaults("xiaomi-token-plan-cn", &mut config);

        assert_eq!(
            config.base_url.as_deref(),
            Some("https://token-plan-cn.xiaomimimo.com/v1")
        );
        assert_eq!(config.api.as_deref(), Some("openai-completions"));
        assert!(config.model_overrides.contains_key("mimo-v2-pro"));
    }

    #[test]
    #[serial]
    fn remove_provider_clears_models_auth_and_default_settings() {
        let temp_home = tempfile::tempdir().expect("create temp home");
        let old_test_home = std::env::var_os("CC_SWITCH_TEST_HOME");
        std::env::set_var("CC_SWITCH_TEST_HOME", temp_home.path());

        let result = (|| -> Result<(), AppError> {
            write_pi_models_config(&json!({
                "providers": {
                    "xiaomi-token-plan-cn": {
                        "apiKey": "$XIAOMI_TOKEN_PLAN_CN_API_KEY",
                        "modelOverrides": {
                            "mimo-v2-pro": {}
                        },
                        "name": "Xiaomi MiMo Token Plan (China)"
                    },
                    "opencode-go": {
                        "apiKey": "$OPENCODE_API_KEY",
                        "modelOverrides": {
                            "kimi-k2.6": {}
                        },
                        "name": "OpenCode Go"
                    }
                }
            }))?;
            write_pi_auth_config(&json!({
                "xiaomi-token-plan-cn": {
                    "type": "api_key",
                    "key": "secret"
                },
                "opencode-go": {
                    "type": "api_key",
                    "key": "secret"
                }
            }))?;
            write_pi_settings_config(&json!({
                "defaultProvider": "xiaomi-token-plan-cn",
                "defaultModel": "mimo-v2-pro",
                "defaultThinkingLevel": "high"
            }))?;

            remove_provider("xiaomi-token-plan-cn")?;

            let providers = get_providers()?;
            assert!(!providers.contains_key("xiaomi-token-plan-cn"));
            assert!(providers.contains_key("opencode-go"));

            let auth = read_pi_auth_config()?;
            assert!(auth.get("xiaomi-token-plan-cn").is_none());
            assert!(auth.get("opencode-go").is_some());

            let settings = read_pi_settings_config()?;
            assert!(settings.get("defaultProvider").is_none());
            assert!(settings.get("defaultModel").is_none());
            assert_eq!(settings["defaultThinkingLevel"], "high");

            let live_ids = get_live_provider_ids()?;
            assert!(!live_ids.iter().any(|id| id == "xiaomi-token-plan-cn"));
            assert!(live_ids.iter().any(|id| id == "opencode-go"));

            Ok(())
        })();

        match old_test_home {
            Some(value) => std::env::set_var("CC_SWITCH_TEST_HOME", value),
            None => std::env::remove_var("CC_SWITCH_TEST_HOME"),
        }

        result.expect("remove provider should clear all Pi live provider sources");
    }

    #[test]
    #[serial]
    fn apply_switch_defaults_clears_stale_model_when_target_has_no_model() {
        let temp_home = tempfile::tempdir().expect("create temp home");
        let old_test_home = std::env::var_os("CC_SWITCH_TEST_HOME");
        std::env::set_var("CC_SWITCH_TEST_HOME", temp_home.path());

        let result = (|| -> Result<(), AppError> {
            write_pi_settings_config(&json!({
                "defaultProvider": "old-provider",
                "defaultModel": "old-model",
                "defaultThinkingLevel": "high"
            }))?;

            apply_switch_defaults(
                "new-provider",
                &json!({
                    "baseUrl": "https://api.example.com/v1",
                    "apiKey": "$NEW_API_KEY",
                    "api": "anthropic-messages"
                }),
            )?;

            let settings = read_pi_settings_config()?;
            assert_eq!(settings["defaultProvider"], "new-provider");
            assert!(settings.get("defaultModel").is_none());
            assert_eq!(settings["defaultThinkingLevel"], "high");

            Ok(())
        })();

        match old_test_home {
            Some(value) => std::env::set_var("CC_SWITCH_TEST_HOME", value),
            None => std::env::remove_var("CC_SWITCH_TEST_HOME"),
        }

        result.expect("switching to provider without models should clear stale default model");
    }
}
