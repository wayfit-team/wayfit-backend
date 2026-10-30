use uuid::Uuid;

/// Generate a prefixed UUID (e.g., "visit_...", "cpn_...", "imp_...")
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

/// Generate a 6-digit numeric display code for coupons (REQ-B11)
pub fn generate_coupon_code6() -> String {
    let mut buf = [0u8; 4];
    let _ = getrandom::getrandom(&mut buf);
    let num = u32::from_le_bytes(buf) % 1_000_000;
    format!("{:06}", num)
}

/// Generate an 8-digit numeric pairing code (REQ-B15).
/// 차 화면의 코드를 폰 숫자 키패드로 옮겨 입력하기 쉽도록 숫자만 쓴다.
pub fn generate_pairing_code8() -> String {
    let mut buf = [0u8; 8];
    let _ = getrandom::getrandom(&mut buf);
    let num = u64::from_le_bytes(buf) % 100_000_000;
    format!("{:08}", num)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generators() {
        let id = new_id("visit");
        assert!(id.starts_with("visit_"));
        let code6 = generate_coupon_code6();
        assert_eq!(code6.len(), 6);
        assert!(code6.chars().all(|c| c.is_ascii_digit()));
        let code8 = generate_pairing_code8();
        assert_eq!(code8.len(), 8);
        assert!(code8.chars().all(|c| c.is_ascii_digit()));
    }
}
