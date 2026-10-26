//! D19 know-how decryption (hub cipher.rs mirror). Key arrives with the
//! authenticated manifest; plaintext lives only inside this process.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};

pub fn open(key_hex: &str, name: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let key = hex_decode32(key_hex)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    if bytes.len() < 7 + 12 {
        return Err("ciphertext too short".into());
    }
    let nonce = &bytes[7..7 + 12];
    let ct = &bytes[7 + 12..];
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload { msg: ct, aad: name.as_bytes() },
        )
        .map_err(|_| "decrypt failed (wrong key or not this flow)".into())
}

fn hex_decode32(s: &str) -> Result<[u8; 32], String> {
    let mut out = [0u8; 32];
    let b = s.as_bytes();
    if b.len() != 64 {
        return Err("key_hex must be 32 bytes hex".into());
    }
    for i in 0..32 {
        let hi = (b[i * 2] as char).to_digit(16).ok_or("bad hex")?;
        let lo = (b[i * 2 + 1] as char).to_digit(16).ok_or("bad hex")?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_cross_flow_rejection() {
        // Mirror of hub cipher.rs: key = sha256(master:name) — reproduce it
        // locally for the test vector.
        use sha2::{Digest, Sha256};
        let master = "test-master";
        let name = "demo-flow";
        let mut h = Sha256::new();
        h.update(master.as_bytes());
        h.update(b":");
        h.update(name.as_bytes());
        let key: [u8; 32] = h.finalize().into();
        let key_hex: String = key.iter().map(|b| format!("{b:02x}")).collect();

        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).unwrap();
        let mut pt = b"window.__knowhow = 'stealth body'".to_vec();
        pt.extend(std::iter::repeat_n(0u8, 5000)); // size beyond trivial
        let sealed = cipher
            .encrypt(
                aes_gcm::Nonce::from_slice(b"123456789012"),
                aes_gcm::aead::Payload { msg: &pt, aad: name.as_bytes() },
            )
            .unwrap();
        let mut blob = b"DITENC1".to_vec();
        blob.extend_from_slice(b"123456789012");
        blob.extend_from_slice(&sealed);

        // Right flow opens; a swapped flow name (AAD) does not.
        let opened = open(&key_hex, name, &blob).unwrap();
        assert_eq!(opened, pt);
        assert!(open(&key_hex, "other-flow", &blob).is_err());
        // Wrong key rejected.
        assert!(open(&"0".repeat(64), name, &blob).is_err());
    }
}
