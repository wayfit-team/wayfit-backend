use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::verify::EvidenceType;
use crate::schemas::coupons::CouponDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisitStatus {
    Accepted,
    Verified,
    Rejected,
    Cancelled,
    Expired,
}

impl VisitStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VisitStatus::Accepted => "ACCEPTED",
            VisitStatus::Verified => "VERIFIED",
            VisitStatus::Rejected => "REJECTED",
            VisitStatus::Cancelled => "CANCELLED",
            VisitStatus::Expired => "EXPIRED",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "ACCEPTED" => VisitStatus::Accepted,
            "VERIFIED" => VisitStatus::Verified,
            "REJECTED" => VisitStatus::Rejected,
            "CANCELLED" => VisitStatus::Cancelled,
            _ => VisitStatus::Expired,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateVisitRequest {
    #[schema(example = "store_01")]
    pub store_id: String,
    #[schema(example = "trip_20261105_001")]
    pub trip_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VisitSummary {
    #[schema(example = "visit_8b12f3a0")]
    pub id: String,
    #[schema(example = "store_01")]
    pub store_id: String,
    #[schema(example = "trip_20261105_001")]
    pub trip_id: String,
    pub status: VisitStatus,
    #[schema(example = "2026-11-05T03:00:00Z")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 100)]
    pub confidence: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateVisitResponse {
    pub visit: VisitSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDto {
    #[serde(rename = "type")]
    pub evidence_type: EvidenceType,
    #[schema(example = "2026-11-05T03:12:40Z")]
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 37.2825)]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 127.0530)]
    pub lng: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyVisitRequest {
    pub evidence: Vec<EvidenceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyVisitResponse {
    pub visit: VisitSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupon: Option<CouponDto>,
}
