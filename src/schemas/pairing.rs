use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingCodeResponse {
    /// 8자리 숫자 페어링 코드 (10분 유효, 1회용)
    #[schema(example = "48201937")]
    pub code: String,
    #[schema(example = "2026-11-05T03:20:00Z")]
    pub expires_at: String,
    /// QR 코드로 인코딩할 PWA 연결 URL
    #[schema(example = "https://pwa.example.com/pair?code=48201937")]
    pub pairing_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DevicePairingStatusResponse {
    #[schema(example = "demo-car-01")]
    pub device_id: String,
    /// 현재 이 차량 기기에 연결된 카카오 사용자(폰) 수 (REQ-B17)
    #[schema(example = 1)]
    pub linked_user_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimPairingRequest {
    #[schema(example = "48201937")]
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimPairingResponse {
    #[schema(example = "kakao_user_1001")]
    pub kakao_user_id: String,
    #[schema(example = "demo-car-01")]
    pub device_id: String,
    #[schema(example = "2026-11-05T03:11:00Z")]
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkedDeviceItem {
    #[schema(example = "demo-car-01")]
    pub device_id: String,
    #[schema(example = "2026-11-05T03:11:00Z")]
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyDevicesResponse {
    pub devices: Vec<LinkedDeviceItem>,
}

/// 차량에 연결된 폰 1대. 카카오 회원번호는 차에 보여주지 않으므로 연결 단위 ID 만 준다
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkedPhoneItem {
    /// 연결 해제 때 쓰는 ID (`phn_` + 숫자). 다시 연결하면 바뀐다
    #[schema(example = "phn_12")]
    pub phone_id: String,
    #[schema(example = "2026-11-05T03:11:00Z")]
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DevicePhonesResponse {
    /// 연결된 순서대로
    pub phones: Vec<LinkedPhoneItem>,
}
