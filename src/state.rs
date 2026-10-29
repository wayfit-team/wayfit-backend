use std::sync::Arc;
use worker::{D1Database, Env};

use crate::error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub env: Arc<Env>,
}

// Cloudflare Workers runs on a single-threaded Wasm runtime.
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

impl AppState {
    pub fn new(env: Env) -> Self {
        Self { env: Arc::new(env) }
    }

    pub fn db(&self) -> Result<D1Database, AppError> {
        self.env
            .d1("wayfit_db")
            .or_else(|_| self.env.d1("DB"))
            .map_err(|e| {
                AppError::internal(format!(
                    "D1 'wayfit_db' (또는 'DB') 바인딩을 찾을 수 없습니다: {}",
                    e
                ))
            })
    }

    pub fn pwa_base_url(&self) -> String {
        self.env
            .var("PWA_BASE_URL")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "https://wayfit.pages.dev".to_string())
    }
}
