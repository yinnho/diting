// ---------------------------------------------------------------------------
// WebCrypto (crypto.subtle) symmetric primitives, split from ops/mod.rs to
// keep the god file under its ratchet cap (batch 218). Stateless ops: the JS
// shim in bootstrap.js owns the CryptoKey objects and hands raw bytes plus
// normalized algorithm parameters to each op.
// ---------------------------------------------------------------------------

use super::*;

/// Real WebCrypto `crypto.subtle.digest`. `algorithm` is the SubtleCrypto
/// algorithm name (`SHA-1` / `SHA-256` / `SHA-384` / `SHA-512`); unknown
/// names fall through to SHA-256 to match the previous JS fallback. Returns
/// the raw digest bytes so the JS shim can hand them back as an ArrayBuffer.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_digest(#[string] algorithm: &str, #[buffer] data: &[u8]) -> Vec<u8> {
    use sha1::Digest as _;
    let alg = algorithm.to_ascii_uppercase();
    match alg.as_str() {
        "SHA-1" => sha1::Sha1::digest(data).to_vec(),
        "SHA-256" => sha2::Sha256::digest(data).to_vec(),
        "SHA-384" => sha2::Sha384::digest(data).to_vec(),
        "SHA-512" => sha2::Sha512::digest(data).to_vec(),
        "SHA-512/224" => sha2::Sha512_224::digest(data).to_vec(),
        "SHA-512/256" => sha2::Sha512_256::digest(data).to_vec(),
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// WebCrypto (crypto.subtle) secret-key primitives.
//
// These ops are stateless. The JS shim in bootstrap.js owns the CryptoKey
// objects and their raw key bytes; it hands the bytes plus normalized algorithm
// parameters to these ops for each operation. Only secret-key algorithms live
// here (HMAC, AES-GCM/CBC/CTR, PBKDF2, HKDF); public-key algorithms are rejected
// in the shim. A fallible op returns a JsErrorBox that the shim turns into the
// appropriate DOMException (OperationError for a bad tag or padding, etc.).
// ---------------------------------------------------------------------------

pub(crate) fn crypto_err(msg: impl std::fmt::Display) -> deno_error::JsErrorBox {
    deno_error::JsErrorBox::generic(msg.to_string())
}

/// HMAC sign. `hash` is a normalized SubtleCrypto hash name; any key length is
/// accepted (HMAC pads or hashes the key per RFC 2104). Returns the MAC bytes;
/// the shim does the constant-time-insensitive compare for `verify`.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_hmac(
    #[string] hash: &str,
    #[buffer] key: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use hmac::{Hmac, KeyInit, Mac};
    macro_rules! run {
        ($d:ty) => {{
            let mut mac = Hmac::<$d>::new_from_slice(key).map_err(crypto_err)?;
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }};
    }
    Ok(match hash {
        "SHA-1" => run!(sha1::Sha1),
        "SHA-256" => run!(sha2::Sha256),
        "SHA-384" => run!(sha2::Sha384),
        "SHA-512" => run!(sha2::Sha512),
        _ => return Err(crypto_err("unsupported HMAC hash")),
    })
}

/// Chrome 157's faces for cipher failures the JS side cannot see coming:
/// AES-CTR's input outgrowing its counter (a DataError there).
pub(crate) const AES_CTR_OVERFLOW: &str = "AES-CTR counter overflow";

/// One AES block encryption for the key's size — the core GCM/CTR are built on.
type AesBlockFn = Box<dyn Fn([u8; 16]) -> [u8; 16]>;
fn aes_block(key: &[u8]) -> Result<AesBlockFn, deno_error::JsErrorBox> {
    use aes::cipher::{BlockCipherEncrypt, KeyInit};
    macro_rules! with {
        ($c:ty) => {{
            let c = <$c>::new_from_slice(key).map_err(crypto_err)?;
            Box::new(move |b: [u8; 16]| {
                let mut blk = aes::Block::from(b);
                c.encrypt_block(&mut blk);
                blk.into()
            })
        }};
    }
    Ok(match key.len() {
        16 => with!(aes::Aes128),
        24 => with!(aes::Aes192),
        32 => with!(aes::Aes256),
        _ => return Err(crypto_err("AES key must be 128, 192, or 256 bits")),
    })
}

