use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KakaoLoginRequest {
    /// 카카오 인가 코드 (Redirect URI로 전달된 `code`)
    #[schema(example = "uOr1cZ8QmF...")]
    pub code: String,
    /// 인가 요청에 사용한 Redirect URI (서버 허용 목록과 정확히 일치해야 함)
    #[schema(example = "https://pwa.example.com/auth/kakao/callback")]
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    /// 이후 요청에 `Authorization: Bearer`로 보내는 세션 토큰 (30일 유효)
    #[schema(example = "wfs_3f9a0c...")]
    pub session_token: String,
    #[schema(example = "2026-12-05T03:00:00Z")]
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDeviceResponse {
    #[schema(example = "dev_8b12f3a0c4d54e0f9a7b6c5d4e3f2a1b")]
    pub device_id: String,
    /// 이후 요청에 `Authorization: Bearer`로 보내는 기기 토큰. 이 응답에서만 한 번 내려주므로 기기에 안전하게 보관
    #[schema(example = "wfd_7c1e4b...")]
    pub device_token: String,
}
