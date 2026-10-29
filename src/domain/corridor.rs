/// Earth radius in meters
const EARTH_RADIUS_M: f64 = 6_371_000.0;

#[derive(Debug, Clone, Copy)]
pub struct CorridorQuery {
    pub lat: f64,
    pub lng: f64,
    pub heading: Option<f64>,
    pub dest_lat: Option<f64>,
    pub dest_lng: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorridorMatch {
    /// Straight-line distance from current vehicle position (meters)
    pub distance_m: i64,
    /// Additional detour distance (meters)
    pub detour_m: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lng: f64,
    pub max_lng: f64,
}

/// Compute a generous latitude/longitude bounding box for D1 index filtering
pub fn compute_bbox(q: &CorridorQuery) -> BoundingBox {
    // 1 degree lat is ~111km; 0.06 deg is ~6.6km margin
    let margin = 0.06;
    let (min_lat, max_lat, min_lng, max_lng) = match (q.dest_lat, q.dest_lng) {
        (Some(dlat), Some(dlng)) => (
            q.lat.min(dlat) - margin,
            q.lat.max(dlat) + margin,
            q.lng.min(dlng) - margin,
            q.lng.max(dlng) + margin,
        ),
        _ => (
            q.lat - margin,
            q.lat + margin,
            q.lng - margin,
            q.lng + margin,
        ),
    };
    BoundingBox {
        min_lat,
        max_lat,
        min_lng,
        max_lng,
    }
}

/// Haversine distance in meters between two WGS84 coordinates
pub fn haversine_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let rlat1 = lat1.to_radians();
    let rlat2 = lat2.to_radians();

