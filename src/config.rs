use dotenv::dotenv;
use serde::Deserialize;
use std::env;

#[derive(Deserialize)]
pub struct Config {
    pub app_url: String,
    pub service_port: String,
    pub database_url: String,
    pub cors_domains: Vec<String>,
    pub PROCESSING_BATCH_SIZE: usize,
}

impl Config {
    pub fn new() -> Self {
        dotenv().ok();

        let PROCESSING_BATCH_SIZE = env::var("PROCESSING_BATCH_SIZE")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .expect("Failed to set memory limit");

        let app_url = env::var("APP_URL").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

        let service_port = env::var("SERVICE_PORT").unwrap_or_else(|_| "5775".to_string());

        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "/data/stats.sqlite".to_string());

        let cors_domains = env::var("CORS_DOMAINS")
            .unwrap_or_else(|_| "".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Config {
            app_url,
            service_port,
            database_url,
            cors_domains,
            PROCESSING_BATCH_SIZE,
        }
    }
}
