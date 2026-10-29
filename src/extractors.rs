use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use crate::error::AppError;

/// Extracts vehicle UUID from `X-Device-Id` header
#[derive(Debug, Clone)]
pub struct DeviceId(pub String);

impl<S> FromRequestParts<S> for DeviceId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let val = parts
            .headers
            .get("x-device-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unauthorized("X-Device-Id 헤더가 필요합니다 (Pleos 차량 앱 기기 UUID)")
            })?;

        Ok(DeviceId(val.to_string()))
    }
}

/// Extracts Kakao OAuth user ID from `X-Kakao-User-Id` header
#[derive(Debug, Clone)]
pub struct KakaoUserId(pub String);

impl<S> FromRequestParts<S> for KakaoUserId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let val = parts
            .headers
            .get("x-kakao-user-id")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unauthorized(
                    "X-Kakao-User-Id 헤더가 필요합니다 (카카오 OAuth 회원 식별자)",
                )
            })?;

        Ok(KakaoUserId(val.to_string()))
    }
}
