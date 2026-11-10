// ---------------------------------------------------------------------------
// WebCrypto RSA-OAEP (crypto.subtle). Key material travels between the JS
// shim and these ops as a flat payload of ten u16-BE length-prefixed
// sections in this order: [pkcs8 DER (private), SPKI DER (public), n, e, d,
// p, q, dp, dq, qi]. Private-only sections carry length 0 on public keys
// and vice versa (spki/n/e always present). The JS shim owns CryptoKey
// objects; these ops are stateless byte-in/byte-out like the ECDSA ones.
//
// Chrome 155 supports exactly RSA-OAEP over SHA-1/256/384/512 on this face
// (RSA-PSS/RSASSA sign is not absorbed yet) and restricts generateKey to
// publicExponent 3 or 65537 — that policy lives in the JS shim; the ops
// accept what they are given.
//
// rsa 0.10-rc swapped num-bigint for crypto-bigint's BoxedUint; integers
// cross the op boundary as minimal big-endian bytes both ways
// (from_be_slice_vartime / to_be_bytes_trimmed_vartime).
// ---------------------------------------------------------------------------

use super::*;

/// Flat ten-section payload shared with the JS shim (`ecdsa_flat` sibling).
pub(crate) fn rsa_flat(parts: [&[u8]; 10]) -> Vec<u8> {
    let mut out = Vec::with_capacity(20 + parts.iter().map(|p| p.len()).sum::<usize>());
    for p in parts {
        out.extend_from_slice(&(p.len() as u16).to_be_bytes());
        out.extend_from_slice(p);
    }
    out
}

fn rsa_key_flat(sk: &rsa::RsaPrivateKey) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};
    use rsa::traits::{PrivateKeyParts, PublicKeyParts};
    let pkcs8 = sk.to_pkcs8_der().map_err(crypto_err)?.as_bytes().to_vec();
    let spki = sk
        .to_public_key()
        .to_public_key_der()
        .map_err(crypto_err)?
        .as_bytes()
        .to_vec();
    let primes = sk.primes();
    if primes.len() != 2 {
        return Err(crypto_err("only two-prime RSA keys are supported"));
    }
    // CRT params come from rc's own precompute (dp/dq as uints, qi as a
    // Montgomery form over p — retrieve() lifts it back to plain uint).
    let (dp, dq, qi) = match (sk.dp(), sk.dq(), sk.qinv()) {
        (Some(dp), Some(dq), Some(qi)) => (
            dp.to_be_bytes_trimmed_vartime(),
            dq.to_be_bytes_trimmed_vartime(),
            qi.retrieve().to_be_bytes_trimmed_vartime(),
        ),
        _ => return Err(crypto_err("only two-prime RSA keys are supported")),
    };
    let n = sk.n_bytes();
    let e = sk.e_bytes();
    let d = sk.d().to_be_bytes_trimmed_vartime();
    let pb = primes[0].to_be_bytes_trimmed_vartime();
    let qb = primes[1].to_be_bytes_trimmed_vartime();
    Ok(rsa_flat([
        &pkcs8[..],
        &spki[..],
        &n[..],
        &e[..],
        &d[..],
        &pb[..],
        &qb[..],
        &dp[..],
        &dq[..],
        &qi[..],
    ]))
}

fn public_flat(pk: &rsa::RsaPublicKey) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use rsa::pkcs8::EncodePublicKey;
    use rsa::traits::PublicKeyParts;
    let spki = pk
        .to_public_key_der()
        .map_err(crypto_err)?
        .as_bytes()
        .to_vec();
    let n = pk.n_bytes();
    let e = pk.e_bytes();
    let empty: &[u8] = &[];
    Ok(rsa_flat([
        empty,
        &spki[..],
        &n[..],
        &e[..],
        empty,
        empty,
        empty,
        empty,
        empty,
        empty,
    ]))
}

fn private_from_components(
    n: rsa::BoxedUint,
    e: rsa::BoxedUint,
    d: rsa::BoxedUint,
    p: rsa::BoxedUint,
    q: rsa::BoxedUint,
) -> Result<rsa::RsaPrivateKey, deno_error::JsErrorBox> {
    rsa::RsaPrivateKey::from_components(n, e, d, vec![p, q])
        .map_err(|_| crypto_err("invalid RSA private key components"))
}

