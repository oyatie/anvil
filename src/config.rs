use std::path::PathBuf;
use crate::ai_driver::{ModelExecutionConfig, ModelProvider};

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub watched_repos: Vec<String>,
    pub repos_dir: PathBuf,
    pub data_dir: PathBuf,
    pub rules_path: Option<PathBuf>,
    pub agy_effort: String,
    pub auto_forward_webhooks: bool,
    pub ai_provider: ModelProvider,
    pub specific_model: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let watched_repos_str = std::env::var("WATCHED_REPOS")
            .unwrap_or_else(|_| "oyatie/oyatie,oyatie/console,oyatie/anvil".to_string());
        let watched_repos = watched_repos_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let repos_dir = std::env::var("REPOS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("repos"));
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| current_dir.join("data"));

        let rules_path = std::env::var("RULES_PATH")
            .map(PathBuf::from)
            .ok()
            .or_else(|| {
                let default_rules = current_dir.join("rules.md");
                if default_rules.exists() {
                    Some(default_rules)
                } else {
                    None
                }
            });

        let agy_effort = std::env::var("AGY_EFFORT").unwrap_or_else(|_| "high".to_string());
        let auto_forward_webhooks = std::env::var("AUTO_FORWARD_WEBHOOKS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(true);

        let ai_provider_str = std::env::var("AI_PROVIDER").unwrap_or_else(|_| "agy".to_string());
        let ai_provider = ModelProvider::from_str_name(&ai_provider_str);
        let specific_model = std::env::var("AI_MODEL").ok();

        Self {
            host,
            port,
            watched_repos,
            repos_dir,
            data_dir,
            rules_path,
            agy_effort,
            auto_forward_webhooks,
            ai_provider,
            specific_model,
        }
    }

    pub fn to_model_config(&self) -> ModelExecutionConfig {
        ModelExecutionConfig {
            provider: self.ai_provider.clone(),
            specific_model: self.specific_model.clone(),
            reasoning_effort: self.agy_effort.clone(),
            print_timeout_secs: 300,
        }
    }
}
