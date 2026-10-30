use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Duration;
use serde::Deserialize;
use serde_json::json;
use worker::{send::SendFuture, wasm_bindgen::JsValue, Fetch, Headers, Method, Request, RequestInit};

use crate::error::{AppError, ErrorCode};
use crate::extractors::{bearer_token, AppJson};
use crate::ids::{new_id, new_token, token_hash, DEVICE_TOKEN_PREFIX, SESSION_TOKEN_PREFIX};
use crate::schemas::auth::{KakaoLoginRequest, RegisterDeviceResponse, SessionResponse};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc};

const KAKAO_TOKEN_URL: &str = "https://kauth.kakao.com/oauth/token";
const KAKAO_USER_ME_URL: &str = "https://kapi.kakao.com/v2/user/me";
const SESSION_TTL_DAYS: i64 = 30;

/// 카카오 인가 코드로 로그인 → 자체 세션 토큰 발급
#[utoipa::path(
    post,
    path = "/v1/auth/kakao",
    tag = "Auth",
    request_body = KakaoLoginRequest,
    responses(
        (status = 200, description = "로그인 성공, 30일 유효 세션 토큰", body = SessionResponse),
        (status = 400, description = "허용되지 않은 redirectUri 또는 형식 오류 (VALIDATION_FAILED)", body = crate::error::ErrorResponse),
        (status = 401, description = "카카오가 인가 코드를 거절함 — 만료·재사용 등 (KAKAO_AUTH_FAILED, details.kakaoErrorCode)", body = crate::error::ErrorResponse)
    )
)]
#[worker::send]
pub async fn kakao_login(
    State(state): State<AppState>,
    AppJson(req): AppJson<KakaoLoginRequest>,
) -> Result<Json<SessionResponse>, AppError> {
    let code = req.code.trim();
    if code.is_empty() {
        return Err(AppError::validation("인가 코드(code)가 필요합니다"));
    }
    if !is_allowed_redirect_uri(&req.redirect_uri, &state.kakao_redirect_uris()) {
        return Err(AppError::validation("허용되지 않은 redirectUri입니다"));
    }

    let kakao_user_id = fetch_kakao_user_id(&state, code, &req.redirect_uri).await?;

    let db = state.db()?;
    let now = now_utc();
    let now_str = format_rfc3339(now);
    let expires_at = format_rfc3339(now + Duration::days(SESSION_TTL_DAYS));
    let session_token = new_token(SESSION_TOKEN_PREFIX);

    // 만료된 세션 정리
    db.prepare("DELETE FROM sessions WHERE kakao_user_id = ?1 AND expires_at <= ?2")
        .bind(&[JsValue::from_str(&kakao_user_id), JsValue::from_str(&now_str)])?
        .run()
        .await?;

    db.prepare(
        "INSERT INTO sessions (token_hash, kakao_user_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        JsValue::from_str(&token_hash(&session_token)),
        JsValue::from_str(&kakao_user_id),
        JsValue::from_str(&now_str),
        JsValue::from_str(&expires_at),
    ])?
    .run()
    .await?;

    Ok(Json(SessionResponse {
        session_token,
        expires_at,
    }))
}

/// 로그아웃 — 현재 세션 토큰 폐기 (멱등)
#[utoipa::path(
    post,
    path = "/v1/auth/logout",
    tag = "Auth",
    security(("session_token" = [])),
    responses((status = 204, description = "세션 폐기 완료 (토큰이 없거나 이미 폐기돼도 204)"))
)]
#[worker::send]
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, AppError> {
    if let Some(token) = bearer_token(&headers, SESSION_TOKEN_PREFIX) {
        state
            .db()?
            .prepare("DELETE FROM sessions WHERE token_hash = ?1")
            .bind(&[JsValue::from_str(&token_hash(token))])?
            .run()
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// 차량 기기 등록 — Pleos 앱 최초 실행 시 1회 호출해 기기 ID·토큰 발급
#[utoipa::path(
    post,
    path = "/v1/devices",
    tag = "Vehicle (Pleos App)",
    responses(
        (status = 201, description = "발급된 기기 ID와 기기 토큰 (토큰은 이 응답에서만 확인 가능)", body = RegisterDeviceResponse)
    )
)]
#[worker::send]
pub async fn register_device(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<RegisterDeviceResponse>), AppError> {
    let device_id = new_id("dev");
    let device_token = new_token(DEVICE_TOKEN_PREFIX);

    state
        .db()?
        .prepare("INSERT INTO devices (id, token_hash, created_at) VALUES (?1, ?2, ?3)")
        .bind(&[
            JsValue::from_str(&device_id),
            JsValue::from_str(&token_hash(&device_token)),
            JsValue::from_str(&format_rfc3339(now_utc())),
        ])?
        .run()
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterDeviceResponse {
            device_id,
            device_token,
        }),
    ))
}

