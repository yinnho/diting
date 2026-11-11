// ---------------------------------------------------------------------------
// WebCrypto ECDSA (P-256 / P-384) and ECDH. The key material travels between
// the JS shim and these ops as a flat payload of four u16-BE length-prefixed
// sections in this order: [pkcs8 DER (private), SEC1 uncompressed point
// (public), scalar bytes (private), SPKI DER (public)]. Absent sections carry
// length 0. Signatures use the WebCrypto P1363 raw form (r||s, no DER
// wrapper) — that is what browsers emit and accept.
// ---------------------------------------------------------------------------

use super::*;

pub(crate) fn ecdsa_flat(parts: [&[u8]; 4]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + parts.iter().map(|p| p.len()).sum::<usize>());
    for p in parts {
        out.extend_from_slice(&(p.len() as u16).to_be_bytes());
        out.extend_from_slice(p);
    }
    out
}

pub(crate) fn ecdsa_digest(hash: &str, data: &[u8]) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    use sha1::Digest as _;
    Ok(match hash {
        "SHA-1" => sha1::Sha1::digest(data).to_vec(),
        "SHA-256" => sha2::Sha256::digest(data).to_vec(),
        "SHA-384" => sha2::Sha384::digest(data).to_vec(),
        "SHA-512" => sha2::Sha512::digest(data).to_vec(),
        _ => return Err(crypto_err("unsupported ECDSA hash")),
    })
}

// Expand the body once per supported curve; each arm binds `ec` to the curve
// crate so the body text is uniform.
macro_rules! ecdsa_curves {
    ($curve:expr, { $($body:tt)* }) => {
        match $curve {
            "P-256" => {
                use p256 as ec;
                $($body)*
            }
            "P-384" => {
                use p384 as ec;
                $($body)*
            }
            _ => return Err(crypto_err(format!("unsupported named curve {}", $curve))),
        }
    };
}

macro_rules! ecdsa_key_flat {
    ($sk:expr) => {{
        let sk = $sk;
        let pkcs8 = sk.to_pkcs8_der().map_err(crypto_err)?.as_bytes().to_vec();
        let vk = sk.verifying_key();
        let sec1 = vk.to_sec1_point(false).as_bytes().to_vec();
        let spki = vk
            .to_public_key_der()
            .map_err(crypto_err)?
            .as_bytes()
            .to_vec();
        let scalar = sk.to_bytes().to_vec();
        ecdsa_flat([&pkcs8[..], &sec1[..], &scalar[..], &spki[..]])
    }};
}

/// Generate an ECDSA keypair. Returns the flat key payload (all four sections
/// populated — the public and private CryptoKey views in the JS shim share it).
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_ecdsa_generate(
    #[string] curve: &str,
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ecdsa_curves!(curve, {
        use ec::ecdsa::SigningKey;
        use ec::elliptic_curve::pkcs8::{EncodePrivateKey, EncodePublicKey};
        use ec::elliptic_curve::Generate;
        let sk = SigningKey::generate();
        Ok(ecdsa_key_flat!(sk))
    })
}

// Import refusals, one sentinel per Chrome 157 face (probed; the JS side maps
// them): EC_INVALID = empty DataError (malformed DER, wrong/absent curve
// params, a point off the curve, a bad pkcs8 scalar, an embedded public key
// that is not the private key's); EC_CURVE_MISMATCH = a valid key of another
// known curve; EC_SCALAR_INVALID = empty OperationError (a jwk "d" of zero or
// >= n); EC_PAIR_MISMATCH = a jwk whose d does not produce its x/y.
pub(crate) const EC_INVALID: &str = "EC key data invalid";
pub(crate) const EC_CURVE_MISMATCH: &str = "EC curve mismatch";
pub(crate) const EC_SCALAR_INVALID: &str = "EC scalar invalid";
pub(crate) const EC_PAIR_MISMATCH: &str = "EC pair mismatch";

fn ec_invalid<E>(_: E) -> deno_error::JsErrorBox {
    crypto_err(EC_INVALID)
}

const OID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";

fn curve_of_oid(oid: &sec1::der::asn1::ObjectIdentifier) -> Option<&'static str> {
    match oid.to_string().as_str() {
        "1.2.840.10045.3.1.7" => Some("P-256"),
        "1.3.132.0.34" => Some("P-384"),
        "1.3.132.0.35" => Some("P-521"),
        _ => None,
    }
}

/// The uncompressed SEC1 form of `point` (compressed or uncompressed) on
/// `curve`, or EC_INVALID — identity, off-curve, x >= p, wrong length.
fn ec_point(curve: &str, point: &[u8]) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ecdsa_curves!(curve, {
        use ec::elliptic_curve::sec1::ToSec1Point;
        let pk = ec::PublicKey::from_sec1_bytes(point).map_err(ec_invalid)?;
        Ok(pk.to_sec1_point(false).as_bytes().to_vec())
    })
}

