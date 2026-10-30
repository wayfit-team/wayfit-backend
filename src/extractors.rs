use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        FromRequest, FromRequestParts,
    },
    http::{header::AUTHORIZATION, request::Parts, HeaderMap},
};
use serde::Deserialize;
use serde_json::json;
use worker::{send::SendFuture, wasm_bindgen::JsValue};

use crate::db::IdRow;
use crate::error::AppError;
use crate::ids::{token_hash, DEVICE_TOKEN_PREFIX, SESSION_TOKEN_PREFIX};
use crate::state::AppState;
use crate::time::{format_rfc3339, now_utc};

/// `axum::Json` 대신 쓰는 요청 본문 추출기.
/// 형식 오류를 axum 기본 텍스트 응답 대신 공통 에러 형식(400 VALIDATION_FAILED)으로 돌려준다 (NFR-B4).
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

/// `axum::extract::Query` 대신 쓰는 쿼리 추출기. 형식 오류를 400 VALIDATION_FAILED로 돌려준다.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct AppQuery<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        let message = match &rejection {
            JsonRejection::MissingJsonContentType(_) => {
                "Content-Type: application/json 헤더가 필요합니다"
            }
            JsonRejection::JsonSyntaxError(_) => "요청 본문이 올바른 JSON이 아닙니다",
            JsonRejection::JsonDataError(_) => "요청 본문의 필드가 누락되었거나 형식이 올바르지 않습니다",
            _ => "요청 본문을 읽을 수 없습니다",
        };
        AppError::validation(message).with_details(json!({ "reason": rejection.body_text() }))
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        AppError::validation("쿼리 파라미터가 누락되었거나 형식이 올바르지 않습니다")
            .with_details(json!({ "reason": rejection.body_text() }))
    }
}

/// `Authorization: Bearer <token>` 값 중 지정한 접두사(wfs_/wfd_)로 시작하는 토큰
pub fn bearer_token<'a>(headers: &'a HeaderMap, prefix: &str) -> Option<&'a str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| t.starts_with(prefix))
}

/// Vehicle device ID resolved from the device token (`Authorization: Bearer wfd_...`)
#[derive(Debug, Clone)]
pub struct DeviceId(pub String);

impl FromRequestParts<AppState> for DeviceId {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let hash = bearer_token(&parts.headers, DEVICE_TOKEN_PREFIX)
            .map(token_hash)
            .ok_or_else(|| {
                AppError::unauthorized(
                    "Authorization: Bearer <기기 토큰> 헤더가 필요합니다 (POST /v1/devices로 발급)",
                )
            })?;
        let state = state.clone();

        SendFuture::new(async move {
            let row: Option<IdRow> = state
                .db()?
                .prepare("SELECT id FROM devices WHERE token_hash = ?1")
                .bind(&[JsValue::from_str(&hash)])?
                .first(None)
                .await?;
            row.map(|r| DeviceId(r.id))
                .ok_or_else(|| AppError::unauthorized("유효하지 않은 기기 토큰입니다"))
        })
        .await
    }
}

#[derive(Deserialize)]
struct SessionUserRow {
    kakao_user_id: String,
}

/// Kakao user ID resolved from a login session (`Authorization: Bearer wfs_...`)
#[derive(Debug, Clone)]
pub struct KakaoUserId(pub String);

impl FromRequestParts<AppState> for KakaoUserId {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let hash = bearer_token(&parts.headers, SESSION_TOKEN_PREFIX)
            .map(token_hash)
            .ok_or_else(|| {
                AppError::unauthorized("로그인이 필요합니다 (Authorization: Bearer <세션 토큰>)")
            })?;
        let state = state.clone();

        SendFuture::new(async move {
            let row: Option<SessionUserRow> = state
                .db()?
                .prepare("SELECT kakao_user_id FROM sessions WHERE token_hash = ?1 AND expires_at > ?2")
                .bind(&[
                    JsValue::from_str(&hash),
                    JsValue::from_str(&format_rfc3339(now_utc())),
                ])?
                .first(None)
                .await?;
            row.map(|r| KakaoUserId(r.kakao_user_id)).ok_or_else(|| {
                AppError::unauthorized("로그인이 만료되었습니다. 다시 로그인해 주세요")
            })
        })
        .await
    }
}
