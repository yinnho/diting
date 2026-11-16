//! ML-KEM-768 and ML-KEM-1024 for `crypto.subtle`, via the BoringSSL already
//! linked by the TLS stack. This BoringSSL has no ML-KEM-512 symbols; that
//! name is rejected in the JS shim instead of being invented here.
//!
//! Private keys stay inside BoringSSL's opaque structs for the duration of
//! one call. What crosses back to JS is the 64-byte seed and the encoded
//! public key.

use diting_ops::diting_op;

use super::crypto_secret::crypto_err;

const SEED: usize = 64;
const SHARED: usize = 32;

fn ensure_init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(btls_sys::init);
}

fn wipe<T>(key: &mut T) {
    // BoringSSL leaves the expanded private key in the struct. The bytes are
    // a valid all-zero pattern (the struct is an opaque byte buffer).
    unsafe { std::ptr::write_bytes(key as *mut T, 0, 1) };
}

fn bad() -> deno_error::JsErrorBox {
    crypto_err("ML-KEM operation failed")
}

macro_rules! kem_generate {
    ($gen:path, $sk:ty, $pub_len:expr) => {{
        let mut encoded = vec![0u8; $pub_len];
        let mut seed = [0u8; SEED];
        let mut sk: Box<$sk> = unsafe { Box::new_zeroed().assume_init() };
        unsafe { $gen(encoded.as_mut_ptr(), seed.as_mut_ptr(), &mut *sk) };
        wipe(&mut *sk);
        let mut out = Vec::with_capacity(SEED + $pub_len);
        out.extend_from_slice(&seed);
        seed.fill(0);
        out.extend_from_slice(&encoded);
        out
    }};
}

macro_rules! kem_public {
    ($from_seed:path, $pub_from:path, $marshal:path, $sk:ty, $pk:ty, $pub_len:expr, $seed:expr) => {{
        if $seed.len() != SEED {
            return Err(bad());
        }
        let mut sk: Box<$sk> = unsafe { Box::new_zeroed().assume_init() };
        let ok = unsafe { $from_seed(&mut *sk, $seed.as_ptr(), $seed.len()) };
        if ok != 1 {
            wipe(&mut *sk);
            return Err(bad());
        }
        let mut pk: Box<$pk> = unsafe { Box::new_zeroed().assume_init() };
        unsafe { $pub_from(&mut *pk, &*sk) };
        wipe(&mut *sk);
        let mut buf = vec![0u8; $pub_len];
        let mut cbb = unsafe { std::mem::zeroed::<btls_sys::CBB>() };
        let init = unsafe { btls_sys::CBB_init_fixed(&mut cbb, buf.as_mut_ptr(), buf.len()) };
        let wrote = if init == 1 {
            unsafe { $marshal(&mut cbb, &*pk) }
        } else {
            0
        };
        let n = unsafe { btls_sys::CBB_len(&cbb) };
        unsafe { btls_sys::CBB_cleanup(&mut cbb) };
        if wrote != 1 || n != $pub_len {
            return Err(bad());
        }
        buf
    }};
}

macro_rules! kem_encap {
    ($parse:path, $encap:path, $pk:ty, $pub_len:expr, $ct_len:expr, $public_key:expr) => {{
        if $public_key.len() != $pub_len {
            return Err(bad());
        }
        let mut pk: Box<$pk> = unsafe { Box::new_zeroed().assume_init() };
        let mut cbs = btls_sys::CBS {
            data: $public_key.as_ptr(),
            len: $public_key.len(),
        };
        let ok = unsafe { $parse(&mut *pk, &mut cbs) };
        if ok != 1 {
            return Err(bad());
        }
        let mut ct = vec![0u8; $ct_len];
        let mut shared = [0u8; SHARED];
        unsafe { $encap(ct.as_mut_ptr(), shared.as_mut_ptr(), &*pk) };
        ct.extend_from_slice(&shared);
        shared.fill(0);
        ct
    }};
}

macro_rules! kem_decap {
    ($from_seed:path, $decap:path, $sk:ty, $ct_len:expr, $seed:expr, $ciphertext:expr) => {{
        if $seed.len() != SEED || $ciphertext.len() != $ct_len {
            return Err(bad());
        }
        let mut sk: Box<$sk> = unsafe { Box::new_zeroed().assume_init() };
        let ok = unsafe { $from_seed(&mut *sk, $seed.as_ptr(), $seed.len()) };
        if ok != 1 {
            wipe(&mut *sk);
            return Err(bad());
        }
        let mut shared = [0u8; SHARED];
        let ok = unsafe {
            $decap(
                shared.as_mut_ptr(),
                $ciphertext.as_ptr(),
                $ciphertext.len(),
                &*sk,
            )
        };
        wipe(&mut *sk);
        if ok != 1 {
            return Err(bad());
        }
        shared.to_vec()
    }};
}