/// A key parsed on its own curve `found` but requested as `wanted`: Chrome
/// names the curve clash only for a key that is valid where it says it is.
fn ec_other_curve(found: &str, check: impl FnOnce() -> bool) -> deno_error::JsErrorBox {
    if found == "P-521" || check() {
        crypto_err(EC_CURVE_MISMATCH)
    } else {
        crypto_err(EC_INVALID)
    }
}

/// Import a private key from either PKCS#8 DER (`is_scalar` false) or a jwk
/// "d" (`is_scalar` true, already length-checked). For a jwk, `expect_sec1` is
/// the uncompressed point its x/y spell (already validated on the curve); the
/// key's own public point must equal it. Returns the full flat payload.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_ecdsa_import_private(
    #[string] curve: &str,
    is_scalar: bool,
    #[buffer] material: &[u8],
    #[buffer] expect_sec1: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    if is_scalar {
        return ecdsa_curves!(curve, {
            use ec::ecdsa::SigningKey;
            use ec::elliptic_curve::pkcs8::{EncodePrivateKey, EncodePublicKey};
            let sk = SigningKey::from_slice(material).map_err(|_| crypto_err(EC_SCALAR_INVALID))?;
            if !expect_sec1.is_empty()
                && sk.verifying_key().to_sec1_point(false).as_bytes() != expect_sec1
            {
                return Err(crypto_err(EC_PAIR_MISMATCH));
            }
            Ok(ecdsa_key_flat!(sk))
        });
    }
    // PKCS#8 by hand (the curve crates' decode either refuses keys Chrome
    // takes — an unpadded or zero-padded scalar — or skips checks Chrome
    // makes): rsaEncryption's EC twin, then RFC 5915 ECPrivateKey.
    use sec1::der::Decode as _;
    let info = p256::pkcs8::PrivateKeyInfoRef::from_der(material).map_err(ec_invalid)?;
    if info.algorithm.oid.to_string() != OID_EC_PUBLIC_KEY {
        return Err(crypto_err(EC_INVALID));
    }
    let outer: sec1::der::asn1::ObjectIdentifier = info
        .algorithm
        .parameters
        .ok_or_else(|| crypto_err(EC_INVALID))?
        .decode_as()
        .map_err(ec_invalid)?;
    let found = curve_of_oid(&outer).ok_or_else(|| crypto_err(EC_INVALID))?;
    let k = sec1::EcPrivateKey::try_from(info.private_key.as_bytes()).map_err(ec_invalid)?;
    if let Some(inner) = k.parameters.and_then(|p| p.named_curve()) {
        if inner != outer {
            return Err(crypto_err(EC_INVALID));
        }
    }
    let scalar: Vec<u8> = k
        .private_key
        .iter()
        .skip_while(|b| **b == 0)
        .copied()
        .collect();
    let embedded = k.public_key;
    let build = |want: &str| -> Result<Vec<u8>, deno_error::JsErrorBox> {
        ecdsa_curves!(want, {
            use ec::ecdsa::SigningKey;
            use ec::elliptic_curve::pkcs8::{EncodePrivateKey, EncodePublicKey};
            let field = ec::FieldBytes::default().len();
            if scalar.len() > field {
                return Err(crypto_err(EC_INVALID));
            }
            let mut padded = vec![0u8; field - scalar.len()];
            padded.extend_from_slice(&scalar);
            let sk = SigningKey::from_slice(&padded).map_err(ec_invalid)?;
            if let Some(pub_bytes) = embedded {
                let derived = sk.verifying_key().to_sec1_point(false).as_bytes().to_vec();
                if ec_point(want, pub_bytes)? != derived {
                    return Err(crypto_err(EC_INVALID));
                }
            }
            Ok(ecdsa_key_flat!(sk))
        })
    };
    if found != curve {
        return Err(ec_other_curve(found, || build(found).is_ok()));
    }
    build(curve)
}

