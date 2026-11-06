use super::*;

macro_rules! tls_options {
    (@build $builder:expr) => {
        $builder.build().into()
    };

    (1) => {
        tls_options!(@build ChromeTlsConfig::builder())
    };
    (2) => {
        tls_options!(@build ChromeTlsConfig::builder().enable_ech_grease(true))
    };
    (3) => {
        tls_options!(@build ChromeTlsConfig::builder().permute_extensions(true))
    };
    (4) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true))
    };
    (5) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true)
            .pre_shared_key(true))
    };
    (6, $curves:expr) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true)
            .pre_shared_key(true)
            .curves($curves))
    };
    (7, $curves:expr) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true)
            .pre_shared_key(true)
            .curves($curves)
            .alps_use_new_codepoint(true))
    };
    // (8) = (7) + the empty trust_anchors extension, as sent by real Chrome 145.
    (8, $curves:expr) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true)
            .pre_shared_key(true)
            .curves($curves)
            .alps_use_new_codepoint(true)
            .empty_trust_anchors(true))
    };
    // (9) = (8) + GREASE/ML-DSA-led verify sigalgs, as sent by real Chrome 157.
    // Degrades to (8)'s sigalgs on BoringSSL snapshots that reject the raw
    // values (see `sigalgs_v157`) — the rest of the 157 face is identical.
    (9, $curves:expr) => {
        tls_options!(@build ChromeTlsConfig::builder()
            .permute_extensions(true)
            .enable_ech_grease(true)
            .pre_shared_key(true)
            .curves($curves)
            .alps_use_new_codepoint(true)
            .empty_trust_anchors(true)
            .sigalgs_mldsa_grease(true))
    };
}

pub const CURVES_1: &str = join!(":", "X25519", "P-256", "P-384");
pub const CURVES_2: &str = join!(":", "X25519Kyber768Draft00", "X25519", "P-256", "P-384");
pub const CURVES_3: &str = join!(":", "X25519MLKEM768", "X25519", "P-256", "P-384");

pub const CIPHER_LIST: &str = join!(
    ":",
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
    "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
    "TLS_RSA_WITH_AES_128_GCM_SHA256",
    "TLS_RSA_WITH_AES_256_GCM_SHA384",
    "TLS_RSA_WITH_AES_128_CBC_SHA",
    "TLS_RSA_WITH_AES_256_CBC_SHA"
);

pub const SIGALGS_LIST: &str = join!(
    ":",
    "ecdsa_secp256r1_sha256",
    "rsa_pss_rsae_sha256",
    "rsa_pkcs1_sha256",
    "ecdsa_secp384r1_sha384",
    "rsa_pss_rsae_sha384",
    "rsa_pkcs1_sha384",
    "rsa_pss_rsae_sha512",
    "rsa_pkcs1_sha512"
);

/// Chrome 157's `signature_algorithms` after the GREASE lead: the ML-DSA trio
/// (0x0904/0x0905/0x0906 — `mldsa44/65/87`, RFC-ietf-tls-mldsa; schemes this
/// BoringSSL snapshot cannot name, hence raw wire values), then `SIGALGS_LIST`'s
/// 8 classics. Measured on 157.0.8103.38 (tools/webplatform/chrome/wire/); the
/// trio arrived between 146 and 156. The transport probes BoringSSL and degrades
/// to the 8 classics until its snapshot learns the values (#25); advertising
/// schemes we cannot verify would only matter if servers issued ML-DSA certs,
/// which production does not yet.
pub const SIGALGS_V157_TAIL: [u16; 11] = [
    0x0904, 0x0905, 0x0906, 0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601,
];

/// Chrome 157's full verify list: a `0x?A?A` GREASE value leads (drawn per
/// emulation build; real Chrome redraws per connection). Unpredictable, not
/// cryptographic — same trick as the HTTP/2 grease setting.
fn sigalgs_v157() -> Vec<u16> {
    use std::{
        collections::hash_map::RandomState,
        hash::{BuildHasher, Hasher},
    };
    let mut h = RandomState::new().build_hasher();
    h.write_u8(0);
    let r = h.finish();
    let grease = 0x0A0A | ((((r >> 8) & 0xF) as u16) << 12) | ((((r >> 16) & 0xF) as u16) << 4);
    let mut prefs = Vec::with_capacity(SIGALGS_V157_TAIL.len() + 1);
    prefs.push(grease);
    prefs.extend_from_slice(&SIGALGS_V157_TAIL);
    prefs
}

pub const CERTIFICATE_COMPRESSORS: &[&'static dyn CertificateCompressor] = &[&BrotliCompressor];

#[derive(TypedBuilder)]
pub struct ChromeTlsConfig {
    #[builder(default = CURVES_1)]
    curves: &'static str,

    #[builder(default = SIGALGS_LIST)]
    sigalgs_list: &'static str,

    #[builder(default = CIPHER_LIST)]
    cipher_list: &'static str,

    #[builder(default = AlpsProtocol::HTTP2, setter(into))]
    alps_protos: AlpsProtocol,

    #[builder(default = false)]
    alps_use_new_codepoint: bool,

    #[builder(default = false, setter(into))]
    enable_ech_grease: bool,

    #[builder(default = false, setter(into))]
    permute_extensions: bool,

    #[builder(default = false, setter(into))]
    pre_shared_key: bool,

    /// Send the `trust_anchors` extension (0xCA34) with an EMPTY list. Real Chrome 145
    /// does (payload 0000); versions we have no capture for are left as they were.
    #[builder(default = false, setter(into))]
    empty_trust_anchors: bool,

    /// Lead the verify sigalgs with GREASE + ML-DSA as Chrome 157 does
    /// (see [`SIGALGS_V157_TAIL`]); `sigalgs_list` then only seeds the signing side.
    #[builder(default = false, setter(into))]
    sigalgs_mldsa_grease: bool,
}

impl From<ChromeTlsConfig> for TlsOptions {
    fn from(val: ChromeTlsConfig) -> Self {
        TlsOptions::builder()
            .grease_enabled(true)
            .enable_ocsp_stapling(true)
            .enable_signed_cert_timestamps(true)
            .curves_list(val.curves)
            .sigalgs_list(val.sigalgs_list)
            .sigalgs_verify_prefs(if val.sigalgs_mldsa_grease {
                Some(std::borrow::Cow::Owned(sigalgs_v157()))
            } else {
                None
            })
            .cipher_list(val.cipher_list)
            .min_tls_version(TlsVersion::TLS_1_2)
            .max_tls_version(TlsVersion::TLS_1_3)
            .permute_extensions(val.permute_extensions)
            .pre_shared_key(val.pre_shared_key)
            .enable_ech_grease(val.enable_ech_grease)
            .alps_protocols([val.alps_protos])
            .alps_use_new_codepoint(val.alps_use_new_codepoint)
            .requested_trust_anchors(
                val.empty_trust_anchors
                    .then_some(std::borrow::Cow::Borrowed(&[][..])),
            )
            .aes_hw_override(true)
            .certificate_compressors(CERTIFICATE_COMPRESSORS)
            .build()
    }
}
