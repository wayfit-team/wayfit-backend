use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CouponStatus {
    Issued,
    Redeemed,
    Expired,
}

impl CouponStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CouponStatus::Issued => "ISSUED",
            CouponStatus::Redeemed => "REDEEMED",
            CouponStatus::Expired => "EXPIRED",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "ISSUED" => CouponStatus::Issued,
            "REDEEMED" => CouponStatus::Redeemed,
            _ => CouponStatus::Expired,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CouponDto {
    #[schema(example = "cpn_9a82c10f")]
    pub id: String,
    #[schema(example = "visit_8b12f3a0")]
    pub visit_id: String,
    #[schema(example = "demo-car-01")]
    pub device_id: String,
    #[schema(example = "store_01")]
    pub store_id: String,
    /// 계산대 제시용 6자리 숫자 코드 (REQ-B11)
    #[schema(example = "482910")]
    pub code6: String,
    #[schema(example = "호수길 로스터리")]
    pub store_name_snapshot: String,
    #[schema(example = "아메리카노 1잔 무료 증정")]
    pub benefit_snapshot: String,
    pub status: CouponStatus,
    #[schema(example = "2026-11-05T03:13:10Z")]
    pub issued_at: String,
    #[schema(example = "2026-11-12T03:13:10Z")]
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CouponListResponse {
    pub coupons: Vec<CouponDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedeemCouponRequest {
    /// 매장 직원 확인용 4자리 PIN 코드 (REQ-B13)
    #[schema(example = "1234")]
    pub pin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedeemCouponResponse {
    pub coupon: CouponDto,
}