/// Import a public ECDSA key from either SPKI DER or a raw uncompressed SEC1
/// point. Returns the flat payload with only the public sections set.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_ecdsa_import_public(
    #[string] curve: &str,
    #[buffer] material: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ecdsa_curves!(curve, {
        use ec::ecdsa::VerifyingKey;
        use ec::elliptic_curve::pkcs8::EncodePublicKey;
        // SPKI by hand (Chrome 157 faces, probed): id-ecPublicKey with a
        // named-curve OID it knows, an octet-aligned point valid on that
        // curve; a valid key of another curve is EC_CURVE_MISMATCH, anything
        // else EC_INVALID. Raw: any SEC1 point, compressed included.
        let point = if material.first() == Some(&0x30) {
            use sec1::der::Decode as _;
            let info =
                p256::pkcs8::SubjectPublicKeyInfoRef::from_der(material).map_err(ec_invalid)?;
            if info.algorithm.oid.to_string() != OID_EC_PUBLIC_KEY {
                return Err(crypto_err(EC_INVALID));
            }
            let oid: sec1::der::asn1::ObjectIdentifier = info
                .algorithm
                .parameters
                .ok_or_else(|| crypto_err(EC_INVALID))?
                .decode_as()
                .map_err(ec_invalid)?;
            let found = curve_of_oid(&oid).ok_or_else(|| crypto_err(EC_INVALID))?;
            let bits = info
                .subject_public_key
                .as_bytes()
                .ok_or_else(|| crypto_err(EC_INVALID))?;
            if found != curve {
                return Err(ec_other_curve(found, || ec_point(found, bits).is_ok()));
            }
            ec_point(curve, bits)?
        } else {
            ec_point(curve, material)?
        };
        let vk = VerifyingKey::from_sec1_bytes(&point).map_err(ec_invalid)?;
        let sec1 = vk.to_sec1_point(false).as_bytes().to_vec();
        let spki = vk
            .to_public_key_der()
            .map_err(crypto_err)?
            .as_bytes()
            .to_vec();
        Ok(ecdsa_flat([
            &[] as &[u8],
            &sec1[..],
            &[] as &[u8],
            &spki[..],
        ]))
    })
}

/// ECDSA sign. `pkcs8` is the private key's PKCS#8 DER; output is the P1363
/// raw signature (64 bytes for P-256, 96 for P-384).
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_ecdsa_sign(
    #[string] curve: &str,
    #[string] hash: &str,
    #[buffer] pkcs8: &[u8],
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    let prehash = ecdsa_digest(hash, data)?;
    ecdsa_curves!(curve, {
        use ::ecdsa::signature::hazmat::PrehashSigner;
        use ec::ecdsa::{Signature, SigningKey};
        use ec::elliptic_curve::pkcs8::DecodePrivateKey;
        let sk = SigningKey::from_pkcs8_der(pkcs8).map_err(crypto_err)?;
        let sig: Signature = sk.sign_prehash(&prehash).map_err(crypto_err)?;
        Ok(sig.to_bytes().to_vec())
    })
}

/// ECDSA verify. `sec1` is the public key's uncompressed SEC1 point,
/// `signature` a P1363 raw signature. Malformed keys or signatures return
/// `false` (the WebCrypto behavior), not an error.
#[diting_op]
pub(crate) fn op_subtle_ecdsa_verify(
    #[string] curve: &str,
    #[string] hash: &str,
    #[buffer] sec1: &[u8],
    #[buffer] signature: &[u8],
    #[buffer] data: &[u8],
) -> Result<bool, deno_error::JsErrorBox> {
    let prehash = ecdsa_digest(hash, data)?;
    ecdsa_curves!(curve, {
        use ::ecdsa::signature::hazmat::PrehashVerifier;
        use ec::ecdsa::{Signature, VerifyingKey};
        let vk = match VerifyingKey::from_sec1_bytes(sec1) {
            Ok(vk) => vk,
            Err(_) => return Ok(false),
        };
        let sig = match Signature::from_slice(signature) {
            Ok(s) => s,
            Err(_) => return Ok(false),
        };
        Ok(vk.verify_prehash(&prehash, &sig).is_ok())
    })
}

/// ECDH shared secret (raw x-coordinate, 32/48 bytes). `scalar` is the
/// private key's raw big-endian scalar, `peer_sec1` the peer public key's
/// uncompressed SEC1 point — the two halves the JS shim already carries in
/// every EC key's material.
#[diting_op]
#[buffer]
pub(crate) fn op_subtle_ecdh_derive_bits(
    #[string] curve: &str,
    #[buffer] scalar: &[u8],
    #[buffer] peer_sec1: &[u8],
) -> Result<Vec<u8>, deno_error::JsErrorBox> {
    ecdsa_curves!(curve, {
        use ec::elliptic_curve::{ecdh, NonZeroScalar};
        let d = NonZeroScalar::from(&ec::SecretKey::from_slice(scalar).map_err(crypto_err)?);
        let peer = ec::PublicKey::from_sec1_bytes(peer_sec1).map_err(crypto_err)?;
        let shared = ecdh::diffie_hellman(d, peer.as_affine());
        Ok(shared.raw_secret_bytes().as_slice().to_vec())
    })
}
