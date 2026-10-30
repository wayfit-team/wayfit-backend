use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::schemas::coupons::{CouponDto, CouponStatus};
use crate::schemas::owner::OwnerStoreDto;
use crate::schemas::stores::StoreCategory;
use crate::schemas::visits::{VisitStatus, VisitSummary};
use crate::time::parse_rfc3339;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRow {
    pub id: String,
    pub owner_kakao_id: String,
    pub name: String,
    pub category: String,
    pub address: String,
    pub lat: f64,
    pub lng: f64,
    pub benefit_text: String,
    pub radius_m: i32,
    pub open_hours_json: String,
    pub pin_code: String,
    pub paused: i32,
    pub subscription_active: i32,
    pub created_at: String,
}

impl StoreRow {
    pub fn into_owner_dto(self) -> OwnerStoreDto {
        let open_hours = serde_json::from_str(&self.open_hours_json).unwrap_or_else(|_| HashMap::new());
        OwnerStoreDto {
            id: self.id,
            owner_kakao_id: self.owner_kakao_id,
            name: self.name,
            category: StoreCategory::from_db(&self.category),
            address: self.address,
            lat: self.lat,
            lng: self.lng,
            benefit_text: self.benefit_text,
            radius_m: self.radius_m,
            open_hours,
            pin_code: self.pin_code,
            paused: self.paused != 0,
            subscription_active: self.subscription_active != 0,
            created_at: self.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisitRow {
    pub id: String,
    pub device_id: String,
    pub store_id: String,
    pub trip_id: String,
    pub status: String,
    pub created_at: String,
    pub kst_date: String,
    pub resolved_at: Option<String>,
    pub reject_reason: Option<String>,
    pub confidence: Option<i32>,
}

impl VisitRow {
    /// REQ-B7: 생성 후 3시간 안에 인증되지 않은 ACCEPTED 방문은 EXPIRED로 본다
    pub fn effective_status(&self, now: DateTime<Utc>) -> VisitStatus {
        let raw = VisitStatus::from_db(&self.status);
        if raw == VisitStatus::Accepted {
            if let Some(created) = parse_rfc3339(&self.created_at) {
                if now - created > Duration::hours(3) {
                    return VisitStatus::Expired;
                }
            }
        }
        raw
    }

    pub fn to_summary(&self, now: DateTime<Utc>) -> VisitSummary {
        VisitSummary {
            id: self.id.clone(),
            store_id: self.store_id.clone(),
            trip_id: self.trip_id.clone(),
            status: self.effective_status(now),
            created_at: self.created_at.clone(),
            resolved_at: self.resolved_at.clone(),
            reject_reason: self.reject_reason.clone(),
            confidence: self.confidence,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouponRow {
    pub id: String,
    pub visit_id: String,
    pub device_id: String,
    pub store_id: String,
    pub code6: String,
    pub store_name_snapshot: String,
    pub benefit_snapshot: String,
    pub status: String,
    pub issued_at: String,
    pub issued_kst_date: String,
    pub expires_at: String,
    pub redeemed_at: Option<String>,
    pub redeemed_kst_date: Option<String>,
    pub pin_fail_count: i32,
    pub locked_until: Option<String>,
}

impl CouponRow {
    /// REQ-B14: 유효기간이 지난 ISSUED 쿠폰은 EXPIRED로 응답한다
    pub fn effective_status(&self, now: DateTime<Utc>) -> CouponStatus {
        let raw = CouponStatus::from_db(&self.status);
        if raw == CouponStatus::Issued {
            if let Some(exp) = parse_rfc3339(&self.expires_at) {
                if now > exp {
                    return CouponStatus::Expired;
                }
            }
        }
        raw
    }

    pub fn into_dto(self, now: DateTime<Utc>) -> CouponDto {
        let status = self.effective_status(now);
        CouponDto {
            id: self.id,
            visit_id: self.visit_id,
            device_id: self.device_id,
            store_id: self.store_id,
            code6: self.code6,
            store_name_snapshot: self.store_name_snapshot,
            benefit_snapshot: self.benefit_snapshot,
            status,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            redeemed_at: self.redeemed_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCodeRow {
    pub code: String,
    pub device_id: String,
    pub expires_at: String,
    pub used_at: Option<String>,
    pub used_by_kakao_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeviceRow {
    pub kakao_user_id: String,
    pub device_id: String,
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdRow {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreIdRow {
    pub store_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountRow {
    pub cnt: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateCountRow {
    pub kst_date: String,
    pub cnt: i64,
}
