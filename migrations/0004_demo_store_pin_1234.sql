-- 데모 매장 직원 확인 PIN 을 1234 로 통일한다 (매장마다 달라 시연 때 헷갈림).
-- 시드(scripts/seed.sql)를 다시 돌리면 쿠폰·방문이 모두 지워지므로 운영 DB 는 이 마이그레이션으로 맞춘다.
-- 사장님이 직접 바꾼 PIN 은 건드리지 않도록 시드 기본값(5678·9999)인 데모 매장만 바꾼다.
UPDATE stores
SET pin_code = '1234'
WHERE id IN ('store_04', 'store_05', 'store_06', 'store_07', 'store_08', 'store_09')
  AND pin_code IN ('5678', '9999');