/// AES-GCM per NIST SP 800-38D, by hand: Chrome 157 takes any non-empty IV
/// (J0 = GHASH of the IV unless it is 96 bits) and tags of 4..=16 bytes
/// (the full tag's prefix); aes-gcm pins both as type parameters. `tag_len`
/// is in bytes and already validated (32..128 bits, Chrome's set). Decrypt
/// input is ciphertext || tag, checked to hold the tag on the JS side. Any
/// authentication failure is Chrome's empty-message OperationError.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_aes_gcm(
    encrypt: bool,
    #[buffer] key: &[u8],
    #[buffer] iv: &[u8],
    #[buffer] aad: &[u8],
    #[buffer] data: &[u8],
    tag_len: u32,
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use ghash::universal_hash::UniversalHash;
    let t = tag_len as usize;
    if iv.is_empty() || !(4..=16).contains(&t) {
        return Err(crypto_err(""));
    }
    let enc = aes_block(key)?;
    let h = enc([0u8; 16]);
    let ghash = |chunks: &[&[u8]]| -> [u8; 16] {
        let mut g = ghash::GHash::new(&ghash::Key::from(h));
        for c in chunks {
            g.update_padded(c);
        }
        g.finalize().into()
    };
    let bits = |n: usize| ((n as u64) * 8).to_be_bytes();
    let j0: [u8; 16] = if iv.len() == 12 {
        let mut j = [0u8; 16];
        j[..12].copy_from_slice(iv);
        j[15] = 1;
        j
    } else {
        let mut lens = [0u8; 16];
        lens[8..].copy_from_slice(&bits(iv.len()));
        ghash(&[iv, &lens])
    };
    let inc32 = |b: &mut [u8; 16]| {
        let n = u32::from_be_bytes([b[12], b[13], b[14], b[15]]).wrapping_add(1);
        b[12..].copy_from_slice(&n.to_be_bytes());
    };
    let gctr = |input: &[u8]| -> Vec<u8> {
        let mut ctr = j0;
        let mut out = Vec::with_capacity(input.len());
        for chunk in input.chunks(16) {
            inc32(&mut ctr);
            let ks = enc(ctr);
            out.extend(chunk.iter().zip(ks.iter()).map(|(a, b)| a ^ b));
        }
        out
    };
    let tag_of = |ct: &[u8]| -> [u8; 16] {
        let mut lens = [0u8; 16];
        lens[..8].copy_from_slice(&bits(aad.len()));
        lens[8..].copy_from_slice(&bits(ct.len()));
        let s = ghash(&[aad, ct, &lens]);
        let e = enc(j0);
        std::array::from_fn(|i| s[i] ^ e[i])
    };
    if encrypt {
        let mut out = gctr(data);
        let tag = tag_of(&out);
        out.extend_from_slice(&tag[..t]);
        Ok(out)
    } else {
        if data.len() < t {
            return Err(crypto_err(""));
        }
        let (ct, tag) = data.split_at(data.len() - t);
        let want = tag_of(ct);
        let diff = want[..t]
            .iter()
            .zip(tag)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        if diff != 0 {
            return Err(crypto_err(""));
        }
        Ok(gctr(ct))
    }
}

/// AES-CBC encrypt/decrypt with PKCS#7 padding (the only padding WebCrypto
/// AES-CBC uses) and a 16-byte IV.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_aes_cbc(
    encrypt: bool,
    #[buffer] key: &[u8],
    #[buffer] iv: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use cbc::cipher::block_padding::Pkcs7;
    use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
    use cbc::{Decryptor, Encryptor};

    if iv.len() != 16 {
        return Err(crypto_err("AES-CBC requires a 16-byte IV"));
    }
    macro_rules! run {
        ($cipher:ty) => {{
            if encrypt {
                Encryptor::<$cipher>::new_from_slices(key, iv)
                    .map_err(crypto_err)?
                    .encrypt_padded_vec::<Pkcs7>(data)
            } else {
                Decryptor::<$cipher>::new_from_slices(key, iv)
                    .map_err(crypto_err)?
                    .decrypt_padded_vec::<Pkcs7>(data)
                    .map_err(|_| crypto_err(""))?
            }
        }};
    }
    Ok(match key.len() {
        16 => run!(aes::Aes128),
        24 => run!(aes::Aes192),
        32 => run!(aes::Aes256),
        _ => return Err(crypto_err("AES-CBC key must be 128, 192, or 256 bits")),
    })
}

