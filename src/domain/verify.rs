use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::corridor::haversine_m;
use crate::error::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceType {
    Location,
    /// Pleos FusedLocation 근접 알림(지오펜스) 진입. 좌표가 있으면 LOCATION과 똑같이 반경 판정에 쓴다.
    GeofenceEnter,
    WaypointArrived,
    /// 기어 P단 (Pleos Vehicle `VehicleGear.GEAR_PARK`). 구 클라이언트 호환을 위해 `PARKING_BRAKE_ON`도 받는다.
    #[serde(alias = "PARKING_BRAKE_ON")]
    GearPark,
    DoorOpen,
}

impl EvidenceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceType::Location => "LOCATION",
            EvidenceType::GeofenceEnter => "GEOFENCE_ENTER",
            EvidenceType::WaypointArrived => "WAYPOINT_ARRIVED",
            EvidenceType::GearPark => "GEAR_PARK",
            EvidenceType::DoorOpen => "DOOR_OPEN",
        }
    }
}

/// 위치 증거의 출처 (선택). 판정에는 쓰지 않고 분석용으로 저장한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceSource {
    /// Pleos FusedLocation
    Fused,
    /// Pleos NaviHelper 현재 위치
    Navi,
}

impl EvidenceSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvidenceSource::Fused => "FUSED",
            EvidenceSource::Navi => "NAVI",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvidenceItem {
    pub evidence_type: EvidenceType,
    pub occurred_at: DateTime<Utc>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct VerifyContext {
    pub visit_created_at: DateTime<Utc>,
    pub server_now: DateTime<Utc>,
    pub store_lat: f64,
    pub store_lng: f64,
    pub store_radius_m: f64,
    pub already_issued_today: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    Verified {
        confidence: i32,
    },
    Rejected {
        reason: ErrorCode,
        message: &'static str,
    },
}

/// Pure verification state machine implementing REQ-B8, REQ-B9, EDGE-B1, EDGE-B2, EDGE-B3.
pub fn evaluate_visit_verification(
    ctx: &VerifyContext,
    evidences: &[EvidenceItem],
) -> VerifyOutcome {
    // REQ-B9: 같은 기기·같은 매장 쿠폰은 하루(KST 자정 기준) 1장
    if ctx.already_issued_today {
        return VerifyOutcome::Rejected {
            reason: ErrorCode::DailyLimit,
            message: "오늘 이미 해당 매장의 쿠폰을 발급받았습니다 (1일 1장 제한)",
        };
    }

    if evidences.is_empty() {
        return VerifyOutcome::Rejected {
            reason: ErrorCode::VisitOutOfRadius,
            message: "제출된 방문 인증 증거가 없습니다",
        };
    }

    // EDGE-B1: 서버 수신이 증거 발생 후 24시간을 넘으면 거절
    for ev in evidences {
        if ctx.server_now - ev.occurred_at > Duration::hours(24) {
            return VerifyOutcome::Rejected {
                reason: ErrorCode::VisitTimeout,
                message: "증거 발생 후 24시간이 경과하여 인증할 수 없습니다",
            };
        }
    }

    // EDGE-B2: 차량 시계가 서버와 10분 넘게 어긋나면 서버 수신 시각으로 3시간 조건 판정
    let latest_occurred = evidences
        .iter()
        .map(|e| e.occurred_at)
        .max()
        .unwrap_or(ctx.server_now);
    let clock_skew = (ctx.server_now - latest_occurred).abs();
    let effective_time = if clock_skew > Duration::minutes(10) {
        ctx.server_now
    } else {
        latest_occurred
    };

    // REQ-B8(c): 방문 생성 후 3시간 이내 발생
    let elapsed = effective_time - ctx.visit_created_at;
    if elapsed > Duration::hours(3) || elapsed < Duration::minutes(-5) {
        return VerifyOutcome::Rejected {
            reason: ErrorCode::VisitTimeout,
            message: "방문 생성 후 3시간 이내에 인증되지 않았습니다",
        };
    }

    // REQ-B8(a) & EDGE-B3: 증거 위치 중 매장과 가장 가까운 지점 기준 반경 이내 판정
    let min_dist_m = evidences
        .iter()
        .filter_map(|e| match (e.lat, e.lng) {
            (Some(lat), Some(lng)) => Some(haversine_m(ctx.store_lat, ctx.store_lng, lat, lng)),
            _ => None,
        })
        .fold(f64::INFINITY, f64::min);

    if min_dist_m > ctx.store_radius_m {
        return VerifyOutcome::Rejected {
            reason: ErrorCode::VisitOutOfRadius,
            message: "매장 인증 반경 내 위치 증거가 확인되지 않았습니다",
        };
    }

    // REQ-B8(b): 매장 근처에서 기어 P단(주차) 기록 확인
    let has_gear_park = evidences.iter().any(|e| {
        if e.evidence_type != EvidenceType::GearPark {
            return false;
        }
        match (e.lat, e.lng) {
            (Some(lat), Some(lng)) => {
                haversine_m(ctx.store_lat, ctx.store_lng, lat, lng) <= ctx.store_radius_m
            }
            // P단 이벤트 자체에 좌표가 생략된 경우, 앞서 반경 내 위치 증거가 확인되었으므로 인정
            _ => true,
        }
    });

    if !has_gear_park {
        return VerifyOutcome::Rejected {
            reason: ErrorCode::VisitNoParking,
            message: "매장 반경 내에서 기어 P단(주차)이 확인되지 않았습니다",
        };
    }

    // 가점: 기본 60점 + WAYPOINT_ARRIVED(20점) + DOOR_OPEN(20점) = 최대 100점
    let mut confidence = 60;
    if evidences
        .iter()
        .any(|e| e.evidence_type == EvidenceType::WaypointArrived)
    {
        confidence += 20;
    }
    if evidences
        .iter()
        .any(|e| e.evidence_type == EvidenceType::DoorOpen)
    {
        confidence += 20;
    }

    VerifyOutcome::Verified { confidence }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::parse_rfc3339;

    fn base_ctx() -> VerifyContext {
        VerifyContext {
            visit_created_at: parse_rfc3339("2026-11-05T03:00:00Z").unwrap(),
            server_now: parse_rfc3339("2026-11-05T03:15:00Z").unwrap(),
            store_lat: 37.2800,
            store_lng: 127.0440,
            store_radius_m: 150.0,
            already_issued_today: false,
        }
    }

    #[test]
    fn test_verify_full_evidence_confidence_100() {
        let ctx = base_ctx();
        let ev = vec![
            EvidenceItem {
                evidence_type: EvidenceType::Location,
                occurred_at: parse_rfc3339("2026-11-05T03:12:00Z").unwrap(),
                lat: Some(37.2801),
                lng: Some(127.0441),
            },
            EvidenceItem {
                evidence_type: EvidenceType::WaypointArrived,
                occurred_at: parse_rfc3339("2026-11-05T03:12:10Z").unwrap(),
                lat: None,
                lng: None,
            },
            EvidenceItem {
                evidence_type: EvidenceType::GearPark,
                occurred_at: parse_rfc3339("2026-11-05T03:12:40Z").unwrap(),
                lat: Some(37.2801),
                lng: Some(127.0441),
            },
            EvidenceItem {
                evidence_type: EvidenceType::DoorOpen,
                occurred_at: parse_rfc3339("2026-11-05T03:13:05Z").unwrap(),
                lat: None,
                lng: None,
            },
        ];

        assert_eq!(
            evaluate_visit_verification(&ctx, &ev),
            VerifyOutcome::Verified { confidence: 100 }
        );
    }

    #[test]
    fn test_verify_gps_bounce_edge_b3_uses_closest_point() {
        let ctx = base_ctx();
        let ev = vec![
            // 1st GPS reading bounced 500m away
            EvidenceItem {
                evidence_type: EvidenceType::Location,
                occurred_at: parse_rfc3339("2026-11-05T03:12:00Z").unwrap(),
                lat: Some(37.2850),
                lng: Some(127.0440),
            },
            // 2nd GPS reading is 20m away (inside 150m radius)
            EvidenceItem {
                evidence_type: EvidenceType::Location,
                occurred_at: parse_rfc3339("2026-11-05T03:12:20Z").unwrap(),
                lat: Some(37.2801),
                lng: Some(127.0440),
            },
            EvidenceItem {
                evidence_type: EvidenceType::GearPark,
                occurred_at: parse_rfc3339("2026-11-05T03:12:40Z").unwrap(),
                lat: None,
                lng: None,
            },
        ];

        assert_eq!(
            evaluate_visit_verification(&ctx, &ev),
            VerifyOutcome::Verified { confidence: 60 }
        );
    }

    #[test]
    fn test_verify_clock_skew_edge_b2_uses_server_time() {
        let mut ctx = base_ctx();
        ctx.server_now = parse_rfc3339("2026-11-05T03:20:00Z").unwrap();
        // Vehicle clock is 2 hours ahead of server (> 10m skew), but server_now is 20m after visit_created_at
        let ev = vec![EvidenceItem {
            evidence_type: EvidenceType::GearPark,
            occurred_at: parse_rfc3339("2026-11-05T05:20:00Z").unwrap(),
            lat: Some(37.2801),
            lng: Some(127.0441),
        }];

        assert_eq!(
            evaluate_visit_verification(&ctx, &ev),
            VerifyOutcome::Verified { confidence: 60 }
        );
    }

    #[test]
    fn test_verify_rejections() {
        let ctx = base_ctx();

        // Out of radius
        let out_ev = vec![EvidenceItem {
            evidence_type: EvidenceType::GearPark,
            occurred_at: parse_rfc3339("2026-11-05T03:12:00Z").unwrap(),
            lat: Some(37.2900),
            lng: Some(127.0500),
        }];
        assert!(matches!(
            evaluate_visit_verification(&ctx, &out_ev),
            VerifyOutcome::Rejected {
                reason: ErrorCode::VisitOutOfRadius,
                ..
            }
        ));

        // No parking brake
        let no_park_ev = vec![EvidenceItem {
            evidence_type: EvidenceType::Location,
            occurred_at: parse_rfc3339("2026-11-05T03:12:00Z").unwrap(),
            lat: Some(37.2801),
            lng: Some(127.0441),
        }];
        assert!(matches!(
            evaluate_visit_verification(&ctx, &no_park_ev),
            VerifyOutcome::Rejected {
                reason: ErrorCode::VisitNoParking,
                ..
            }
        ));

        // Daily limit
        let mut limit_ctx = base_ctx();
        limit_ctx.already_issued_today = true;
        assert!(matches!(
            evaluate_visit_verification(&limit_ctx, &no_park_ev),
            VerifyOutcome::Rejected {
                reason: ErrorCode::DailyLimit,
                ..
            }
        ));
    }

    #[test]
    fn test_verify_geofence_enter_counts_as_location() {
        let ctx = base_ctx();
        let ev = vec![
            EvidenceItem {
                evidence_type: EvidenceType::GeofenceEnter,
                occurred_at: parse_rfc3339("2026-11-05T03:12:00Z").unwrap(),
                lat: Some(37.2801),
                lng: Some(127.0441),
            },
            EvidenceItem {
                evidence_type: EvidenceType::GearPark,
                occurred_at: parse_rfc3339("2026-11-05T03:12:40Z").unwrap(),
                lat: None,
                lng: None,
            },
        ];

        assert_eq!(
            evaluate_visit_verification(&ctx, &ev),
            VerifyOutcome::Verified { confidence: 60 }
        );
    }

    #[test]
    fn test_evidence_type_wire_names() {
        let parse = |s: &str| serde_json::from_str::<EvidenceType>(&format!("\"{}\"", s)).unwrap();
        assert_eq!(parse("GEAR_PARK"), EvidenceType::GearPark);
        assert_eq!(parse("GEOFENCE_ENTER"), EvidenceType::GeofenceEnter);
        // 구 클라이언트 호환
        assert_eq!(parse("PARKING_BRAKE_ON"), EvidenceType::GearPark);
        assert_eq!(
            serde_json::to_string(&EvidenceType::GearPark).unwrap(),
            "\"GEAR_PARK\""
        );
        for t in [
            EvidenceType::Location,
            EvidenceType::GeofenceEnter,
            EvidenceType::WaypointArrived,
            EvidenceType::GearPark,
            EvidenceType::DoorOpen,
        ] {
            assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{}\"", t.as_str()));
        }
    }
}
