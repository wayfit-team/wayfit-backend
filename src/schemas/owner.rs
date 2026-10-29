use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::hours::OpenHoursMap;
use crate::schemas::stores::StoreCategory;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerStoreDto {
    #[schema(example = "store_01")]
    pub id: String,
    #[schema(example = "kakao_owner_01")]
    pub owner_kakao_id: String,
    #[schema(example = "호수길 로스터리")]
    pub name: String,
    pub category: StoreCategory,
    #[schema(example = "경기도 수원시 영통구 월드컵로 199")]
    pub address: String,
    #[schema(example = 37.2825)]
    pub lat: f64,
    #[schema(example = 127.0530)]
    pub lng: f64,
    #[schema(example = "아메리카노 1잔 무료 증정")]
    pub benefit_text: String,
    /// 방문 인증 허용 반경 (100, 150, 300m 중 선택)
    #[schema(example = 150)]
    pub radius_m: i32,
    pub open_hours: OpenHoursMap,
    /// 매장 4자리 PIN 코드
    #[schema(example = "1234")]
    pub pin_code: String,
    pub paused: bool,
    pub subscription_active: bool,
    #[schema(example = "2026-10-29T00:00:00Z")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerStoresResponse {
    pub stores: Vec<OwnerStoreDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerStoreResponse {
    pub store: OwnerStoreDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOwnerStoreRequest {
    pub name: Option<String>,
    pub category: Option<StoreCategory>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    /// 혜택 문구 (최대 40자, REQ-B20)
    pub benefit_text: Option<String>,
    /// 인증 반경 (100, 150, 300 중 하나, REQ-B20)
    pub radius_m: Option<i32>,
    pub open_hours: Option<OpenHoursMap>,
    /// 매장 4자리 숫자 PIN 코드
    pub pin_code: Option<String>,
    /// 노출 일시중지 여부
    pub paused: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoreStatsQuery {
    /// 조회 기간 (7 또는 30일, 기본값 7)
    #[param(example = 7)]
    pub days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyStatItem {
    #[schema(example = "2026-11-05")]
    pub date: String,
    pub impressions: i64,
    pub visits_accepted: i64,
    pub visits_verified: i64,
    pub coupons_redeemed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoreStatsTotals {
    pub impressions: i64,
    pub visits_accepted: i64,
    pub visits_verified: i64,
    pub coupons_redeemed: i64,
    /// 노출 대비 방문 수락률 (0.0 ~ 1.0)
    pub accept_rate: f64,
    /// 방문 수락 대비 실제 도착·인증률 (0.0 ~ 1.0)
    pub verify_rate: f64,
    /// 인증(쿠폰 발급) 대비 실제 매장 사용률 (0.0 ~ 1.0)
    pub redeem_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoreStatsResponse {
    pub store_id: String,
    pub days: u32,
    pub totals: StoreStatsTotals,
    pub daily: Vec<DailyStatItem>,
}