fn is_allowed_redirect_uri(uri: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|a| a == uri)
}

#[derive(Deserialize)]
struct KakaoTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct KakaoUserResponse {
    id: i64,
}

#[derive(Deserialize, Default)]
struct KakaoErrorResponse {
    #[serde(default)]
    error_code: Option<String>,
}

/// 인가 코드 → 카카오 액세스 토큰 → 회원번호. 카카오 토큰은 저장하지 않고 버린다
async fn fetch_kakao_user_id(state: &AppState, code: &str, redirect_uri: &str) -> Result<String, AppError> {
    let client_id = state.kakao_rest_api_key()?;
    let client_secret = state.kakao_client_secret()?;
    let form = serde_urlencoded::to_string([
        ("grant_type", "authorization_code"),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("code", code),
        ("client_secret", client_secret.as_str()),
    ])
    .map_err(|e| AppError::internal(format!("카카오 토큰 요청 생성 실패: {}", e)))?;

    let headers = Headers::new();
    headers.set("Content-Type", "application/x-www-form-urlencoded;charset=utf-8")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&form)));
    let mut res = SendFuture::new(Fetch::Request(Request::new_with_init(KAKAO_TOKEN_URL, &init)?).send()).await?;

    if res.status_code() != 200 {
        let status = res.status_code();
        let body: KakaoErrorResponse = SendFuture::new(res.json()).await.unwrap_or_default();
        if status >= 500 {
            return Err(AppError::internal(format!(
                "카카오 토큰 API 오류 {} ({:?})",
                status, body.error_code
            )));
        }
        return Err(AppError::new(
            ErrorCode::KakaoAuthFailed,
            "카카오 로그인에 실패했습니다. 다시 시도해 주세요",
        )
        .with_details(json!({ "kakaoErrorCode": body.error_code })));
    }
    let token: KakaoTokenResponse = SendFuture::new(res.json()).await?;

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", token.access_token))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let mut res = SendFuture::new(Fetch::Request(Request::new_with_init(KAKAO_USER_ME_URL, &init)?).send()).await?;
    if res.status_code() != 200 {
        return Err(AppError::internal(format!(
            "카카오 사용자 정보 API 오류 {}",
            res.status_code()
        )));
    }
    let user: KakaoUserResponse = SendFuture::new(res.json()).await?;
    Ok(user.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redirect_uri_must_match_exactly() {
        let allowed = vec![
            "https://wayfit.pages.dev/auth/kakao/callback".to_string(),
            "http://localhost:5173/auth/kakao/callback".to_string(),
        ];
        assert!(is_allowed_redirect_uri("https://wayfit.pages.dev/auth/kakao/callback", &allowed));
        assert!(!is_allowed_redirect_uri("https://wayfit.pages.dev/auth/kakao/callback/", &allowed));
        assert!(!is_allowed_redirect_uri("https://evil.example/auth/kakao/callback", &allowed));
        assert!(!is_allowed_redirect_uri(
            "https://wayfit.pages.dev.evil.example/auth/kakao/callback",
            &allowed
        ));
        assert!(!is_allowed_redirect_uri("https://wayfit.pages.dev/auth/kakao/callback", &[]));
    }
}
