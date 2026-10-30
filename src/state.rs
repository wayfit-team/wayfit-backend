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

    /// 카카오 앱 REST API 키 (인가 요청의 client_id, 공개값)
    pub fn kakao_rest_api_key(&self) -> Result<String, AppError> {
        self.non_empty_var("KAKAO_REST_API_KEY")
            .ok_or_else(|| AppError::internal("KAKAO_REST_API_KEY가 설정되지 않았습니다"))
    }

    /// 카카오 앱 Client Secret (`wrangler secret put KAKAO_CLIENT_SECRET`)
    pub fn kakao_client_secret(&self) -> Result<String, AppError> {
        self.env
            .secret("KAKAO_CLIENT_SECRET")
            .map(|v| v.to_string())
            .ok()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| AppError::internal("KAKAO_CLIENT_SECRET이 설정되지 않았습니다"))
    }

    /// 허용된 카카오 Redirect URI 목록 (쉼표 구분, 카카오 콘솔에 등록한 값과 같아야 한다)
    pub fn kakao_redirect_uris(&self) -> Vec<String> {
        self.non_empty_var("KAKAO_REDIRECT_URIS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn non_empty_var(&self, name: &str) -> Option<String> {
        self.env
            .var(name)
            .map(|v| v.to_string())
            .ok()
            .filter(|v| !v.trim().is_empty())
    }

    /// 운영 PWA 주소 (차량 QR 의 pairingUrl). 기본값을 두지 않는다 — 남의 도메인으로 QR 이 나가지 않도록
    pub fn pwa_base_url(&self) -> Result<String, AppError> {
        self.non_empty_var("PWA_BASE_URL")
            .ok_or_else(|| AppError::internal("PWA_BASE_URL이 설정되지 않았습니다"))
    }
}