/// AES-CTR. Encrypt and decrypt are the same keystream XOR. Only the low
/// `counter_length` bits (1..=128, validated on the JS side) of the 16-byte
/// block count, wrapping inside that width; an input needing more blocks
/// than the counter has values is AES_CTR_OVERFLOW (Chrome 157's DataError).
/// By hand: ctr's flavors stop at 32/64/128-bit counters.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_aes_ctr(
    #[buffer] key: &[u8],
    #[buffer] counter: &[u8],
    counter_length: u32,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let block: [u8; 16] = counter.try_into().map_err(|_| crypto_err(""))?;
    if !(1..=128).contains(&counter_length) {
        return Err(crypto_err(""));
    }
    let blocks = data.len().div_ceil(16) as u128;
    if counter_length < 128 && blocks > (1u128 << counter_length) {
        return Err(crypto_err(AES_CTR_OVERFLOW));
    }
    let enc = aes_block(key)?;
    let base = u128::from_be_bytes(block);
    let mask = if counter_length == 128 {
        u128::MAX
    } else {
        (1u128 << counter_length) - 1
    };
    let mut out = Vec::with_capacity(data.len());
    for (i, chunk) in data.chunks(16).enumerate() {
        let low = (base & mask).wrapping_add(i as u128) & mask;
        let ks = enc(((base & !mask) | low).to_be_bytes());
        out.extend(chunk.iter().zip(ks.iter()).map(|(a, b)| a ^ b));
    }
    Ok(out)
}

/// AES-KW key wrap / unwrap (RFC 3394) — Chrome exposes this face only via
/// wrapKey/unwrapKey, not encrypt/decrypt (the JS shim enforces that split).
/// Wrap input (and the plaintext behind an unwrap input) must be a nonempty
/// multiple of 8 bytes of at least 16 (two 64-bit blocks); the wrapped form
/// is 8 bytes longer. Chrome 155 dropped 192-bit AES, so only 128/256-bit
/// KEKs are accepted here (the JS layer rejects 192 before reaching the op).
/// Failures surface as OperationError with an empty message, matching
/// Chrome's corrupt-unwrap face.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_aes_kw(
    encrypt: bool,
    #[buffer] key: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};

    const IV: [u8; 8] = [0xA6; 8];
    if data.is_empty() || !data.len().is_multiple_of(8) || data.len() < 16 {
        return Err(crypto_err(""));
    }
    macro_rules! kw {
        ($ty:ty) => {{
            let cipher = <$ty>::new_from_slice(key).map_err(crypto_err)?;
            if encrypt {
                let n = data.len() / 8;
                let mut a = IV;
                let mut r: Vec<[u8; 8]> = data
                    .chunks_exact(8)
                    .map(|c| c.try_into().unwrap())
                    .collect();
                for j in 0..6u64 {
                    for i in 0..n {
                        let mut block = [0u8; 16];
                        block[..8].copy_from_slice(&a);
                        block[8..].copy_from_slice(&r[i]);
                        let mut b = aes::Block::from(block);
                        cipher.encrypt_block(&mut b);
                        let t = n as u64 * j + i as u64 + 1;
                        let a_int = u64::from_be_bytes(b[..8].try_into().unwrap()) ^ t;
                        a = a_int.to_be_bytes();
                        r[i] = b[8..].try_into().unwrap();
                    }
                }
                let mut out = Vec::with_capacity(data.len() + 8);
                out.extend_from_slice(&a);
                for part in &r {
                    out.extend_from_slice(part);
                }
                out
            } else {
                if data.len() < 24 {
                    return Err(crypto_err(""));
                }
                let n = (data.len() - 8) / 8;
                let mut a: [u8; 8] = data[..8].try_into().unwrap();
                let mut r: Vec<[u8; 8]> = data[8..]
                    .chunks_exact(8)
                    .map(|c| c.try_into().unwrap())
                    .collect();
                for j in (0..6u64).rev() {
                    for i in (0..n).rev() {
                        let t = n as u64 * j + i as u64 + 1;
                        let a_int = u64::from_be_bytes(a) ^ t;
                        let mut block = [0u8; 16];
                        block[..8].copy_from_slice(&a_int.to_be_bytes());
                        block[8..].copy_from_slice(&r[i]);
                        let mut b = aes::Block::from(block);
                        cipher.decrypt_block(&mut b);
                        a = b[..8].try_into().unwrap();
                        r[i] = b[8..].try_into().unwrap();
                    }
                }
                if a != IV {
                    return Err(crypto_err(""));
                }
                let mut out = Vec::with_capacity(data.len() - 8);
                for part in &r {
                    out.extend_from_slice(part);
                }
                out
            }
        }};
    }
    Ok(match key.len() {
        16 => kw!(aes::Aes128),
        32 => kw!(aes::Aes256),
        _ => return Err(crypto_err("AES-KW key must be 128 or 256 bits")),
    })
}