/// rand_core 0.10's `CryptoRng` bound (what rsa's keygen/encrypt take) is
/// infallible-only; the OS entropy source is a fallible `TryRng`
/// (getrandom 0.4 `SysRng`). `UnwrapErr` bridges: it panics on OS-entropy
/// failure, exactly like the old rand_core 0.6 `OsRng`.
fn os_rng() -> rsa::rand_core::UnwrapErr<getrandom::SysRng> {
    rsa::rand_core::UnwrapErr(getrandom::SysRng)
}

/// Generate an RSA keypair. `bits` is the modulus length; `e` the public
/// exponent as big-endian bytes. Returns the full ten-section payload.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_generate(
    bits: u32,
    #[buffer] e: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let exponent = rsa::BoxedUint::from_be_slice_vartime(e);
    let sk = rsa::RsaPrivateKey::new_with_exp(&mut os_rng(), bits as usize, exponent)
        .map_err(crypto_err)?;
    rsa_key_flat(&sk)
}

/// Import a public key from (n, e) big-endian integers (JWK path). Returns
/// the payload with only the public sections set.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_import_public(
    #[buffer] n: &[u8],
    #[buffer] e: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let pk = rsa::RsaPublicKey::new(
        rsa::BoxedUint::from_be_slice_vartime(n),
        rsa::BoxedUint::from_be_slice_vartime(e),
    )
    .map_err(crypto_err)?;
    public_flat(&pk)
}

/// Import a public key from SPKI DER. Returns the public-only payload.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_import_spki(
    #[buffer] spki: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use rsa::pkcs8::DecodePublicKey;
    let pk = rsa::RsaPublicKey::from_public_key_der(spki).map_err(crypto_err)?;
    public_flat(&pk)
}

/// Import a private key from (n, e, d, p, q) big-endian integers (JWK path —
/// dp/dq/qi are recomputed per RFC 8017 so exported JWKs round-trip). Returns
/// the full payload.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_import_private(
    #[buffer] n: &[u8],
    #[buffer] e: &[u8],
    #[buffer] d: &[u8],
    #[buffer] p: &[u8],
    #[buffer] q: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let sk = private_from_components(
        rsa::BoxedUint::from_be_slice_vartime(n),
        rsa::BoxedUint::from_be_slice_vartime(e),
        rsa::BoxedUint::from_be_slice_vartime(d),
        rsa::BoxedUint::from_be_slice_vartime(p),
        rsa::BoxedUint::from_be_slice_vartime(q),
    )?;
    rsa_key_flat(&sk)
}

/// Import a private key from PKCS#8 DER. Returns the full payload.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_import_pkcs8(
    #[buffer] pkcs8: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use rsa::pkcs8::DecodePrivateKey;
    let sk = rsa::RsaPrivateKey::from_pkcs8_der(pkcs8).map_err(crypto_err)?;
    rsa_key_flat(&sk)
}

/// RSA-OAEP encrypt/decrypt. `hash` selects the MGF1 hash (and the label
/// hash — WebCrypto uses the same hash for both); `label` is the optional
/// OAEP label. The encrypt arm takes only (n, e); the decrypt arm takes the
/// full private components. Any crypto failure (corrupt ciphertext, label
/// mismatch, wrong length) surfaces as the WebCrypto OperationError with an
/// empty message, matching Chrome.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_oaep(
    encrypt: bool,
    #[string] hash: &str,
    #[buffer] label: &[u8],
    #[buffer] n: &[u8],
    #[buffer] e: &[u8],
    #[buffer] d: &[u8],
    #[buffer] p: &[u8],
    #[buffer] q: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let pk = rsa::RsaPublicKey::new(
        rsa::BoxedUint::from_be_slice_vartime(n),
        rsa::BoxedUint::from_be_slice_vartime(e),
    )
    .map_err(crypto_err)?;
    // rc takes the label as raw bytes — arbitrary WebCrypto labels now pass
    // through exactly (the 0.9 lossy-utf8 bend is gone).
    macro_rules! run {
        ($digest:ty) => {{
            let scheme = if label.is_empty() {
                rsa::Oaep::<$digest>::new()
            } else {
                rsa::Oaep::<$digest>::new_with_label(label)
            };
            if encrypt {
                pk.encrypt(&mut os_rng(), scheme, data)
                    .map_err(|_| crypto_err(""))?
            } else {
                let sk = private_from_components(
                    rsa::BoxedUint::from_be_slice_vartime(n),
                    rsa::BoxedUint::from_be_slice_vartime(e),
                    rsa::BoxedUint::from_be_slice_vartime(d),
                    rsa::BoxedUint::from_be_slice_vartime(p),
                    rsa::BoxedUint::from_be_slice_vartime(q),
                )?;
                sk.decrypt(scheme, data).map_err(|_| crypto_err(""))?
            }
        }};
    }
    Ok(match hash {
        "SHA-1" => run!(sha1::Sha1),
        "SHA-256" => run!(rsa::sha2::Sha256),
        "SHA-384" => run!(rsa::sha2::Sha384),
        "SHA-512" => run!(rsa::sha2::Sha512),
        _ => return Err(crypto_err("unsupported RSA-OAEP hash")),
    })
}

