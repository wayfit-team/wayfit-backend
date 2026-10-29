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

/// Generate an 8-character unambiguous alphanumeric pairing code (REQ-B15)
pub fn generate_pairing_code8() -> String {
    const CHARSET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut buf = [0u8; 8];
    let _ = getrandom::getrandom(&mut buf);
    buf.iter()
        .map(|b| CHARSET[(*b as usize) % CHARSET.len()] as char)
        .collect()
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
    }
}