/// Generous upper bounds on PBKDF2 parameters. WebCrypto imposes no limit, but
/// page JS drives this op on the single-threaded runtime: an unbounded
/// iteration count pins the V8 isolate (blocking every other CDP command on the
/// connection) and a huge output length forces an unbounded `vec![0u8; length]`
/// allocation. Both caps sit far above any legitimate use — OWASP recommends
/// ~600k iterations and derived keys are tens of bytes.
pub(crate) const PBKDF2_MAX_ITERATIONS: u32 = 10_000_000;
pub(crate) const PBKDF2_MAX_OUTPUT_BYTES: u32 = 1024 * 1024;

/// PBKDF2 key derivation with DoS guards. Split out from the op so the bounds
/// are unit-testable without the `#[op2]` wrapper.
pub(crate) fn pbkdf2_derive(
    hash: &str,
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    length: u32,
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    // Chrome 157 (probed): checked before the length, so length 0 with zero
    // iterations still fails this way.
    if iterations == 0 {
        return Err(crypto_err("PBKDF2 requires iterations > 0"));
    }
    if iterations > PBKDF2_MAX_ITERATIONS {
        return Err(crypto_err(format!(
            "PBKDF2 iteration count {iterations} exceeds the supported maximum of {PBKDF2_MAX_ITERATIONS}"
        )));
    }
    if length > PBKDF2_MAX_OUTPUT_BYTES {
        return Err(crypto_err(format!(
            "PBKDF2 output length {length} bytes exceeds the supported maximum of {PBKDF2_MAX_OUTPUT_BYTES}"
        )));
    }
    use pbkdf2::pbkdf2_hmac;
    let mut dk = vec![0u8; length as usize];
    match hash {
        "SHA-1" => pbkdf2_hmac::<sha1::Sha1>(password, salt, iterations, &mut dk),
        "SHA-256" => pbkdf2_hmac::<sha2::Sha256>(password, salt, iterations, &mut dk),
        "SHA-384" => pbkdf2_hmac::<sha2::Sha384>(password, salt, iterations, &mut dk),
        "SHA-512" => pbkdf2_hmac::<sha2::Sha512>(password, salt, iterations, &mut dk),
        _ => return Err(crypto_err("unsupported PBKDF2 hash")),
    }
    Ok(dk)
}

/// HKDF key derivation. `length` is the output length in bytes. An empty salt
/// behaves as RFC 5869 specifies (HMAC zero-pads it to the block size, which is
/// what browsers do).
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_hkdf(
    #[string] hash: &str,
    #[buffer] ikm: &[u8],
    #[buffer] salt: &[u8],
    #[buffer] info: &[u8],
    length: u32,
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use hkdf::Hkdf;
    // Cap before allocating (obscura #910 family): RFC 5869 bounds output at
    // 255*HashLen anyway, but the old shape allocated `length` bytes BEFORE
    // expand() rejected an oversized request — a page-controlled length was
    // still a transient multi-hundred-MB spike even when it eventually
    // errored. Same treatment PBKDF2 gets via PBKDF2_MAX_OUTPUT_BYTES.
    if length > 65536 {
        return Err(crypto_err(format!(
            "HKDF output length {length} bytes exceeds the supported maximum of 65536"
        )));
    }
    let mut okm = vec![0u8; length as usize];
    macro_rules! run {
        ($d:ty) => {
            Hkdf::<$d>::new(Some(salt), ikm)
                .expand(info, &mut okm)
                .map_err(|_| crypto_err("HKDF: requested key length is too long"))?
        };
    }
    match hash {
        "SHA-1" => run!(sha1::Sha1),
        "SHA-256" => run!(sha2::Sha256),
        "SHA-384" => run!(sha2::Sha384),
        "SHA-512" => run!(sha2::Sha512),
        _ => return Err(crypto_err("unsupported HKDF hash")),
    }
    Ok(okm)
}
/// Fill `len` bytes from the OS CSPRNG. Backs `crypto.getRandomValues`,
/// `crypto.randomUUID`, and `generateKey`, replacing the old Math.random shim
/// (which was neither uniform across typed-array widths nor cryptographically
/// random, and was a fingerprinting tell).
#[diting_op]
#[buffer]
pub(crate) fn op_random_bytes(len: u32) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    // Output-length cap (obscura #910 family): the HMAC generateKey path
    // forwards a page-chosen key length here unclamped, and `vec![0u8; len]`
    // with a multi-hundred-MB len is an instant OOM abort rather than a
    // catchable error. 65536 matches the bound getRandomValues already
    // enforces per spec; no real key or UUID comes near it.
    if len > 65536 {
        return Err(crypto_err("requested random length exceeds 65536 bytes"));
    }
    let mut buf = vec![0u8; len as usize];
    getrandom::fill(&mut buf).map_err(|e| crypto_err(format!("getrandom failed: {e}")))?;
    Ok(buf)
}
