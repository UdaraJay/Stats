use dotenv::dotenv;
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct Config {
    pub app_url: String,
    pub service_port: String,
    pub database_url: String,
    pub cors_domains: Vec<String>,
    pub processing_batch_size: usize,
    pub is_development: bool,
}

// TODO: potentially replace this with arctix settings later
impl Config {
    pub fn new() -> Self {
        dotenv().ok();

        Config {
            app_url: Self::get_env("APP_URL", "127.0.0.1:8080"),
            service_port: Self::get_env("SERVICE_PORT", "5775"),
            database_url: Self::get_env("DATABASE_URL", "/data/stats.sqlite"),
            cors_domains: Self::get_env_list("CORS_DOMAINS", ""),
            processing_batch_size: Self::get_env_usize("PROCESSING_BATCH_SIZE", 4),
            is_development: Self::get_env_bool("IS_DEVELOPMENT", false),
        }
    }

    fn get_env(key: &str, default: &str) -> String {
        env::var(key).unwrap_or_else(|_| default.to_string())
    }

    fn get_env_list(key: &str, default: &str) -> Vec<String> {
        env::var(key)
            .unwrap_or_else(|_| default.to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn get_env_usize(key: &str, default: usize) -> usize {
        env::var(key)
            .unwrap_or_else(|_| default.to_string())
            .parse()
            .expect(&format!("Failed to parse {}", key))
    }

    fn get_env_bool(key: &str, default: bool) -> bool {
        env::var(key)
            .unwrap_or_else(|_| default.to_string())
            .parse()
            .expect(&format!("Failed to parse {}", key))
    }
}