    let a = (dlat / 2.0).sin().powi(2)
        + rlat1.cos() * rlat2.cos() * (dlng / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_M * c
}

/// Convert (lat, lng) relative to origin (lat0, lng0) into local planar (east_m, north_m)
fn to_local_xy_m(lat0: f64, lng0: f64, lat: f64, lng: f64) -> (f64, f64) {
    let north_m = (lat - lat0).to_radians() * EARTH_RADIUS_M;
    let east_m = (lng - lng0).to_radians() * EARTH_RADIUS_M * lat0.to_radians().cos();
    (east_m, north_m)
}

/// Initial bearing in degrees [0, 360) from (lat1, lng1) to (lat2, lng2)
pub fn bearing_deg(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let (east, north) = to_local_xy_m(lat1, lng1, lat2, lng2);
    let angle_rad = east.atan2(north); // 0 = North, pi/2 = East
    let deg = angle_rad.to_degrees();
    (deg + 360.0) % 360.0
}

/// Smallest angular difference in degrees [0, 180]
pub fn angle_diff_deg(a: f64, b: f64) -> f64 {
    let diff = (a - b).abs() % 360.0;
    if diff > 180.0 {
        360.0 - diff
    } else {
        diff
    }
}

/// Evaluate whether a candidate store at `(store_lat, store_lng)` satisfies REQ-B2 & EDGE-B9.
/// Returns `Some(CorridorMatch)` if eligible, or `None` if outside corridor.
pub fn evaluate_store_corridor(
    q: &CorridorQuery,
    store_lat: f64,
    store_lng: f64,
) -> Option<CorridorMatch> {
    let dist_curr_to_store = haversine_m(q.lat, q.lng, store_lat, store_lng);

    if let (Some(dest_lat), Some(dest_lng)) = (q.dest_lat, q.dest_lng) {
        let total_route_m = haversine_m(q.lat, q.lng, dest_lat, dest_lng);
        // EDGE-B9: 목적지가 현재 위치에서 300m 이내면 빈 결과
        if total_route_m <= 300.0 {
            return None;
        }

        // REQ-B2: 현재 위치에서 300m ~ 5km 앞에 있는 매장
        if !(300.0..=5000.0).contains(&dist_curr_to_store) {
            return None;
        }

        let (bx, by) = to_local_xy_m(q.lat, q.lng, dest_lat, dest_lng);
        let (px, py) = to_local_xy_m(q.lat, q.lng, store_lat, store_lng);
        let ab_len_sq = bx * bx + by * by;
        if ab_len_sq <= 1e-6 {
            return None;
        }

        // Projection progress t along A -> B: must be in [0.0, 1.0]
        let t = (px * bx + py * by) / ab_len_sq;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }

        // Perpendicular distance to segment A -> B: must be <= 1000m (1km)
        let proj_x = t * bx;
        let proj_y = t * by;
        let perp_dist_m = ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt();
        if perp_dist_m > 1000.0 {
            return None;
        }

        let dist_store_to_dest = haversine_m(store_lat, store_lng, dest_lat, dest_lng);
        let detour_m = (dist_curr_to_store + dist_store_to_dest - total_route_m).max(0.0);

        Some(CorridorMatch {
            distance_m: dist_curr_to_store.round() as i64,
            detour_m: detour_m.round() as i64,
        })
    } else {
        // REQ-B2 (no destination): 반경 2km 이내
        if dist_curr_to_store > 2000.0 {
            return None;
        }

        // 진행 방향이 있으면 ±60° 이내
        if let Some(heading) = q.heading {
            if dist_curr_to_store > 1.0 {
                let brg = bearing_deg(q.lat, q.lng, store_lat, store_lng);
                if angle_diff_deg(heading, brg) > 60.0 {
                    return None;
                }
            }
        }

        let d = dist_curr_to_store.round() as i64;
        Some(CorridorMatch {
            distance_m: d,
            detour_m: d,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 아주대학교 정문: (37.2792, 127.0437)
    // 광교호수공원:   (37.2865, 127.0650) (~2.05km ENE)
    const AJOU_LAT: f64 = 37.2792;
    const AJOU_LNG: f64 = 127.0437;
    const LAKE_LAT: f64 = 37.2865;
    const LAKE_LNG: f64 = 127.0650;

    #[test]
    fn test_corridor_with_destination_matches_midpoint_store() {
        let q = CorridorQuery {
            lat: AJOU_LAT,
            lng: AJOU_LNG,
            heading: None,
            dest_lat: Some(LAKE_LAT),
            dest_lng: Some(LAKE_LNG),
        };

        // Store near midpoint (~1km ahead, slightly off line)
        let res = evaluate_store_corridor(&q, 37.2830, 127.0545);
        assert!(res.is_some());
        let m = res.unwrap();
        assert!(m.distance_m >= 300 && m.distance_m <= 5000);
        assert!(m.detour_m < 200);
    }

    #[test]
    fn test_corridor_edge_b9_destination_within_300m_returns_none() {
        let q = CorridorQuery {
            lat: AJOU_LAT,
            lng: AJOU_LNG,
            heading: None,
            dest_lat: Some(37.2800), // ~90m away
            dest_lng: Some(127.0440),
        };
        assert!(evaluate_store_corridor(&q, 37.2796, 127.0438).is_none());
    }

    #[test]
    fn test_corridor_excludes_behind_or_too_close_store() {
        let q = CorridorQuery {
            lat: AJOU_LAT,
            lng: AJOU_LNG,
            heading: None,
            dest_lat: Some(LAKE_LAT),
            dest_lng: Some(LAKE_LNG),
        };
        // Store 100m ahead (< 300m minimum)
        assert!(evaluate_store_corridor(&q, 37.2795, 127.0445).is_none());
        // Store behind current position (west-southwest)
        assert!(evaluate_store_corridor(&q, 37.2750, 127.0300).is_none());
    }

    #[test]
    fn test_no_destination_with_heading_cone() {
        let q = CorridorQuery {
            lat: AJOU_LAT,
            lng: AJOU_LNG,
            heading: Some(0.0), // Due North
            dest_lat: None,
            dest_lng: None,
        };
        // Store 800m due North -> inside ±60°
        assert!(evaluate_store_corridor(&q, 37.2865, AJOU_LNG).is_some());
        // Store 800m due South -> outside ±60°
        assert!(evaluate_store_corridor(&q, 37.2710, AJOU_LNG).is_none());
    }
}
