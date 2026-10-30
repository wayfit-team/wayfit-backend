-- 방문 인증 증거: 주차 판정을 사이드브레이크에서 기어 P단으로 변경하고 지오펜스 진입을 위치 증거로 추가 (REQ-B8)
-- SQLite는 CHECK 제약을 ALTER할 수 없어 테이블을 다시 만든다. 기존 PARKING_BRAKE_ON 기록은 GEAR_PARK로 옮긴다.

CREATE TABLE visit_evidences_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  visit_id TEXT NOT NULL REFERENCES visits(id),
  type TEXT NOT NULL CHECK (type IN ('LOCATION', 'GEOFENCE_ENTER', 'WAYPOINT_ARRIVED', 'GEAR_PARK', 'DOOR_OPEN')),
  source TEXT CHECK (source IS NULL OR source IN ('FUSED', 'NAVI')),
  occurred_at TEXT NOT NULL,
  lat REAL,
  lng REAL
);

INSERT INTO visit_evidences_new (id, visit_id, type, source, occurred_at, lat, lng)
SELECT id, visit_id,
       CASE type WHEN 'PARKING_BRAKE_ON' THEN 'GEAR_PARK' ELSE type END,
       NULL, occurred_at, lat, lng
FROM visit_evidences;

DROP TABLE visit_evidences;
ALTER TABLE visit_evidences_new RENAME TO visit_evidences;
CREATE INDEX idx_visit_evidences_visit ON visit_evidences(visit_id);
