-- WayFit Cloudflare D1 Schema (7 Core Domain Tables · Kakao OAuth Unification)

-- 1. 제휴 매장 (REQ-B2, B20, B23) — 사장님은 owner_kakao_id로 직접 매칭
CREATE TABLE stores (
  id TEXT PRIMARY KEY,
  owner_kakao_id TEXT NOT NULL,         -- 사장님 카카오 OAuth 회원번호
  name TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN ('CAFE', 'FOOD', 'BAKERY', 'ETC')),
  address TEXT NOT NULL,
  lat REAL NOT NULL,
  lng REAL NOT NULL,
  benefit_text TEXT NOT NULL,           -- 최대 40자
  radius_m INTEGER NOT NULL DEFAULT 150 CHECK (radius_m IN (100, 150, 300)),
  open_hours_json TEXT NOT NULL,        -- 요일별 시작/종료 JSON
  pin_code TEXT NOT NULL,               -- 매장 4자리 PIN 코드
  paused INTEGER NOT NULL DEFAULT 0,    -- 0: 정상, 1: 일시중지
  subscription_active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_stores_lat_lng ON stores(lat, lng);
CREATE INDEX idx_stores_owner_kakao ON stores(owner_kakao_id);

-- 2. QR 페어링 코드 (REQ-B15, EDGE-B5)
CREATE TABLE pairing_codes (
  code TEXT PRIMARY KEY,                -- 8자리 코드 (10분 유효, 1회용)
  device_id TEXT NOT NULL,              -- 차량 앱 생성 UUID
  expires_at TEXT NOT NULL,
  used_at TEXT,
  used_by_kakao_id TEXT                 -- 스캔하여 연결한 운전자 카카오 ID
);

-- 3. 카카오 사용자 ↔ 차량 기기 다대다 연결 (REQ-B16~B18)
CREATE TABLE user_devices (
  kakao_user_id TEXT NOT NULL,          -- 운전자 카카오 OAuth 회원번호
  device_id TEXT NOT NULL,              -- 차량 앱 생성 UUID
  linked_at TEXT NOT NULL,
  PRIMARY KEY (kakao_user_id, device_id)
);
CREATE INDEX idx_user_devices_device ON user_devices(device_id);

-- 4. 추천 카드 노출 기록 (REQ-B4: 기기·매장·트립 당 1회 중복 제거)
CREATE TABLE impressions (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL,
  store_id TEXT NOT NULL REFERENCES stores(id),
  trip_id TEXT NOT NULL,
  shown_at TEXT NOT NULL,
  kst_date TEXT NOT NULL,               -- 'YYYY-MM-DD'
  UNIQUE(device_id, store_id, trip_id)
);
CREATE INDEX idx_impressions_store_date ON impressions(store_id, kst_date);

-- 5. 방문 (REQ-B5~B10)
CREATE TABLE visits (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL,
  store_id TEXT NOT NULL REFERENCES stores(id),
  trip_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ACCEPTED', 'VERIFIED', 'REJECTED', 'CANCELLED', 'EXPIRED')),
  created_at TEXT NOT NULL,
  kst_date TEXT NOT NULL,               -- 'YYYY-MM-DD'
  resolved_at TEXT,
  reject_reason TEXT,
  confidence INTEGER
);
CREATE INDEX idx_visits_device_status ON visits(device_id, status);
CREATE INDEX idx_visits_store_date ON visits(store_id, kst_date);

-- 6. 방문 인증 증거 (REQ-B8, NFR-B3)
CREATE TABLE visit_evidences (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  visit_id TEXT NOT NULL REFERENCES visits(id),
  type TEXT NOT NULL CHECK (type IN ('LOCATION', 'WAYPOINT_ARRIVED', 'PARKING_BRAKE_ON', 'DOOR_OPEN')),
  occurred_at TEXT NOT NULL,
  lat REAL,
  lng REAL
);
CREATE INDEX idx_visit_evidences_visit ON visit_evidences(visit_id);

-- 7. 쿠폰 (REQ-B11~B14, EDGE-B6~B8)
CREATE TABLE coupons (
  id TEXT PRIMARY KEY,
  visit_id TEXT NOT NULL UNIQUE REFERENCES visits(id),
  device_id TEXT NOT NULL,
  store_id TEXT NOT NULL REFERENCES stores(id),
  code6 TEXT NOT NULL,                  -- 계산대 제시용 6자리 코드
  store_name_snapshot TEXT NOT NULL,
  benefit_snapshot TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ISSUED', 'REDEEMED', 'EXPIRED')),
  issued_at TEXT NOT NULL,
  issued_kst_date TEXT NOT NULL,        -- KST 기준 일일 1장 제한 검사 및 통계용
  expires_at TEXT NOT NULL,             -- 기본 발급 후 7일
  redeemed_at TEXT,
  redeemed_kst_date TEXT,
  pin_fail_count INTEGER NOT NULL DEFAULT 0,
  locked_until TEXT
);
CREATE INDEX idx_coupons_device_store_date ON coupons(device_id, store_id, issued_kst_date);
CREATE INDEX idx_coupons_store_redeemed ON coupons(store_id, redeemed_kst_date);