pub(crate) fn mlkem_generate(name: &str) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ensure_init();
    Ok(match name {
        "ML-KEM-768" => kem_generate!(
            btls_sys::MLKEM768_generate_key,
            btls_sys::MLKEM768_private_key,
            1184
        ),
        "ML-KEM-1024" => kem_generate!(
            btls_sys::MLKEM1024_generate_key,
            btls_sys::MLKEM1024_private_key,
            1568
        ),
        _ => return Err(bad()),
    })
}

pub(crate) fn mlkem_public(name: &str, seed: &[u8]) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ensure_init();
    Ok(match name {
        "ML-KEM-768" => kem_public!(
            btls_sys::MLKEM768_private_key_from_seed,
            btls_sys::MLKEM768_public_from_private,
            btls_sys::MLKEM768_marshal_public_key,
            btls_sys::MLKEM768_private_key,
            btls_sys::MLKEM768_public_key,
            1184,
            seed
        ),
        "ML-KEM-1024" => kem_public!(
            btls_sys::MLKEM1024_private_key_from_seed,
            btls_sys::MLKEM1024_public_from_private,
            btls_sys::MLKEM1024_marshal_public_key,
            btls_sys::MLKEM1024_private_key,
            btls_sys::MLKEM1024_public_key,
            1568,
            seed
        ),
        _ => return Err(bad()),
    })
}

/// Ciphertext followed by the 32-byte shared secret.
pub(crate) fn mlkem_encap(
    name: &str,
    public_key: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ensure_init();
    Ok(match name {
        "ML-KEM-768" => kem_encap!(
            btls_sys::MLKEM768_parse_public_key,
            btls_sys::MLKEM768_encap,
            btls_sys::MLKEM768_public_key,
            1184,
            1088,
            public_key
        ),
        "ML-KEM-1024" => kem_encap!(
            btls_sys::MLKEM1024_parse_public_key,
            btls_sys::MLKEM1024_encap,
            btls_sys::MLKEM1024_public_key,
            1568,
            1568,
            public_key
        ),
        _ => return Err(bad()),
    })
}

pub(crate) fn mlkem_decap(
    name: &str,
    seed: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ensure_init();
    Ok(match name {
        "ML-KEM-768" => kem_decap!(
            btls_sys::MLKEM768_private_key_from_seed,
            btls_sys::MLKEM768_decap,
            btls_sys::MLKEM768_private_key,
            1088,
            seed,
            ciphertext
        ),
        "ML-KEM-1024" => kem_decap!(
            btls_sys::MLKEM1024_private_key_from_seed,
            btls_sys::MLKEM1024_decap,
            btls_sys::MLKEM1024_private_key,
            1568,
            seed,
            ciphertext
        ),
        _ => return Err(bad()),
    })
}

#[diting_op]
#[buffer]
pub(crate) fn op_subtle_mlkem_generate(
    #[string] name: &str,
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    mlkem_generate(name)
}

#[diting_op]
#[buffer]
pub(crate) fn op_subtle_mlkem_public(
    #[string] name: &str,
    #[buffer] seed: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    mlkem_public(name, seed)
}

#[diting_op]
#[buffer]
pub(crate) fn op_subtle_mlkem_encap(
    #[string] name: &str,
    #[buffer] public_key: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    mlkem_encap(name, public_key)
}

#[diting_op]
#[buffer]
pub(crate) fn op_subtle_mlkem_decap(
    #[string] name: &str,
    #[buffer] seed: &[u8],
    #[buffer] ciphertext: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    mlkem_decap(name, seed, ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(name: &str, pub_len: usize, ct_len: usize) {
        let flat = mlkem_generate(name).unwrap();
        assert_eq!(flat.len(), SEED + pub_len);
        let (seed, pk) = flat.split_at(SEED);
        let enc = mlkem_encap(name, pk).unwrap();
        assert_eq!(enc.len(), ct_len + SHARED);
        let ss = mlkem_decap(name, seed, &enc[..ct_len]).unwrap();
        assert_eq!(ss, &enc[ct_len..]);
        let mut flipped = enc[..ct_len].to_vec();
        flipped[0] ^= 0xff;
        let rejected = mlkem_decap(name, seed, &flipped).unwrap();
        assert_ne!(rejected, ss);
        assert!(mlkem_decap(name, seed, &[0u8; 4]).is_err());
        assert_eq!(mlkem_public(name, seed).unwrap(), pk);
    }

    #[test]
    fn mlkem_boringssl_roundtrip() {
        roundtrip("ML-KEM-768", 1184, 1088);
        roundtrip("ML-KEM-1024", 1568, 1568);
        assert!(mlkem_generate("ML-KEM-512").is_err());
    }
}