// RSASSA-PKCS1-v1_5 (#273) — the JWT RS256 family. PKCS#1 v1.5 signatures
// are deterministic, so the parity pin in runtime/tests.rs bakes a
// Chrome-produced signature over a fixed jwk and asserts byte equality.
// The padding scheme types ride rsa's own digest line (`rsa::sha2`,
// `sha1` — AssociatedOid lives there), while the prehash is computed
// with the same crates: identical output bytes, and the scheme only
// reads D's constants.
fn rsassa_prehash(hash: &str, data: &[u8]) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use sha2::Digest as _;
    Ok(match hash {
        "SHA-1" => sha1::Sha1::digest(data).to_vec(),
        "SHA-256" => sha2::Sha256::digest(data).to_vec(),
        "SHA-384" => sha2::Sha384::digest(data).to_vec(),
        "SHA-512" => sha2::Sha512::digest(data).to_vec(),
        _ => return Err(crypto_err("unsupported RSASSA hash")),
    })
}

/// RSASSA-PKCS1-v1_5 sign. Takes the private components; returns the
/// modulus-length signature. Crypto failures surface as an empty-message
/// OperationError, matching Chrome.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_rsa_pkcs1v15_sign(
    #[string] hash: &str,
    #[buffer] n: &[u8],
    #[buffer] e: &[u8],
    #[buffer] d: &[u8],
    #[buffer] p: &[u8],
    #[buffer] q: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let prehash = rsassa_prehash(hash, data)?;
    let sk = private_from_components(
        rsa::BoxedUint::from_be_slice_vartime(n),
        rsa::BoxedUint::from_be_slice_vartime(e),
        rsa::BoxedUint::from_be_slice_vartime(d),
        rsa::BoxedUint::from_be_slice_vartime(p),
        rsa::BoxedUint::from_be_slice_vartime(q),
    )?;
    macro_rules! run {
        ($digest:ty) => {{
            sk.sign(rsa::Pkcs1v15Sign::new::<$digest>(), &prehash)
                .map_err(|_| crypto_err(""))?
        }};
    }
    Ok(match hash {
        "SHA-1" => run!(sha1::Sha1),
        "SHA-256" => run!(rsa::sha2::Sha256),
        "SHA-384" => run!(rsa::sha2::Sha384),
        "SHA-512" => run!(rsa::sha2::Sha512),
        _ => return Err(crypto_err("unsupported RSASSA hash")),
    })
}

/// RSASSA-PKCS1-v1_5 verify. Takes only (n, e). Any failure — corrupt
/// signature, wrong key, wrong length — is `false`, never an error: a bad
/// verify is a normal outcome, not an exception (Chrome pinned).
#[diting_op]
pub(crate) fn op_subtle_rsa_pkcs1v15_verify(
    #[string] hash: &str,
    #[buffer] n: &[u8],
    #[buffer] e: &[u8],
    #[buffer] signature: &[u8],
    #[buffer] data: &[u8],
) -> Result<bool, deno_error::JsErrorBox> {
    let prehash = rsassa_prehash(hash, data)?;
    let pk = rsa::RsaPublicKey::new(
        rsa::BoxedUint::from_be_slice_vartime(n),
        rsa::BoxedUint::from_be_slice_vartime(e),
    )
    .map_err(crypto_err)?;
    macro_rules! run {
        ($digest:ty) => {{
            pk.verify(rsa::Pkcs1v15Sign::new::<$digest>(), &prehash, signature)
                .is_ok()
        }};
    }
    Ok(match hash {
        "SHA-1" => run!(sha1::Sha1),
        "SHA-256" => run!(rsa::sha2::Sha256),
        "SHA-384" => run!(rsa::sha2::Sha384),
        "SHA-512" => run!(rsa::sha2::Sha512),
        _ => return Err(crypto_err("unsupported RSASSA hash")),
    })
}
