//! WebCrypto 全家：算法 roundtrip + like_chrome 面拍（单 API 电池，超 2500 例外）
use super::super::*;
use super::common::setup_runtime;

#[tokio::test(flavor = "current_thread")]
async fn crypto_derive_output_lengths_are_capped() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // obscura #910 family: page-chosen crypto output lengths (HMAC
    // generateKey key length, HKDF deriveBits output) must fail with a
    // catchable error, not allocate the requested size first — the old
    // shape was an instant OOM abort on `vec![0u8; 2**28]`.
    let script = r#"async () => {
            const results = [];
            try {
                await crypto.subtle.generateKey({ name: 'HMAC', hash: 'SHA-256', length: 2 ** 31 }, false, ['sign']);
                results.push('hmac-generated');
            } catch (e) { results.push(e.message); }
            try {
                const k = await crypto.subtle.importKey('raw', new Uint8Array(16), { name: 'HKDF' }, false, ['deriveBits']);
                await crypto.subtle.deriveBits(
                    { name: 'HKDF', hash: 'SHA-256', salt: new Uint8Array(0), info: new Uint8Array(0) },
                    k, 2 ** 31);
                results.push('hkdf-derived');
            } catch (e) { results.push(e.message); }
            return results;
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let values = result.value.unwrap();
    let arr = values.as_array().unwrap();
    assert!(
        arr[0].as_str().unwrap().contains("65536"),
        "oversized HMAC key must be rejected: {arr:?}"
    );
    // HKDF now stops at Chrome's own 255*HashLen bound before any
    // allocation; the op's 65536-byte cap stays as the backstop.
    assert_eq!(
        arr[1].as_str().unwrap(),
        "The length provided for HKDF is too large.",
        "oversized HKDF output must be rejected: {arr:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_subtle_digest_variants_and_rejection() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // SHA-512/224 and SHA-512/256 were silently falling through to SHA-256,
    // and unknown names (MD5) returned a SHA-256 hash with no error. Verify
    // the FIPS 180-4 test vectors and the NotSupportedError rejection.
    let script = r#"async () => {
            const hex = (buf) => Array.from(new Uint8Array(buf)).map(b => b.toString(16).padStart(2, '0')).join('');
            const enc = new TextEncoder();
            const sha256 = hex(await crypto.subtle.digest('SHA-256', enc.encode('abc')));
            const sha512_224 = hex(await crypto.subtle.digest('SHA-512/224', enc.encode('abc')));
            const sha512_256 = hex(await crypto.subtle.digest('SHA-512/256', enc.encode('abc')));
            let threw = false;
            try { await crypto.subtle.digest('MD5', enc.encode('abc')); } catch (e) {
                threw = e.name === 'NotSupportedError';
            }
            return [sha256, sha512_224, sha512_256, threw];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "4634270f707b6a54daae7530460842e20e37ed265ceee9a43e8924aa",
            "53048e2681941ef99b2e29b76b4c7dabe4c2d0c634fc6d46e0e2f13107e7af23",
            true
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_secret_key_roundtrips() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // HMAC sign/verify, AES-GCM and AES-CBC encrypt/decrypt roundtrips, and
    // PBKDF2/HKDF derivation all work through the RustCrypto ops (the old
    // stubs returned fake data).
    let script = r#"async () => {
            const enc = new TextEncoder();
            const dec = new TextDecoder();

            // HMAC sign/verify (RFC 4231 key/data).
            const hk = await crypto.subtle.importKey('raw', enc.encode('key'), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign', 'verify']);
            const sig = await crypto.subtle.sign('HMAC', hk, enc.encode('The quick brown fox jumps over the lazy dog'));
            const sigHex = Array.from(new Uint8Array(sig)).map(b => b.toString(16).padStart(2, '0')).join('');
            const verifyOk = await crypto.subtle.verify('HMAC', hk, sig, enc.encode('The quick brown fox jumps over the lazy dog'));
            const verifyBad = await crypto.subtle.verify('HMAC', hk, sig, enc.encode('tampered'));

            // AES-GCM roundtrip.
            const gk = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, true, ['encrypt', 'decrypt']);
            const giv = crypto.getRandomValues(new Uint8Array(12));
            const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: giv }, gk, enc.encode('hello gcm'));
            const pt = dec.decode(await crypto.subtle.decrypt({ name: 'AES-GCM', iv: giv }, gk, ct));

            // AES-CBC roundtrip.
            const ck = await crypto.subtle.generateKey({ name: 'AES-CBC', length: 128 }, true, ['encrypt', 'decrypt']);
            const civ = crypto.getRandomValues(new Uint8Array(16));
            const cct = await crypto.subtle.encrypt({ name: 'AES-CBC', iv: civ }, ck, enc.encode('hello cbc'));
            const cpt = dec.decode(await crypto.subtle.decrypt({ name: 'AES-CBC', iv: civ }, ck, cct));

            // PBKDF2 derivation (RFC 6070 vector: PBKDF2-HMAC-SHA256, 1 iter).
            const pk = await crypto.subtle.importKey('raw', enc.encode('password'), { name: 'PBKDF2' }, false, ['deriveBits']);
            const dk = await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('salt'), iterations: 1 }, pk, 256);
            const dkHex = Array.from(new Uint8Array(dk)).map(b => b.toString(16).padStart(2, '0')).join('');

            return [sigHex, verifyOk, !verifyBad, pt, cpt, dkHex];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    // RFC 4231 HMAC-SHA-256("key", "The quick brown fox...") =
    //   f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8
    // RFC 6070 PBKDF2-HMAC-SHA256("password", "salt", 1, 32) =
    //   120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!([
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
            true,
            true,
            "hello gcm",
            "hello cbc",
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_ed25519_vector_and_roundtrips() {
    // RFC 8032 section 7.1 test 1 (empty message) pins the curve math;
    // the rest exercises every import/export format and the usage rules.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
            const hex = (b) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('');
            const unhex = (h) => Uint8Array.from(h.match(/../g).map((x) => parseInt(x, 16)));
            const seed = '9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60';
            const pub = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a';
            const sigHex = 'e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b';
            const pk8 = unhex('302e020100300506032b657004220420' + seed);
            const priv = await crypto.subtle.importKey('pkcs8', pk8, 'Ed25519', true, ['sign']);
            const sig = await crypto.subtle.sign('Ed25519', priv, new Uint8Array(0));
            const pubKey = await crypto.subtle.importKey('raw', unhex(pub), { name: 'Ed25519' }, true, ['verify']);
            const ok = await crypto.subtle.verify('Ed25519', pubKey, sig, new Uint8Array(0));
            const jwk = await crypto.subtle.exportKey('jwk', priv);
            const jwkBack = await crypto.subtle.importKey('jwk', jwk, 'Ed25519', true, ['sign']);
            const spki = await crypto.subtle.exportKey('spki', pubKey);
            const spkiKey = await crypto.subtle.importKey('spki', spki, 'Ed25519', true, ['verify']);
            const g = await crypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
            const msg = new TextEncoder().encode('hello');
            const gs = await crypto.subtle.sign('Ed25519', g.privateKey, msg);
            const gOk = await crypto.subtle.verify('Ed25519', g.publicKey, gs, msg);
            const gBad = await crypto.subtle.verify('Ed25519', g.publicKey, gs, new TextEncoder().encode('hellp'));
            let usage = '', badRaw = '';
            try { await crypto.subtle.sign('Ed25519', g.publicKey, msg); } catch (e) { usage = e.name; }
            try { await crypto.subtle.importKey('raw', new Uint8Array(31), 'Ed25519', true, ['verify']); } catch (e) { badRaw = e.name; }
            return [hex(sig) === sigHex, ok, Object.keys(jwk).join(','), jwk.kty + '/' + jwk.crv, hex(await crypto.subtle.exportKey('pkcs8', jwkBack)) === hex(pk8),
              hex(spki) === '302a300506032b6570032100' + pub, spkiKey.algorithm.name, gs.byteLength, gOk, gBad, usage, badRaw,
              g.publicKey.usages.join(), g.privateKey.usages.join(), g.publicKey.extractable];
        }"#;
    let mut result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.take().unwrap();
    assert_eq!(
        v,
        serde_json::json!([
            true,
            true,
            "crv,d,ext,key_ops,kty,x",
            "OKP/Ed25519",
            true,
            true,
            "Ed25519",
            64,
            true,
            false,
            "InvalidAccessError",
            "DataError",
            "verify",
            "sign",
            true
        ])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_ecdsa_roundtrips() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // #103: ECDSA P-256/P-384 through the real face — generate → export
    // (spki/pkcs8/jwk) → sign → verify, tamper rejection, and re-import
    // from every format the Douyin security SDK uses (jwk with a private
    // scalar, spki, raw point).
    let script = r#"async () => {
            const enc = new TextEncoder();

            // P-256 generate + shape.
            const { publicKey, privateKey } = await crypto.subtle.generateKey(
                { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
            const shape = [
                publicKey.type, publicKey.algorithm.name, publicKey.algorithm.namedCurve,
                publicKey.usages.join(','), publicKey.extractable,
                privateKey.type, privateKey.usages.join(','),
            ];

            // Sign → 64-byte P1363 (r||s).
            const data = enc.encode('diting ecdsa p256');
            const sig = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, privateKey, data);
            const sigLen = new Uint8Array(sig).length;

            // Verify with the generated public key, and with a re-imported
            // spki copy — both true; tampered data false.
            const ok = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, publicKey, sig, data);
            const spki = await crypto.subtle.exportKey('spki', publicKey);
            const spkiKey = await crypto.subtle.importKey('spki', spki, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['verify']);
            const okSpki = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, spkiKey, sig, data);
            const bad = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, publicKey, sig, enc.encode('tampered'));

            // pkcs8 re-import signs; raw point import verifies.
            const pkcs8 = await crypto.subtle.exportKey('pkcs8', privateKey);
            const pkcs8Key = await crypto.subtle.importKey('pkcs8', pkcs8, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
            const sig2 = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, pkcs8Key, data);
            const okSig2 = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, publicKey, sig2, data);

            // jwk export/import roundtrip — the shape the Douyin SDK feeds us.
            const privJwk = await crypto.subtle.exportKey('jwk', privateKey);
            const pubJwk = await crypto.subtle.exportKey('jwk', publicKey);
            const jwkShape = [privJwk.kty, privJwk.crv, privJwk.d !== undefined, pubJwk.d !== undefined, privJwk.x === pubJwk.x, privJwk.y === pubJwk.y];
            const jwkPriv = await crypto.subtle.importKey('jwk', privJwk, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
            const jwkPub = await crypto.subtle.importKey('jwk', pubJwk, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['verify']);
            const sig3 = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, jwkPriv, data);
            const okSig3 = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, jwkPub, sig3, data);

            // P-384: 96-byte signature, full roundtrip.
            const k384 = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-384' }, true, ['sign', 'verify']);
            const sig384 = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-384' }, k384.privateKey, data);
            const ok384 = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-384' }, k384.publicKey, sig384, data);

            // A raw-point import (SEC1 uncompressed) of the P-256 public key
            // verifies too — some SDKs pass raw points.
            const raw = await crypto.subtle.exportKey('raw', publicKey);
            const rawKey = await crypto.subtle.importKey('raw', raw, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['verify']);
            const okRaw = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, rawKey, sig, data);

            return [shape, sigLen, ok, okSpki, !bad, okSig2, jwkShape, okSig3, new Uint8Array(sig384).length, ok384, okRaw];
        }"#;
    let mut result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let value = result.value.take().unwrap();
    assert_eq!(
        value[0],
        serde_json::json!(["public", "ECDSA", "P-256", "verify", true, "private", "sign"])
    );
    assert_eq!(value[1], 64, "P-256 P1363 signature must be 64 bytes");
    assert_eq!(value[2], true, "generated key verifies its own signature");
    assert_eq!(value[3], true, "spki re-import verifies");
    assert_eq!(value[4], true, "tampered data must fail verification");
    assert_eq!(value[5], true, "pkcs8 re-import signature verifies");
    assert_eq!(
        value[6],
        serde_json::json!(["EC", "P-256", true, false, true, true]),
        "jwk export shape: private has d, public does not, x/y match"
    );
    assert_eq!(value[7], true, "jwk roundtrip signs and verifies");
    assert_eq!(value[8], 96, "P-384 P1363 signature must be 96 bytes");
    assert_eq!(value[9], true, "P-384 roundtrip verifies");
    assert_eq!(value[10], true, "raw SEC1 point re-import verifies");
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_ecdh_derivation() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // ECDH P-256/P-384 key agreement — the Douyin login chain's
    // deriveEcdhKey step: generate two pairs, both directions of the
    // exchange must land on the same secret; deriveKey must yield a
    // working AES-GCM key; curve mismatch rejects; usage faces follow
    // Chrome (public ECDH keys carry no usages).
    let script = r#"async () => {
            const enc = new TextEncoder();
            const dec = new TextDecoder();
            const a = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveKey', 'deriveBits']);
            const b = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
            const shape = [
                a.publicKey.type, a.publicKey.usages.length,
                a.privateKey.usages.slice().sort().join(','),
            ];

            const s1 = new Uint8Array(await crypto.subtle.deriveBits({ name: 'ECDH', public: b.publicKey }, a.privateKey, null));
            const s2 = new Uint8Array(await crypto.subtle.deriveBits({ name: 'ECDH', public: a.publicKey }, b.privateKey, 256));
            const symmetric = s1.length === 32 && s2.length === 32 && s1.every((v, i) => v === s2[i]);

            // deriveKey → AES-GCM roundtrip through the shared secret.
            const aes = await crypto.subtle.deriveKey(
                { name: 'ECDH', public: b.publicKey }, a.privateKey,
                { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
            const iv = crypto.getRandomValues(new Uint8Array(12));
            const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, aes, enc.encode('ecdh secret message'));
            const pt = dec.decode(await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, aes, ct));

            // jwk export/import roundtrip of an ECDH public key.
            const pubJwk = await crypto.subtle.exportKey('jwk', a.publicKey);
            const rePub = await crypto.subtle.importKey('jwk', pubJwk, { name: 'ECDH', namedCurve: 'P-256' }, true, []);
            const s3 = new Uint8Array(await crypto.subtle.deriveBits({ name: 'ECDH', public: rePub }, b.privateKey, null));
            const jwkOk = s3.length === 32 && s3.every((v, i) => v === s1[i]);

            // P-384 agreement works; cross-curve mix rejects.
            const p384a = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-384' }, false, ['deriveBits']);
            const p384b = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-384' }, false, ['deriveBits']);
            const s384 = new Uint8Array(await crypto.subtle.deriveBits({ name: 'ECDH', public: p384b.publicKey }, p384a.privateKey, null));
            let mixedThrew = false;
            try { await crypto.subtle.deriveBits({ name: 'ECDH', public: b.publicKey }, p384a.privateKey, null); }
            catch (e) { mixedThrew = e.name === 'InvalidAccessError'; }

            return [shape, symmetric, pt, jwkOk, s384.length, mixedThrew];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v[0],
        serde_json::json!(["public", 0, "deriveBits,deriveKey"]),
        "ECDH key shape: public carries no usages, private carries both derive usages"
    );
    assert_eq!(v[1], true, "both directions of the exchange agree");
    assert_eq!(v[2], serde_json::json!("ecdh secret message"));
    assert_eq!(v[3], true, "jwk roundtrip preserves the public point");
    assert_eq!(v[4], 48, "P-384 shared secret is 48 bytes");
    assert_eq!(v[5], true, "cross-curve derivation rejects");
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_pbkdf2_rejects_excessive_iterations() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // A page asking for 2^32 iterations must not pin the single-threaded
    // runtime; the op rejects it with OperationError (upstream cfda91b).
    let script = r#"async () => {
            const enc = new TextEncoder();
            const pk = await crypto.subtle.importKey('raw', enc.encode('password'), { name: 'PBKDF2' }, false, ['deriveBits']);
            try {
                await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('salt'), iterations: 4294967295 }, pk, 256);
                return 'no-throw';
            } catch (e) {
                return e.name;
            }
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!("OperationError"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_aes_kw_wrap_unwrap() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // #269: AES-KW (RFC 3394) wrapKey/unwrapKey against the RFC's own
    // test vectors and live-Chrome-155 error faces (AES-KW is reachable
    // only through wrapKey/unwrapKey; direct encrypt throws).
    let script = r#"async () => {
            const enc = new TextEncoder();
            const hex = (u8) => Array.from(u8, b => b.toString(16).padStart(2, '0')).join('');
            const face = async (p) => { try { await p; return 'no-throw'; } catch (e) { return e.name + ': ' + e.message; } };
            const S = crypto.subtle;

            // RFC 3394 §4.1: KEK 000102..0f wraps 00112233..ff.
            const kek = await S.importKey('raw', new Uint8Array([0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]), { name: 'AES-KW' }, true, ['wrapKey', 'unwrapKey']);
            const dat = await S.importKey('raw', new Uint8Array([0x00,0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88,0x99,0xaa,0xbb,0xcc,0xdd,0xee,0xff]), { name: 'AES-GCM' }, true, ['encrypt']);
            const v1 = hex(new Uint8Array(await S.wrapKey('raw', dat, kek, 'AES-KW')));
            // 256-bit KEK wrapping 32 bytes of key data (Chrome-155 capture).
            const kek256 = await S.importKey('raw', new Uint8Array([...Array(32).keys()]), { name: 'AES-KW' }, true, ['wrapKey', 'unwrapKey']);
            const dat32 = await S.importKey('raw', new Uint8Array([...Array(32).keys()]), { name: 'AES-GCM' }, true, ['encrypt']);
            const v2 = hex(new Uint8Array(await S.wrapKey('raw', dat32, kek256, 'AES-KW')));
            // Unwrap round-trips back to the exact key bytes.
            const unwrapped = await S.unwrapKey('raw', await S.wrapKey('raw', dat, kek, 'AES-KW'), kek, 'AES-KW', { name: 'AES-GCM', length: 128 }, true, ['encrypt']);
            const round = hex(new Uint8Array(await S.exportKey('raw', unwrapped)));

            const gen = await S.generateKey({ name: 'AES-KW', length: 256 }, true, ['wrapKey', 'unwrapKey']);
            const genShape = [gen.type, gen.usages.join(','), gen.algorithm.name, gen.algorithm.length];
            const expJwk = await S.exportKey('jwk', gen);

            const errors = {
                jwkFormat: await face(S.wrapKey('jwk', dat, kek, { name: 'AES-KW' })),
                directEncrypt: await face(S.encrypt({ name: 'AES-KW' }, kek, new Uint8Array(16))),
                directDecrypt: await face(S.decrypt({ name: 'AES-KW' }, kek, new Uint8Array(24))),
                noWrapUsage: await face((async () => {
                    const k = await S.generateKey({ name: 'AES-KW', length: 128 }, true, ['unwrapKey']);
                    return S.wrapKey('raw', dat, k, { name: 'AES-KW' });
                })()),
                corrupt: await face((async () => {
                    const w = new Uint8Array(await S.wrapKey('raw', dat, kek, { name: 'AES-KW' }));
                    w[3] ^= 0xff;
                    return S.unwrapKey('raw', w, kek, 'AES-KW', { name: 'AES-GCM', length: 128 }, true, ['encrypt']);
                })()),
                unwrapBadUsage: await face(S.unwrapKey('raw', await S.wrapKey('raw', dat, kek, { name: 'AES-KW' }), kek, 'AES-KW', { name: 'AES-GCM', length: 128 }, true, ['sign'])),
                gen192: await face(S.generateKey({ name: 'AES-KW', length: 192 }, true, ['wrapKey'])),
                gen100: await face(S.generateKey({ name: 'AES-KW', length: 100 }, true, ['wrapKey'])),
                genEncryptUsage: await face(S.generateKey({ name: 'AES-KW', length: 128 }, true, ['encrypt'])),
                raw192: await face(S.importKey('raw', new Uint8Array(24), { name: 'AES-KW' }, true, ['wrapKey'])),
                jwk192: await face(S.importKey('jwk', { kty: 'oct', k: btoa('123456789012345678901234').replace(/\+/g, '-').replace(/\//g, '_') }, { name: 'AES-KW', length: 192 }, true, ['wrapKey'])),
                emptyUsages: await face(S.importKey('raw', new Uint8Array(16), { name: 'AES-KW' }, true, [])),
                bogusUsage: await face(S.importKey('raw', new Uint8Array(16), { name: 'AES-KW' }, true, ['bogus'])),
                hmac64Wrap: await (async () => {
                    const h = await S.generateKey({ name: 'HMAC', hash: 'SHA-512' }, true, ['sign']);
                    const w = await S.wrapKey('raw', h, gen, { name: 'AES-KW' });
                    return w.byteLength;
                })(),
                gcmWrap: await (async () => {
                    const g = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['wrapKey']);
                    const w = await S.wrapKey('raw', dat, g, { name: 'AES-GCM', iv: new Uint8Array(12) });
                    return w.byteLength;
                })(),
            };
            return { v1, v2, round, genShape, expOrder: Object.keys(expJwk), expAlg: expJwk.alg, expOps: expJwk.key_ops.join(','), errors };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["v1"],
        serde_json::json!("1fa68b0a8112b447aef34bd8fb5a7b829d3e862371d2cfe5"),
        "RFC 3394 §4.1 wrap vector"
    );
    assert_eq!(
        v["v2"],
        serde_json::json!(
            "82c693bff487db7c31a7f0cc440ad9e37709511b52efce48094b4548eee3cfc2bf7805b51201bc01"
        ),
        "256-bit KEK over 32 bytes of data (Chrome-155 capture)"
    );
    assert_eq!(
        v["round"],
        serde_json::json!("00112233445566778899aabbccddeeff"),
        "unwrap restores the key bytes"
    );
    assert_eq!(
        v["genShape"],
        serde_json::json!(["secret", "wrapKey,unwrapKey", "AES-KW", 256])
    );
    assert_eq!(
        v["expOrder"],
        serde_json::json!(["alg", "ext", "k", "key_ops", "kty"]),
        "jwk export fields in Chrome's alphabetical order"
    );
    assert_eq!(v["expAlg"], serde_json::json!("A256KW"));
    assert_eq!(v["expOps"], serde_json::json!("wrapKey,unwrapKey"));
    let e = &v["errors"];
    assert_eq!(
        e["jwkFormat"],
        serde_json::json!(
            "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes"
        )
    );
    assert_eq!(
        e["directEncrypt"],
        serde_json::json!(
            "NotSupportedError: Failed to execute 'encrypt' on 'SubtleCrypto': AES-KW: Unsupported operation: encrypt"
        )
    );
    assert_eq!(
        e["directDecrypt"],
        serde_json::json!(
            "NotSupportedError: Failed to execute 'decrypt' on 'SubtleCrypto': AES-KW: Unsupported operation: decrypt"
        )
    );
    assert_eq!(
        e["noWrapUsage"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation")
    );
    assert_eq!(
        e["corrupt"],
        serde_json::json!("OperationError: "),
        "corrupt unwrap fails with Chrome's empty-message OperationError"
    );
    assert_eq!(
        e["unwrapBadUsage"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["gen192"],
        serde_json::json!("OperationError: 192-bit AES keys are not supported")
    );
    assert_eq!(
        e["gen100"],
        serde_json::json!("OperationError: AES key length must be 128 or 256 bits")
    );
    assert_eq!(
        e["genEncryptUsage"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["raw192"],
        serde_json::json!("OperationError: 192-bit AES keys are not supported")
    );
    // Chrome 155 said DataError ("right length") on the jwk path; 157 gives
    // the raw path's OperationError for this exact input (re-probed).
    assert_eq!(
        e["jwk192"],
        serde_json::json!("OperationError: 192-bit AES keys are not supported")
    );
    assert_eq!(
        e["emptyUsages"],
        serde_json::json!("SyntaxError: Usages cannot be empty when creating a key.")
    );
    assert_eq!(
        e["bogusUsage"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': Invalid keyUsages argument"
        )
    );
    assert_eq!(
        e["hmac64Wrap"],
        serde_json::json!(136),
        "HMAC-SHA-512 default key is 128 bytes → 136-byte wrap"
    );
    assert_eq!(
        e["gcmWrap"],
        serde_json::json!(32),
        "AES-GCM wrapper (16-byte key + 16-byte tag)"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_rsa_oaep_import_export_cipher() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // #269: RSA-OAEP generate/import/export/encrypt faces pinned from
    // Chrome 155 — jwk field order (alphabetical), spki/pkcs8 DER shape,
    // OAEP round-trips with labels, and the exact error wording.
    let script = r#"async () => {
            const enc = new TextEncoder();
            const dec = new TextDecoder();
            const hex = (u8) => Array.from(u8, b => b.toString(16).padStart(2, '0')).join('');
            const face = async (p) => { try { await p; return 'no-throw'; } catch (e) { return e.name + ': ' + e.message; } };
            const S = crypto.subtle;
            const OA = { name: 'RSA-OAEP', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' };

            const pair = await S.generateKey(OA, true, ['encrypt', 'decrypt', 'wrapKey', 'unwrapKey']);
            const genShape = [
                pair.publicKey.type, pair.publicKey.usages.join(','), pair.publicKey.extractable,
                pair.privateKey.type, pair.privateKey.usages.join(','), pair.privateKey.extractable,
                pair.publicKey.algorithm.name, pair.publicKey.algorithm.modulusLength,
                Array.from(pair.publicKey.algorithm.publicExponent).join(','),
                pair.publicKey.algorithm.hash.name,
            ];

            const jwkPub = await S.exportKey('jwk', pair.publicKey);
            const jwkPriv = await S.exportKey('jwk', pair.privateKey);
            const pubOrder = Object.keys(jwkPub);
            const privOrder = Object.keys(jwkPriv);

            const spki = new Uint8Array(await S.exportKey('spki', pair.publicKey));
            const pkcs8 = new Uint8Array(await S.exportKey('pkcs8', pair.privateKey));
            const spkiHex = hex(spki);
            const pkcs8Hex = hex(pkcs8);

            // Round-trips: spki/pkcs8/jwk imports all reproduce the same key.
            const rePub = await S.importKey('spki', spki, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt']);
            const spkiRt = (await S.exportKey('jwk', rePub)).n === jwkPub.n;
            const rePriv = await S.importKey('pkcs8', pkcs8, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt']);
            const pkcs8Rt = (await S.exportKey('jwk', rePriv)).d === jwkPriv.d;
            const jwkPrivRe = await S.importKey('jwk', jwkPriv, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt']);
            const jwkRt = (await S.exportKey('jwk', jwkPrivRe)).d === jwkPriv.d;
            const bare = await S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt']);

            // encrypt/decrypt, labels, and corruption.
            const msg = enc.encode('attack at dawn');
            const ct = new Uint8Array(await S.encrypt({ name: 'RSA-OAEP' }, pair.publicKey, msg));
            const pt = dec.decode(await S.decrypt({ name: 'RSA-OAEP' }, pair.privateKey, ct));
            const ctLbl = new Uint8Array(await S.encrypt({ name: 'RSA-OAEP', label: enc.encode('lbl') }, pair.publicKey, msg));
            const ptLbl = dec.decode(await S.decrypt({ name: 'RSA-OAEP', label: enc.encode('lbl') }, pair.privateKey, ctLbl));
            const bad = ct.slice(); bad[13] ^= 1;

            // wrapKey with the RSA wrapper.
            const aesk = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['encrypt']);
            const rw = await S.wrapKey('raw', aesk, pair.publicKey, { name: 'RSA-OAEP' });
            const uw = await S.unwrapKey('raw', rw, pair.privateKey, { name: 'RSA-OAEP' }, { name: 'AES-GCM', length: 128 }, true, ['encrypt']);

            const errors = {
                encPriv: await face(S.encrypt({ name: 'RSA-OAEP' }, pair.privateKey, msg)),
                decPub: await face(S.decrypt({ name: 'RSA-OAEP' }, pair.publicKey, ct)),
                corrupt: await face(S.decrypt({ name: 'RSA-OAEP' }, pair.privateKey, bad)),
                labelMismatch: await face(S.decrypt({ name: 'RSA-OAEP' }, pair.privateKey, ctLbl)),
                shortCt: await face(S.decrypt({ name: 'RSA-OAEP' }, pair.privateKey, new Uint8Array(10))),
                exportSpkiPriv: await face(S.exportKey('spki', pair.privateKey)),
                exportRaw: await face(S.exportKey('raw', pair.publicKey)),
                importSpkiDecrypt: await face(S.importKey('spki', spki, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt'])),
                spkiNoHash: await face(S.importKey('spki', spki, { name: 'RSA-OAEP' }, true, ['encrypt'])),
                pkcs8NoHash: await face(S.importKey('pkcs8', pkcs8, { name: 'RSA-OAEP' }, true, ['decrypt'])),
                genE1: await face(S.generateKey({ name: 'RSA-OAEP', modulusLength: 1024, publicExponent: new Uint8Array([1]), hash: 'SHA-256' }, true, ['encrypt', 'decrypt'])),
                genMod1023: await face(S.generateKey({ name: 'RSA-OAEP', modulusLength: 1023, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['encrypt', 'decrypt'])),
                genMd5: await face(S.generateKey({ name: 'RSA-OAEP', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: 'MD5' }, true, ['encrypt', 'decrypt'])),
                genEmpty: await face(S.generateKey({ name: 'RSA-OAEP', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, [])),
                genSign: await face(S.generateKey({ name: 'RSA-OAEP', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['sign'])),
                jwkAlg: await face(S.importKey('jwk', jwkPub, { name: 'RSA-OAEP', hash: 'SHA-1' }, true, ['encrypt'])),
                jwkKty: await face(S.importKey('jwk', { kty: 'oct', k: 'AAAA' }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkMissingN: await face(S.importKey('jwk', { kty: 'RSA', e: jwkPub.e }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkMissingE: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkMissingD: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, p: jwkPriv.p }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt'])),
                jwkDOnly: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, d: jwkPriv.d }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt'])),
                jwkExtFalse: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, ext: false }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkOpsDup: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, key_ops: ['encrypt', 'encrypt'] }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkOpsConflict: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, key_ops: ['decrypt'] }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                jwkUseSig: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e, use: 'sig' }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
                usageBogus: await face(S.importKey('jwk', { kty: 'RSA', n: jwkPub.n, e: jwkPub.e }, { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['nope'])),
                rawFormat: await face(S.importKey('raw', new Uint8Array(4), { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['encrypt'])),
            };
            return {
                genShape, pubOrder, privOrder, jwkPubAlg: jwkPub.alg, jwkE: jwkPub.e,
                spkiLen: spki.length,
                spkiOk: spkiHex.startsWith('3081') && spkiHex.includes('2a864886f70d010101'),
                pkcs8Len: pkcs8.length, pkcs8HeadOk: pkcs8Hex.startsWith('3082') && pkcs8Hex.includes('020100300d06092a864886f70d01010105000482'),
                rt: [spkiRt, pkcs8Rt, jwkRt], bareUsages: bare.usages.join(','), bareExt: bare.extractable,
                ctLen: ct.length, pt, ptLbl, wrapLen: rw.byteLength, unwrapUsages: uw.usages.join(','),
                privAlgNames: jwkPriv.alg, errors,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["genShape"],
        serde_json::json!([
            "public",
            "encrypt,wrapKey",
            true,
            "private",
            "decrypt,unwrapKey",
            true,
            "RSA-OAEP",
            1024,
            "1,0,1",
            "SHA-256"
        ]),
        "generateKey splits usages across the halves exactly like Chrome"
    );
    assert_eq!(
        v["pubOrder"],
        serde_json::json!(["alg", "e", "ext", "key_ops", "kty", "n"]),
        "public jwk fields in Chrome's order"
    );
    assert_eq!(
        v["privOrder"],
        serde_json::json!([
            "alg", "d", "dp", "dq", "e", "ext", "key_ops", "kty", "n", "p", "q", "qi"
        ]),
        "private jwk fields in Chrome's order"
    );
    assert_eq!(v["jwkPubAlg"], serde_json::json!("RSA-OAEP-256"));
    assert_eq!(v["jwkE"], serde_json::json!("AQAB"));
    let spki_len = v["spkiLen"].as_u64().unwrap();
    assert!(
        (162..=163).contains(&spki_len),
        "1024-bit spki is 162 bytes (163 when the modulus MSB needs a 0x00 pad), got {spki_len}"
    );
    assert_eq!(
        v["spkiOk"],
        serde_json::json!(true),
        "spki is a DER SEQUENCE carrying the rsaEncryption OID 2a864886f70d010101"
    );
    assert_eq!(
        v["pkcs8HeadOk"],
        serde_json::json!(true),
        "pkcs8 is a standard PrivateKeyInfo wrapping RSAPrivateKey"
    );
    let pkcs8_len = v["pkcs8Len"].as_u64().unwrap();
    assert!(
        (605..=640).contains(&pkcs8_len),
        "1024-bit pkcs8 length in the leading-zero-variable band, got {pkcs8_len}"
    );
    assert_eq!(
        v["rt"],
        serde_json::json!([true, true, true]),
        "spki/pkcs8/jwk imports reproduce the key"
    );
    assert_eq!(v["bareUsages"], serde_json::json!("encrypt"));
    assert_eq!(
        v["ctLen"],
        serde_json::json!(128),
        "1024-bit OAEP ciphertext"
    );
    assert_eq!(v["pt"], serde_json::json!("attack at dawn"));
    assert_eq!(v["ptLbl"], serde_json::json!("attack at dawn"));
    assert_eq!(v["wrapLen"], serde_json::json!(128));
    assert_eq!(v["unwrapUsages"], serde_json::json!("encrypt"));
    let e = &v["errors"];
    assert_eq!(
        e["encPriv"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation")
    );
    assert_eq!(
        e["decPub"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation")
    );
    assert_eq!(e["corrupt"], serde_json::json!("OperationError: "));
    assert_eq!(e["labelMismatch"], serde_json::json!("OperationError: "));
    assert_eq!(e["shortCt"], serde_json::json!("OperationError: "));
    assert_eq!(
        e["exportSpkiPriv"],
        serde_json::json!("InvalidAccessError: The key is not of the expected type")
    );
    assert_eq!(
        e["exportRaw"],
        serde_json::json!("NotSupportedError: Unsupported export key format for algorithm")
    );
    assert_eq!(
        e["importSpkiDecrypt"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["spkiNoHash"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': RsaHashedImportParams: hash: Missing or not an AlgorithmIdentifier"
        )
    );
    assert_eq!(
        e["pkcs8NoHash"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': RsaHashedImportParams: hash: Missing or not an AlgorithmIdentifier"
        )
    );
    assert_eq!(
        e["genE1"],
        serde_json::json!("OperationError: The \"publicExponent\" must be either 3 or 65537")
    );
    assert_eq!(
        e["genMod1023"],
        serde_json::json!(
            "OperationError: The modulus length must be a multiple of 8 bits and >= 256 and <= 8192"
        )
    );
    assert_eq!(
        e["genMd5"],
        serde_json::json!(
            "NotSupportedError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: hash: Algorithm: Unrecognized name"
        )
    );
    assert_eq!(
        e["genEmpty"],
        serde_json::json!("SyntaxError: Usages cannot be empty when creating a key.")
    );
    assert_eq!(
        e["genSign"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["jwkAlg"],
        serde_json::json!(
            "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call"
        )
    );
    assert_eq!(
        e["jwkKty"],
        serde_json::json!("DataError: The JWK \"kty\" member was not \"RSA\"")
    );
    assert_eq!(
        e["jwkMissingN"],
        serde_json::json!("DataError: The required JWK member \"n\" was missing")
    );
    assert_eq!(
        e["jwkMissingE"],
        serde_json::json!("DataError: The required JWK member \"e\" was missing")
    );
    assert_eq!(
        e["jwkMissingD"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages."),
        "no d ⇒ public key ⇒ decrypt usage rejected first"
    );
    assert_eq!(
        e["jwkDOnly"],
        serde_json::json!("DataError: The required JWK member \"p\" was missing"),
        "private jwk needs the full CRT set"
    );
    assert_eq!(
        e["jwkExtFalse"],
        serde_json::json!(
            "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call"
        ),
        "Chrome's typo kept verbatim"
    );
    assert_eq!(
        e["jwkOpsDup"],
        serde_json::json!(
            "DataError: The \"key_ops\" member of the JWK dictionary contains duplicate usages."
        )
    );
    assert_eq!(
        e["jwkOpsConflict"],
        serde_json::json!(
            "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(
        e["jwkUseSig"],
        serde_json::json!(
            "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(
        e["usageBogus"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': Invalid keyUsages argument"
        )
    );
    assert_eq!(
        e["rawFormat"],
        serde_json::json!("NotSupportedError: Unsupported key format for RSA keys: raw")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_keygen_runs_off_the_js_thread() {
    // #69: RSA keygen (30-200ms of CPU each in a debug build) was a sync op,
    // so a few back-to-back generateKey calls ran as one continuous wall and
    // the patience watchdog beheaded the caller's chain. Off the JS thread,
    // twelve of them (well past patience in total) settle untouched.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const kinds = [];
        for (let i = 0; i < 12; i++) {
            const p = await crypto.subtle.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: 1024,
                publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['sign', 'verify']);
            kinds.push(p.privateKey.type);
        }
        return kinds.length;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(12));
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_pbkdf2_runs_off_the_js_thread() {
    // #69 follow-up: PBKDF2 is real CPU (OWASP's recommended 600k iterations
    // is ~2.7s in a debug build). As a sync op one derivation was one
    // continuous wall on the JS thread, and the eval patience watchdog
    // beheaded the caller. Off the thread it settles, bytes unchanged
    // (vector from Python's hashlib.pbkdf2_hmac).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder();
        const base = await S.importKey('raw', enc.encode('password'), 'PBKDF2', false, ['deriveBits']);
        const bits = await S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('salt'), iterations: 200000 }, base, 256);
        return Array.from(new Uint8Array(bits), x => x.toString(16).padStart(2, '0')).join('');
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!("ca64cfe28ca5559c62fba4afcb19f26889a67d5b135e571bffb087647e01becd")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_sign_runs_off_the_js_thread() {
    // #69 follow-up: an RSA private-key signature is tens of ms in a debug
    // build, and a JWT-style run of them on the JS thread chained into one
    // wall past patience — the caller got beheaded. Off the thread, 28
    // 2048-bit sign+verify rounds settle.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder();
        const pair = await S.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: 2048,
            publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['sign', 'verify']);
        let ok = 0;
        for (let i = 0; i < 28; i++) {
            const msg = enc.encode('jwt-' + i);
            const sig = await S.sign('RSASSA-PKCS1-v1_5', pair.privateKey, msg);
            if (await S.verify('RSASSA-PKCS1-v1_5', pair.publicKey, sig, msg)) ok++;
        }
        return ok;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(28));
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_oaep_runs_off_the_js_thread() {
    // #69 follow-up: OAEP decrypt is a private-key op (same cost class as
    // signing). A run of them on the JS thread chained past patience and the
    // caller got beheaded; off the thread, 20 2048-bit round trips settle
    // and wrap/unwrap (the other caller of the cipher core) still works.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder(), dec = new TextDecoder();
        const alg = { name: 'RSA-OAEP' };
        const pair = await S.generateKey({ name: 'RSA-OAEP', modulusLength: 2048,
            publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['encrypt', 'decrypt', 'wrapKey', 'unwrapKey']);
        let ok = 0;
        for (let i = 0; i < 20; i++) {
            const ct = await S.encrypt(alg, pair.publicKey, enc.encode('m' + i));
            if (dec.decode(await S.decrypt(alg, pair.privateKey, ct)) === 'm' + i) ok++;
        }
        const aes = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['encrypt']);
        const wrapped = await S.wrapKey('raw', aes, pair.publicKey, alg);
        const back = await S.unwrapKey('raw', wrapped, pair.privateKey, alg, 'AES-GCM', true, ['encrypt']);
        const same = (await S.exportKey('raw', back)).byteLength === 16;
        let bad;
        try { await S.decrypt(alg, pair.privateKey, new Uint8Array(256)); bad = 'no-throw'; }
        catch (e) { bad = e.name + ':' + e.message; }
        return { ok, same, bad };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({ "ok": 20, "same": true, "bad": "OperationError:" })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_aes_cipher_params_like_chrome() {
    // AES encrypt/decrypt parameters against Chrome 157, ciphertexts byte for
    // byte (deterministic keys/IVs): GCM with 0..32-byte IVs, every tag size
    // (32..128) plus invalid ones, AAD, tamper, too-short input, missing iv;
    // CBC IV sizes, padding failures; CTR counter sizes, counter widths
    // 1..128 including wrap-around and overflow; KW wrap/unwrap sizes.
    // Expected = Chrome 157's output for this exact script (headless,
    // file://). We used to take only 12-byte GCM IVs and 128-bit tags, only
    // 32/64/128-bit CTR counters, and spoke our own error texts.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const hex = (b) => Array.from(new Uint8Array(b), v => v.toString(16).padStart(2, '0')).join('');
  const key = async (name, n, us) => S.importKey('raw', bytes(n, 11), name, true, us || ['encrypt', 'decrypt']);
  const pt = bytes(37, 3), aad = bytes(9, 5);
  // GCM
  const g = await key('AES-GCM', 16);
  for (const n of [0, 1, 8, 12, 16, 32]) r['gcm.iv' + n] = await face(async () => { const c = await S.encrypt({ name: 'AES-GCM', iv: bytes(n, 7) }, g, pt); const p = await S.decrypt({ name: 'AES-GCM', iv: bytes(n, 7) }, g, c); return hex(c) + ':' + (hex(p) === hex(pt)); });
  for (const t of [32, 64, 96, 104, 112, 120, 128, 0, 40, 136]) r['gcm.tag' + t] = await face(async () => { const P = { name: 'AES-GCM', iv: bytes(12, 7), tagLength: t }; const c = await S.encrypt(P, g, pt); const p = await S.decrypt(P, g, c); return hex(c) + ':' + (hex(p) === hex(pt)); });
  r['gcm.aad'] = await face(async () => { const P = { name: 'AES-GCM', iv: bytes(12, 7), additionalData: aad }; const c = await S.encrypt(P, g, pt); return hex(c) + ':' + (hex(await S.decrypt(P, g, c)) === hex(pt)); });
  r['gcm.aadMismatch'] = await face(async () => { const c = await S.encrypt({ name: 'AES-GCM', iv: bytes(12, 7), additionalData: aad }, g, pt); return hex(await S.decrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, g, c)); });
  r['gcm.tamper'] = await face(async () => { const c = new Uint8Array(await S.encrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, g, pt)); c[3] ^= 1; return hex(await S.decrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, g, c)); });
  r['gcm.shortCt'] = await face(async () => hex(await S.decrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, g, bytes(15, 1))));
  r['gcm.shortCt96'] = await face(async () => hex(await S.decrypt({ name: 'AES-GCM', iv: bytes(12, 7), tagLength: 96 }, g, bytes(11, 1))));
  r['gcm.emptyPt'] = await face(async () => hex(await S.encrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, g, new Uint8Array(0))));
  r['gcm.noIv'] = await face(async () => hex(await S.encrypt({ name: 'AES-GCM' }, g, pt)));
  r['gcm.256'] = await face(async () => hex(await S.encrypt({ name: 'AES-GCM', iv: bytes(12, 7) }, await key('AES-GCM', 32), pt)));
  // CBC
  const b = await key('AES-CBC', 16);
  for (const n of [0, 15, 16, 17]) r['cbc.iv' + n] = await face(async () => { const c = await S.encrypt({ name: 'AES-CBC', iv: bytes(n, 2) }, b, pt); return hex(c) + ':' + (hex(await S.decrypt({ name: 'AES-CBC', iv: bytes(n, 2) }, b, c)) === hex(pt)); });
  r['cbc.emptyPt'] = await face(async () => hex(await S.encrypt({ name: 'AES-CBC', iv: bytes(16, 2) }, b, new Uint8Array(0))));
  r['cbc.blockPt'] = await face(async () => hex(await S.encrypt({ name: 'AES-CBC', iv: bytes(16, 2) }, b, bytes(32, 1))));
  r['cbc.decNotBlock'] = await face(async () => hex(await S.decrypt({ name: 'AES-CBC', iv: bytes(16, 2) }, b, bytes(31, 1))));
  r['cbc.decEmpty'] = await face(async () => hex(await S.decrypt({ name: 'AES-CBC', iv: bytes(16, 2) }, b, new Uint8Array(0))));
  r['cbc.badPad'] = await face(async () => hex(await S.decrypt({ name: 'AES-CBC', iv: bytes(16, 2) }, b, bytes(32, 1))));
  r['cbc.noIv'] = await face(async () => hex(await S.encrypt({ name: 'AES-CBC' }, b, pt)));
  // CTR
  const c = await key('AES-CTR', 16);
  for (const n of [0, 15, 16, 17]) r['ctr.counter' + n] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: bytes(n, 4), length: 64 }, c, pt)));
  for (const L of [1, 8, 64, 127, 128]) r['ctr.len' + L] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: bytes(16, 4), length: L }, c, pt)));
  const ff = new Uint8Array(16).fill(0xff);
  r['ctr.wrap8'] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: ff, length: 8 }, c, bytes(48, 1))));
  r['ctr.wrap128'] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: ff, length: 128 }, c, bytes(48, 1))));
  r['ctr.overflow2'] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: bytes(16, 4), length: 2 }, c, bytes(16 * 5, 1))));
  r['ctr.overflow1'] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', counter: bytes(16, 4), length: 1 }, c, bytes(16 * 3, 1))));
  r['ctr.noCounter'] = await face(async () => hex(await S.encrypt({ name: 'AES-CTR', length: 64 }, c, pt)));
  // KW
  const kw = await key('AES-KW', 16, ['wrapKey', 'unwrapKey']);
  for (const n of [8, 15, 16, 24, 32]) r['kw.wrap' + n] = await face(async () => { const k = await S.importKey('raw', bytes(n, 6), { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); const w = await S.wrapKey('raw', k, kw, 'AES-KW'); const u = await S.unwrapKey('raw', w, kw, 'AES-KW', { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); return hex(w) + ':' + hex(await S.exportKey('raw', u)); });
  r['kw.unwrapBad'] = await face(async () => { const u = await S.unwrapKey('raw', bytes(24, 1), kw, 'AES-KW', { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); return hex(await S.exportKey('raw', u)); });
  r['kw.unwrapShort'] = await face(async () => { const u = await S.unwrapKey('raw', bytes(16, 1), kw, 'AES-KW', { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); return 'unwrapped'; });
  r['kw.unwrapNot8'] = await face(async () => { const u = await S.unwrapKey('raw', bytes(25, 1), kw, 'AES-KW', { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); return 'unwrapped'; });
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "gcm.iv0": "OperationError: ",
 "gcm.iv1": "0ee174beae6a3f9f347cba3701a2bba900d12e79cbb66dc7c95b8d8506a917bc52b8c05fba80254b58e494d9c01dfcce996ac701e4:true",
 "gcm.iv8": "0c28539cb44d31c44e20fc892c2d135e141b0a425e3780c8a78329d8ed82e710b05b4316a8e110f452f60194993d2bf74400020854:true",
 "gcm.iv12": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a321102e5b6d5:true",
 "gcm.iv16": "b3df94acd52733fe0523b6fd9db67223f86459d5f6458ed2f69aab78de1cfbe259cf3be9afcc2025f717c7f7b98d28563f47e47865:true",
 "gcm.iv32": "19480a79d5df2eb51245146207ba23406a97b58b05bf5c3c979e7af7baccd9e9a62e954f8b71a391fead9dcc255d1cf3662b2316d4:true",
 "gcm.tag32": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e0:true",
 "gcm.tag64": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a1:true",
 "gcm.tag96": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a3211:true",
 "gcm.tag104": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a321102:true",
 "gcm.tag112": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a321102e5:true",
 "gcm.tag120": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a321102e5b6:true",
 "gcm.tag128": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d25a23443e04ae581a18f0a321102e5b6d5:true",
 "gcm.tag0": "OperationError: The tag length is invalid: Must be 32, 64, 96, 104, 112, 120, or 128 bits",
 "gcm.tag40": "OperationError: The tag length is invalid: Must be 32, 64, 96, 104, 112, 120, or 128 bits",
 "gcm.tag136": "OperationError: The tag length is invalid: Must be 32, 64, 96, 104, 112, 120, or 128 bits",
 "gcm.aad": "dd923f168541dc020198ebd309497af319d21a81c8a9b4197f9cd295cc14dd19af7eb45d255906e769180113aa03411c1baee437c0:true",
 "gcm.aadMismatch": "OperationError: ",
 "gcm.tamper": "OperationError: ",
 "gcm.shortCt": "OperationError: The provided data is too small",
 "gcm.shortCt96": "OperationError: The provided data is too small",
 "gcm.emptyPt": "188199675cd3bca345a36d6f2663144a",
 "gcm.noIv": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: iv: Missing required property",
 "gcm.256": "2c81e831e61b3227dbc1d7315d5d05b547402ce24c88418574bc6a2e9c12ae87b5c6365f74da84f053b901bb72799809e1ea327092",
 "cbc.iv0": "OperationError: The \"iv\" has an unexpected length -- must be 16 bytes",
 "cbc.iv15": "OperationError: The \"iv\" has an unexpected length -- must be 16 bytes",
 "cbc.iv16": "f3c3afc7b82c07ad09f7f628e138c213ac43f6cf5401a6386f875f2cdbf3a25b1fc5490009333728389bcf748548e33d:true",
 "cbc.iv17": "OperationError: The \"iv\" has an unexpected length -- must be 16 bytes",
 "cbc.emptyPt": "d4979466e490e5f5bcc0c487ccb0f7ea",
 "cbc.blockPt": "5d160e67f214f9c388703e743c4f0ead4bd4c6dc3d432f037a8d240db8da6f06041835fb8455b919ad3d19cb30efea53",
 "cbc.decNotBlock": "OperationError: ",
 "cbc.decEmpty": "OperationError: ",
 "cbc.badPad": "OperationError: ",
 "cbc.noIv": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCbcParams: iv: Missing required property",
 "ctr.counter0": "OperationError: The \"counter\" has an unexpected length -- must be 16 bytes",
 "ctr.counter15": "OperationError: The \"counter\" has an unexpected length -- must be 16 bytes",
 "ctr.counter16": "5dac03e194412eb922cf00950c0571b56d8614519732ef694e955fc2564a97ed8d6ee1632d",
 "ctr.counter17": "OperationError: The \"counter\" has an unexpected length -- must be 16 bytes",
 "ctr.len1": "DataError: The input is too large for the counter length.",
 "ctr.len8": "5dac03e194412eb922cf00950c0571b56d8614519732ef694e955fc2564a97ed8d6ee1632d",
 "ctr.len64": "5dac03e194412eb922cf00950c0571b56d8614519732ef694e955fc2564a97ed8d6ee1632d",
 "ctr.len127": "5dac03e194412eb922cf00950c0571b56d8614519732ef694e955fc2564a97ed8d6ee1632d",
 "ctr.len128": "5dac03e194412eb922cf00950c0571b56d8614519732ef694e955fc2564a97ed8d6ee1632d",
 "ctr.wrap8": "bbb89f0a415836d320b98c3e04fda16e99711dfa391bef96f3f1f324bfa8aa6a9e2bbd063a1f5a094ee381e0a2142536",
 "ctr.wrap128": "bbb89f0a415836d320b98c3e04fda16e00bd335c335efbcc97b08fd3fae3a57a1a7b14c29fb969d19536b344b8b6e215",
 "ctr.overflow2": "DataError: The input is too large for the counter length.",
 "ctr.overflow1": "DataError: The input is too large for the counter length.",
 "ctr.noCounter": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: counter: Missing required property",
 "kw.wrap8": "OperationError: The provided data is too small",
 "kw.wrap15": "OperationError: The provided data is too small",
 "kw.wrap16": "b9c4049a1f997a40b928302e2a324c3ae295f280823d08f0:062b50759abfe4092e53789dc2e70c31",
 "kw.wrap24": "dbe2470b7d58383effca56c1015cd81d89e62a4355c200797f4357d4b252351f:062b50759abfe4092e53789dc2e70c31567ba0c5ea0f3459",
 "kw.wrap32": "378d7aa2876d64d0b089fd88e8153a6f61f41f0f4fa4a10aa6deb79056e86601a540dff26e0e0d83:062b50759abfe4092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81",
 "kw.unwrapBad": "OperationError: ",
 "kw.unwrapShort": "OperationError: The provided data is too small",
 "kw.unwrapNot8": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes"
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_hmac_aes_import_like_chrome() {
    // HMAC and AES (GCM/CBC/CTR/KW) raw + jwk imports: raw sizes 0..64, jwk
    // alg of the right mode at the right/wrong size and of another mode, 24-
    // byte k with and without A192*, kty, missing/empty/padded/standard-
    // base64 k, use, key_ops, ext, wrong and empty usages, export shape; HMAC
    // raw 0/1/64/200 bytes (each signs once), missing / unknown hash, jwk
    // HS1..HS512 against the hash. Expected = Chrome 157's output for this
    // exact script (headless, file://). We took an empty HMAC key, read k ""
    // as a base64 failure, said "alg inconsistent" for a right-mode alg of
    // the wrong size, and kept Chrome 155's DataError for a 192-bit jwk.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const b64u = (b) => btoa(String.fromCharCode(...b)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  const AES = { 'AES-GCM': ['encrypt', 'decrypt'], 'AES-CBC': ['encrypt', 'decrypt'], 'AES-CTR': ['encrypt', 'decrypt'], 'AES-KW': ['wrapKey', 'unwrapKey'] };
  const SUF = { 'AES-GCM': 'GCM', 'AES-CBC': 'CBC', 'AES-CTR': 'CTR', 'AES-KW': 'KW' };
  const desc = (k) => k.type + ':' + k.algorithm.name + ':' + k.algorithm.length + ':' + k.usages.join(',');
  for (const [name, us] of Object.entries(AES)) {
    for (const n of [0, 15, 16, 17, 24, 32, 64]) r[name + '.raw' + n] = await face(async () => desc(await S.importKey('raw', bytes(n, 1), name, true, us)));
    const k16 = b64u(bytes(16, 3)), k32 = b64u(bytes(32, 3)), k24 = b64u(bytes(24, 3));
    const J = (o) => Object.assign({ kty: 'oct', k: k16 }, o);
    const jw = {
      ok: J({}), alg128: J({ alg: 'A128' + SUF[name] }), alg256on16: J({ alg: 'A256' + SUF[name] }), alg256: J({ k: k32, alg: 'A256' + SUF[name] }),
      algOther: J({ alg: name === 'AES-GCM' ? 'A128CBC' : 'A128GCM' }), algBogus: J({ alg: 'HS256' }), len24: J({ k: k24 }), len24alg: J({ k: k24, alg: 'A192' + SUF[name] }),
      ktyRSA: J({ kty: 'RSA' }), noK: (() => { const o = J({}); delete o.k; return o; })(), kEmpty: J({ k: '' }), kPadded: J({ k: k16 + '==' }), kStd: J({ k: btoa(String.fromCharCode(...bytes(16, 250))) }),
      useEnc: J({ use: 'enc' }), useSig: J({ use: 'sig' }), keyOpsOk: J({ key_ops: us }), keyOpsMiss: J({ key_ops: [us[0]] }), extFalse: J({ ext: false }),
    };
    for (const [k, j] of Object.entries(jw)) r[name + '.jwk.' + k] = await face(async () => desc(await S.importKey('jwk', j, name, true, us)));
    r[name + '.badUsage'] = await face(async () => desc(await S.importKey('raw', bytes(16, 1), name, true, name === 'AES-KW' ? ['encrypt'] : ['sign'])));
    r[name + '.emptyUsage'] = await face(async () => desc(await S.importKey('raw', bytes(16, 1), name, true, [])));
    r[name + '.export'] = await face(async () => { const k = await S.importKey('raw', bytes(32, 9), name, true, us); const j = await S.exportKey('jwk', k); return JSON.stringify([Object.keys(j), j.alg, j.k === b64u(bytes(32, 9))]) + ':' + (await S.exportKey('raw', k)).byteLength; });
  }
  const H = (h, extra) => Object.assign({ name: 'HMAC', hash: h }, extra || {});
  const hdesc = (k) => k.type + ':' + k.algorithm.hash.name + ':' + k.algorithm.length + ':' + k.usages.join(',');
  const use = async (k) => (await S.sign('HMAC', k, enc.encode('m'))).byteLength;
  for (const n of [0, 1, 64, 200]) r['HMAC.raw' + n] = await face(async () => { const k = await S.importKey('raw', bytes(n, 5), H('SHA-256'), true, ['sign']); return hdesc(k) + ':' + (await use(k)); });
  r['HMAC.noHash'] = await face(async () => hdesc(await S.importKey('raw', bytes(16, 5), { name: 'HMAC' }, true, ['sign'])));
  r['HMAC.hashBogus'] = await face(async () => hdesc(await S.importKey('raw', bytes(16, 5), H('SHA-3'), true, ['sign'])));
  const hk = b64u(bytes(32, 7));
  const HJ = (o) => Object.assign({ kty: 'oct', k: hk }, o);
  const hj = {
    ok: HJ({}), hs256: HJ({ alg: 'HS256' }), hs384: HJ({ alg: 'HS384' }), hs1: HJ({ alg: 'HS1' }), algA128: HJ({ alg: 'A128GCM' }), ktyEC: HJ({ kty: 'EC' }), noK: { kty: 'oct' },
    kEmpty: HJ({ k: '' }), kPadded: HJ({ k: hk + '=' }), useEnc: HJ({ use: 'enc' }), useSig: HJ({ use: 'sig' }), keyOpsMiss: HJ({ key_ops: ['verify'] }), extFalse: HJ({ ext: false }),
  };
  for (const [k, j] of Object.entries(hj)) r['HMAC.jwk.' + k] = await face(async () => { const key = await S.importKey('jwk', j, H('SHA-256'), true, ['sign']); return hdesc(key) + ':' + (await use(key)); });
  r['HMAC.jwkEmptyK.len0'] = await face(async () => hdesc(await S.importKey('jwk', HJ({ k: '' }), H('SHA-256', { length: 0 }), true, ['sign'])));
  r['HMAC.badUsage'] = await face(async () => hdesc(await S.importKey('raw', bytes(16, 5), H('SHA-256'), true, ['encrypt'])));
  r['HMAC.emptyUsage'] = await face(async () => hdesc(await S.importKey('raw', bytes(16, 5), H('SHA-256'), true, [])));
  r['HMAC.export'] = await face(async () => { const k = await S.importKey('raw', bytes(20, 9), H('SHA-384'), true, ['sign', 'verify']); const j = await S.exportKey('jwk', k); return JSON.stringify([Object.keys(j), j.alg, j.key_ops]) + ':' + (await S.exportKey('raw', k)).byteLength; });
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "AES-GCM.raw0": "DataError: AES key data must be 128 or 256 bits",
 "AES-GCM.raw15": "DataError: AES key data must be 128 or 256 bits",
 "AES-GCM.raw16": "secret:AES-GCM:128:encrypt,decrypt",
 "AES-GCM.raw17": "DataError: AES key data must be 128 or 256 bits",
 "AES-GCM.raw24": "OperationError: 192-bit AES keys are not supported",
 "AES-GCM.raw32": "secret:AES-GCM:256:encrypt,decrypt",
 "AES-GCM.raw64": "DataError: AES key data must be 128 or 256 bits",
 "AES-GCM.jwk.ok": "secret:AES-GCM:128:encrypt,decrypt",
 "AES-GCM.jwk.alg128": "secret:AES-GCM:128:encrypt,decrypt",
 "AES-GCM.jwk.alg256on16": "DataError: The JWK \"k\" member did not include the right length of key data for the given algorithm.",
 "AES-GCM.jwk.alg256": "secret:AES-GCM:256:encrypt,decrypt",
 "AES-GCM.jwk.algOther": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-GCM.jwk.algBogus": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-GCM.jwk.len24": "OperationError: 192-bit AES keys are not supported",
 "AES-GCM.jwk.len24alg": "OperationError: 192-bit AES keys are not supported",
 "AES-GCM.jwk.ktyRSA": "DataError: The JWK \"kty\" member was not \"oct\"",
 "AES-GCM.jwk.noK": "DataError: The required JWK member \"k\" was missing",
 "AES-GCM.jwk.kEmpty": "DataError: AES key data must be 128 or 256 bits",
 "AES-GCM.jwk.kPadded": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-GCM.jwk.kStd": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-GCM.jwk.useEnc": "secret:AES-GCM:128:encrypt,decrypt",
 "AES-GCM.jwk.useSig": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-GCM.jwk.keyOpsOk": "secret:AES-GCM:128:encrypt,decrypt",
 "AES-GCM.jwk.keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-GCM.jwk.extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "AES-GCM.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "AES-GCM.emptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "AES-GCM.export": "[[\"alg\",\"ext\",\"k\",\"key_ops\",\"kty\"],\"A256GCM\",true]:32",
 "AES-CBC.raw0": "DataError: AES key data must be 128 or 256 bits",
 "AES-CBC.raw15": "DataError: AES key data must be 128 or 256 bits",
 "AES-CBC.raw16": "secret:AES-CBC:128:encrypt,decrypt",
 "AES-CBC.raw17": "DataError: AES key data must be 128 or 256 bits",
 "AES-CBC.raw24": "OperationError: 192-bit AES keys are not supported",
 "AES-CBC.raw32": "secret:AES-CBC:256:encrypt,decrypt",
 "AES-CBC.raw64": "DataError: AES key data must be 128 or 256 bits",
 "AES-CBC.jwk.ok": "secret:AES-CBC:128:encrypt,decrypt",
 "AES-CBC.jwk.alg128": "secret:AES-CBC:128:encrypt,decrypt",
 "AES-CBC.jwk.alg256on16": "DataError: The JWK \"k\" member did not include the right length of key data for the given algorithm.",
 "AES-CBC.jwk.alg256": "secret:AES-CBC:256:encrypt,decrypt",
 "AES-CBC.jwk.algOther": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-CBC.jwk.algBogus": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-CBC.jwk.len24": "OperationError: 192-bit AES keys are not supported",
 "AES-CBC.jwk.len24alg": "OperationError: 192-bit AES keys are not supported",
 "AES-CBC.jwk.ktyRSA": "DataError: The JWK \"kty\" member was not \"oct\"",
 "AES-CBC.jwk.noK": "DataError: The required JWK member \"k\" was missing",
 "AES-CBC.jwk.kEmpty": "DataError: AES key data must be 128 or 256 bits",
 "AES-CBC.jwk.kPadded": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-CBC.jwk.kStd": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-CBC.jwk.useEnc": "secret:AES-CBC:128:encrypt,decrypt",
 "AES-CBC.jwk.useSig": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-CBC.jwk.keyOpsOk": "secret:AES-CBC:128:encrypt,decrypt",
 "AES-CBC.jwk.keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-CBC.jwk.extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "AES-CBC.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "AES-CBC.emptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "AES-CBC.export": "[[\"alg\",\"ext\",\"k\",\"key_ops\",\"kty\"],\"A256CBC\",true]:32",
 "AES-CTR.raw0": "DataError: AES key data must be 128 or 256 bits",
 "AES-CTR.raw15": "DataError: AES key data must be 128 or 256 bits",
 "AES-CTR.raw16": "secret:AES-CTR:128:encrypt,decrypt",
 "AES-CTR.raw17": "DataError: AES key data must be 128 or 256 bits",
 "AES-CTR.raw24": "OperationError: 192-bit AES keys are not supported",
 "AES-CTR.raw32": "secret:AES-CTR:256:encrypt,decrypt",
 "AES-CTR.raw64": "DataError: AES key data must be 128 or 256 bits",
 "AES-CTR.jwk.ok": "secret:AES-CTR:128:encrypt,decrypt",
 "AES-CTR.jwk.alg128": "secret:AES-CTR:128:encrypt,decrypt",
 "AES-CTR.jwk.alg256on16": "DataError: The JWK \"k\" member did not include the right length of key data for the given algorithm.",
 "AES-CTR.jwk.alg256": "secret:AES-CTR:256:encrypt,decrypt",
 "AES-CTR.jwk.algOther": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-CTR.jwk.algBogus": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-CTR.jwk.len24": "OperationError: 192-bit AES keys are not supported",
 "AES-CTR.jwk.len24alg": "OperationError: 192-bit AES keys are not supported",
 "AES-CTR.jwk.ktyRSA": "DataError: The JWK \"kty\" member was not \"oct\"",
 "AES-CTR.jwk.noK": "DataError: The required JWK member \"k\" was missing",
 "AES-CTR.jwk.kEmpty": "DataError: AES key data must be 128 or 256 bits",
 "AES-CTR.jwk.kPadded": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-CTR.jwk.kStd": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-CTR.jwk.useEnc": "secret:AES-CTR:128:encrypt,decrypt",
 "AES-CTR.jwk.useSig": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-CTR.jwk.keyOpsOk": "secret:AES-CTR:128:encrypt,decrypt",
 "AES-CTR.jwk.keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-CTR.jwk.extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "AES-CTR.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "AES-CTR.emptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "AES-CTR.export": "[[\"alg\",\"ext\",\"k\",\"key_ops\",\"kty\"],\"A256CTR\",true]:32",
 "AES-KW.raw0": "DataError: AES key data must be 128 or 256 bits",
 "AES-KW.raw15": "DataError: AES key data must be 128 or 256 bits",
 "AES-KW.raw16": "secret:AES-KW:128:wrapKey,unwrapKey",
 "AES-KW.raw17": "DataError: AES key data must be 128 or 256 bits",
 "AES-KW.raw24": "OperationError: 192-bit AES keys are not supported",
 "AES-KW.raw32": "secret:AES-KW:256:wrapKey,unwrapKey",
 "AES-KW.raw64": "DataError: AES key data must be 128 or 256 bits",
 "AES-KW.jwk.ok": "secret:AES-KW:128:wrapKey,unwrapKey",
 "AES-KW.jwk.alg128": "secret:AES-KW:128:wrapKey,unwrapKey",
 "AES-KW.jwk.alg256on16": "DataError: The JWK \"k\" member did not include the right length of key data for the given algorithm.",
 "AES-KW.jwk.alg256": "secret:AES-KW:256:wrapKey,unwrapKey",
 "AES-KW.jwk.algOther": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-KW.jwk.algBogus": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "AES-KW.jwk.len24": "OperationError: 192-bit AES keys are not supported",
 "AES-KW.jwk.len24alg": "OperationError: 192-bit AES keys are not supported",
 "AES-KW.jwk.ktyRSA": "DataError: The JWK \"kty\" member was not \"oct\"",
 "AES-KW.jwk.noK": "DataError: The required JWK member \"k\" was missing",
 "AES-KW.jwk.kEmpty": "DataError: AES key data must be 128 or 256 bits",
 "AES-KW.jwk.kPadded": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-KW.jwk.kStd": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "AES-KW.jwk.useEnc": "secret:AES-KW:128:wrapKey,unwrapKey",
 "AES-KW.jwk.useSig": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-KW.jwk.keyOpsOk": "secret:AES-KW:128:wrapKey,unwrapKey",
 "AES-KW.jwk.keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "AES-KW.jwk.extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "AES-KW.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "AES-KW.emptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "AES-KW.export": "[[\"alg\",\"ext\",\"k\",\"key_ops\",\"kty\"],\"A256KW\",true]:32",
 "HMAC.raw0": "DataError: HMAC key data must not be empty",
 "HMAC.raw1": "secret:SHA-256:8:sign:32",
 "HMAC.raw64": "secret:SHA-256:512:sign:32",
 "HMAC.raw200": "secret:SHA-256:1600:sign:32",
 "HMAC.noHash": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Missing or not an AlgorithmIdentifier",
 "HMAC.hashBogus": "NotSupportedError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Algorithm: Unrecognized name",
 "HMAC.jwk.ok": "secret:SHA-256:256:sign:32",
 "HMAC.jwk.hs256": "secret:SHA-256:256:sign:32",
 "HMAC.jwk.hs384": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "HMAC.jwk.hs1": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "HMAC.jwk.algA128": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "HMAC.jwk.ktyEC": "DataError: The JWK \"kty\" member was not \"oct\"",
 "HMAC.jwk.noK": "DataError: The required JWK member \"k\" was missing",
 "HMAC.jwk.kEmpty": "DataError: HMAC key data must not be empty",
 "HMAC.jwk.kPadded": "DataError: The JWK member \"k\" could not be base64url decoded or contained padding",
 "HMAC.jwk.useEnc": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "HMAC.jwk.useSig": "secret:SHA-256:256:sign:32",
 "HMAC.jwk.keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "HMAC.jwk.extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "HMAC.jwkEmptyK.len0": "DataError: HMAC key data must not be empty",
 "HMAC.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "HMAC.emptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "HMAC.export": "[[\"alg\",\"ext\",\"k\",\"key_ops\",\"kty\"],\"HS384\",[\"sign\",\"verify\"]]:20"
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_ec_p521_like_chrome() {
    // P-521 (p521 joins p256/p384 — same RustCrypto elliptic-curve 0.14 line,
    // one new crate). The EC import matrix again on P-521 with P-256 as the
    // other curve, expected = Chrome 157's output for this exact script
    // (headless, file://), including the ECDH secret of the matrix's own key
    // pair byte for byte. A signature Chrome made with the fixed key must
    // verify here (ours was checked to verify in Chrome when this landed),
    // and keygen round-trips: 132-byte signatures, 66-byte jwk members,
    // 528-bit ECDH. We used to refuse P-521 outright (Chrome supports it).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const D = {"v": {"raw": {"ok": "BAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "compressed": "AgB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBg==", "short": "BAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0", "long": "BAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aAA=", "prefix05": "BQB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "offCurve": "BAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aQ==", "xGeP": "BAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "infinity": "AA==", "empty": "", "otherPoint": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0="}, "spki": {"ok": "MIGbMBAGByqGSM49AgEGBSuBBAAjA4GGAAQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOUegsTdGg=", "garbage": "CwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5P", "truncated": "MIGbMBAGByqGSM49AgEGBSuBBAAjA4GGAAQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOU", "trailing": "MIGbMBAGByqGSM49AgEGBSuBBAAjA4GGAAQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOUegsTdGgA", "curveOther": "MIGeMBMGByqGSM49AgEGCCqGSM49AwEHA4GGAAQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOUegsTdGg=", "noCurve": "MIGUMAkGByqGSM49AgEDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "unusedBits": "MIGbMBAGByqGSM49AgEGBSuBBAAjA4GGAQQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOUegsTdGg=", "offCurve": "MIGbMBAGByqGSM49AgEGBSuBBAAjA4GGAAQAfvlRCZSb9kdOfkVAzq3ZUI/kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G/vS5CVTytE3sRIDAkg7p/BPvebwYB7446lYACS3dcPRsbXkvCLtGq5bC4BxODvinoS+Rfu8Y0HmHWUL6J7NhUWW0RQXnIK59d/wb6te5d3wOUegsTdGk=", "compressed": "MFgwEAYHKoZIzj0CAQYFK4EEACMDRAACAH75UQmUm/ZHTn5FQM6t2VCP5GK+0VxLd7rowdYkBE3JOrCs1GpVpX7pD/dxv70uQlU8rRN7ESAwJIO6fwT73m8G", "ecdhOid": "MIGZMA4GBSuBBAEMBgUrgQQAIwOBhgAEAH75UQmUm/ZHTn5FQM6t2VCP5GK+0VxLd7rowdYkBE3JOrCs1GpVpX7pD/dxv70uQlU8rRN7ESAwJIO6fwT73m8GAe+OOpWAAkt3XD0bG15Lwi7RquWwuAcTg74p6EvkX7vGNB5h1lC+iezYVFltEUF5yCufXf8G+rXuXd8DlHoLE3Ro", "otherKey": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQ=="}, "pkcs8": {"ok": "MIHuAgEAMBAGByqGSM49AgEGBSuBBAAjBIHWMIHTAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52hgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "noPublic": "MGACAQAwEAYHKoZIzj0CAQYFK4EEACMESTBHAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee50=", "innerCurve": "MIH3AgEAMBAGByqGSM49AgEGBSuBBAAjBIHfMIHcAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52gBwYFK4EEACOhgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "innerCurveMismatch": "MIH6AgEAMBAGByqGSM49AgEGBSuBBAAjBIHiMIHfAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52gCgYIKoZIzj0DAQehgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "outerOther": "MIHxAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBIHWMIHTAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52hgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aA==", "scalarZero": "MGACAQAwEAYHKoZIzj0CAQYFK4EEACMESTBHAgEBBEIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "scalarN": "MGACAQAwEAYHKoZIzj0CAQYFK4EEACMESTBHAgEBBEIB///////////////////////////////////////////6UYaHg78vlmt/zAFI9wml0Du1ybiJnEeuu2+3HpE4ZAk=", "scalarShort": "MF8CAQAwEAYHKoZIzj0CAQYFK4EEACMESDBGAgEBBEEAABs9X3qcHjtdf5ocPlt9nxo8XnudHzpcfpsdP1p8nhs9XypMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nQ==", "scalarLong": "MGECAQAwEAYHKoZIzj0CAQYFK4EEACMESjBIAgEBBEMAAAAAGz1fepweO11/mhw+W32fGjxee50fOlx+mx0/WnyeGz1fKkxujwsdP1p8nhs9X3qcHjtdf5ocPlt9nxo8Xnud", "pubMismatch": "MIHuAgEAMBAGByqGSM49AgEGBSuBBAAjBIHWMIHTAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52hgYkDgYYABAAHbKH6lhOnDoss7Nj2SHeWjjK5GxbNZRZspEsUOYg6kZ4mPAnP8vfznncOvakFfS8RmIBKHBYPC7meua5vqyj3iwCzEh/lT46D2b3qZD6PegH7bgeR1NRZrFuh6kNLh9LWudZHal9SHuD2uN485PNHRmAjX4gkDKfQ3bpZoH1JNpK9aw==", "pubOffCurve": "MIHuAgEAMBAGByqGSM49AgEGBSuBBAAjBIHWMIHTAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52hgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aQ==", "garbage": "AwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZ", "trailing": "MIHuAgEAMBAGByqGSM49AgEGBSuBBAAjBIHWMIHTAgEBBEIAAAAbPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee52hgYkDgYYABAB++VEJlJv2R05+RUDOrdlQj+RivtFcS3e66MHWJARNyTqwrNRqVaV+6Q/3cb+9LkJVPK0TexEgMCSDun8E+95vBgHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O+KehL5F+7xjQeYdZQvons2FRZbRFBecgrn13/Bvq17l3fA5R6CxN0aAA=", "otherKey": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN"}}, "j": {"x": "AH75UQmUm_ZHTn5FQM6t2VCP5GK-0VxLd7rowdYkBE3JOrCs1GpVpX7pD_dxv70uQlU8rRN7ESAwJIO6fwT73m8G", "y": "Ae-OOpWAAkt3XD0bG15Lwi7RquWwuAcTg74p6EvkX7vGNB5h1lC-iezYVFltEUF5yCufXf8G-rXuXd8DlHoLE3Ro", "d": "AAAAGz1fepweO11_mhw-W32fGjxee50fOlx-mx0_WnyeGz1fKkxujwsdP1p8nhs9X3qcHjtdf5ocPlt9nxo8Xnud", "x3": "3ekkyaO3PdweD_LJdGppCjgug4xpSwnfzlxsZ8mUckc", "y3": "J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0", "d3": "HTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4", "xShort": "fvlRCZSb9kdOfkVAzq3ZUI_kYr7RXEt3uujB1iQETck6sKzUalWlfukP93G_vS5CVTytE3sRIDAkg7p_BPvebwY", "yLong": "AAHvjjqVgAJLd1w9GxteS8Iu0arlsLgHE4O-KehL5F-7xjQeYdZQvons2FRZbRFBecgrn13_Bvq17l3fA5R6CxN0aA", "yOff": "Ae-OOpWAAkt3XD0bG15Lwi7RquWwuAcTg74p6EvkX7vGNB5h1lC-iezYVFltEUF5yCufXf8G-rXuXd8DlHoLE3Rp", "dZero": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "dShort": "AAAbPV96nB47XX-aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8qTG6PCx0_WnyeGz1fepweO11_mhw-W32fGjxee50", "dOther": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADA5"}};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const msg = enc.encode('diting-ec');
  const EC = (name) => ({ name, namedCurve: 'P-521' });
  const SIG = { name: 'ECDSA', hash: 'SHA-512' };
  const okPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDSA'), true, ['sign']);
  const okPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDSA'), true, ['verify']);
  const okSig = await S.sign(SIG, okPriv, msg);
  const dhPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDH'), true, ['deriveBits']);
  const dhPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDH'), true, []);
  const usePub = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, k, okSig, msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: k }, dhPriv, 528)).byteLength;
  const usePriv = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, okPub, await S.sign(SIG, k, msg), msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: dhPub }, k, 528)).byteLength;
  for (const name of ['ECDSA', 'ECDH']) {
    const pubUs = name === 'ECDSA' ? ['verify'] : [], privUs = name === 'ECDSA' ? ['sign'] : ['deriveBits'];
    for (const fmt of ['raw', 'spki']) for (const [k, v] of Object.entries(D.v[fmt])) {
      r[name + '.' + fmt + '.' + k] = await face(async () => { const key = await S.importKey(fmt, b64(v), EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    }
    for (const [k, v] of Object.entries(D.v.pkcs8)) {
      r[name + '.pkcs8.' + k] = await face(async () => { const key = await S.importKey('pkcs8', b64(v), EC(name), true, privUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePriv(key, name)); });
    }
    const J = D.j, P = { kty: 'EC', crv: 'P-521', x: J.x, y: J.y };
    const W = (o, e) => Object.assign({}, o, e), drop = (o, m) => { const c = Object.assign({}, o); delete c[m]; return c; };
    const pubJ = { ok: P, crvOther: W(P, { crv: 'P-256' }), crvBogus: W(P, { crv: 'P-999' }), noCrv: drop(P, 'crv'), noX: drop(P, 'x'), noY: drop(P, 'y'),
      xShort: W(P, { x: J.xShort }), yLong: W(P, { y: J.yLong }), offCurve: W(P, { y: J.yOff }), ktyRSA: W(P, { kty: 'RSA' }), otherCoords: W(P, { x: J.x3, y: J.y3 }),
      algES256: W(P, { alg: 'ES256' }), algES384: W(P, { alg: 'ES384' }), useEnc: W(P, { use: 'enc' }), useSig: W(P, { use: 'sig' }), xEmpty: W(P, { x: '' }), xPadded: W(P, { x: J.x + '=' }) };
    for (const [k, j] of Object.entries(pubJ)) r[name + '.jwkPub.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    const Q = W(P, { d: J.d });
    const privJ = { ok: Q, dZero: W(Q, { d: J.dZero }), dShort: W(Q, { d: J.dShort }), dMismatch: W(Q, { d: J.dOther }), noD: drop(Q, 'd'), dEmpty: W(Q, { d: '' }), offCurve: W(Q, { y: J.yOff }) };
    for (const [k, j] of Object.entries(privJ)) r[name + '.jwkPriv.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, privUs); return 'ok:' + key.type + ':' + (await (key.type === 'private' ? usePriv(key, name) : usePub(key, name))); });
  }
  const unhexLen = (s) => atob(s.replace(/-/g, '+').replace(/_/g, '/')).length;
  const hex = (x) => Array.from(new Uint8Array(x), v => v.toString(16).padStart(2, '0')).join('');
  r['interop.dh'] = hex(await S.deriveBits({ name: 'ECDH', public: dhPub }, dhPriv, 528));
  const unhex = (h) => Uint8Array.from(h.match(/../g), v => parseInt(v, 16));
  // Chrome 157's own P-521 signature over msg with this key.
  r['interop.chromeSigVerifies'] = String(await S.verify(SIG, okPub, unhex("00756c9eb98e0bc1d03e0899a39b0ee29ea49bf16037b33fc0c4e9b5323c8c2669478cc763c8b5384f1427a1caef0afb3c4f95302ff4da6d5351b4233fc62ae21a48015de37bf0d333d6c7d7df30f4ecaa57300caf8966b12fc529776d7703ce2b4fa3d74df6bdaa5bb8e31590eb307a031413d33c58856271834feddd6e56f5904d20fd"), msg));
  const g = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-521' }, true, ['sign', 'verify']);
  const gs = await S.sign(SIG, g.privateKey, msg);
  const gj = await S.exportKey('jwk', g.privateKey);
  r['gen.ecdsa'] = gs.byteLength + ':' + (await S.verify(SIG, g.publicKey, gs, msg)) + ':' + gj.crv + ':' + unhexLen(gj.x) + ':' + unhexLen(gj.d);
  const h = await S.generateKey({ name: 'ECDH', namedCurve: 'P-521' }, true, ['deriveBits']);
  r['gen.ecdh'] = String((await S.deriveBits({ name: 'ECDH', public: h.publicKey }, h.privateKey, 528)).byteLength);
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "ECDSA.raw.ok": "ok:P-521:verify=true",
 "ECDSA.raw.compressed": "ok:P-521:verify=true",
 "ECDSA.raw.short": "DataError: ",
 "ECDSA.raw.long": "DataError: ",
 "ECDSA.raw.prefix05": "DataError: ",
 "ECDSA.raw.offCurve": "DataError: ",
 "ECDSA.raw.xGeP": "DataError: ",
 "ECDSA.raw.infinity": "DataError: ",
 "ECDSA.raw.empty": "DataError: ",
 "ECDSA.raw.otherPoint": "DataError: ",
 "ECDSA.spki.ok": "ok:P-521:verify=true",
 "ECDSA.spki.garbage": "DataError: ",
 "ECDSA.spki.truncated": "DataError: ",
 "ECDSA.spki.trailing": "DataError: ",
 "ECDSA.spki.curveOther": "DataError: ",
 "ECDSA.spki.noCurve": "DataError: ",
 "ECDSA.spki.unusedBits": "DataError: ",
 "ECDSA.spki.offCurve": "DataError: ",
 "ECDSA.spki.compressed": "ok:P-521:verify=true",
 "ECDSA.spki.ecdhOid": "DataError: ",
 "ECDSA.spki.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.pkcs8.ok": "ok:P-521:verify=true",
 "ECDSA.pkcs8.noPublic": "ok:P-521:verify=true",
 "ECDSA.pkcs8.innerCurve": "ok:P-521:verify=true",
 "ECDSA.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDSA.pkcs8.outerOther": "DataError: ",
 "ECDSA.pkcs8.scalarZero": "DataError: ",
 "ECDSA.pkcs8.scalarN": "DataError: ",
 "ECDSA.pkcs8.scalarShort": "ok:P-521:verify=true",
 "ECDSA.pkcs8.scalarLong": "ok:P-521:verify=true",
 "ECDSA.pkcs8.pubMismatch": "DataError: ",
 "ECDSA.pkcs8.pubOffCurve": "DataError: ",
 "ECDSA.pkcs8.garbage": "DataError: ",
 "ECDSA.pkcs8.trailing": "DataError: ",
 "ECDSA.pkcs8.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.jwkPub.ok": "ok:P-521:verify=true",
 "ECDSA.jwkPub.crvOther": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDSA.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDSA.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDSA.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 65 bytes but should be 66",
 "ECDSA.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 67 bytes but should be 66",
 "ECDSA.jwkPub.offCurve": "DataError: ",
 "ECDSA.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDSA.jwkPub.otherCoords": "DataError: The JWK's \"x\" member defines an octet string of length 32 bytes but should be 66",
 "ECDSA.jwkPub.algES256": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "ECDSA.jwkPub.algES384": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "ECDSA.jwkPub.useEnc": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "ECDSA.jwkPub.useSig": "ok:P-521:verify=true",
 "ECDSA.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 66",
 "ECDSA.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDSA.jwkPriv.ok": "ok:private:verify=true",
 "ECDSA.jwkPriv.dZero": "OperationError: ",
 "ECDSA.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 65 bytes but should be 66",
 "ECDSA.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDSA.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDSA.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 66",
 "ECDSA.jwkPriv.offCurve": "DataError: ",
 "ECDH.raw.ok": "ok:P-521:derive=66",
 "ECDH.raw.compressed": "ok:P-521:derive=66",
 "ECDH.raw.short": "DataError: ",
 "ECDH.raw.long": "DataError: ",
 "ECDH.raw.prefix05": "DataError: ",
 "ECDH.raw.offCurve": "DataError: ",
 "ECDH.raw.xGeP": "DataError: ",
 "ECDH.raw.infinity": "DataError: ",
 "ECDH.raw.empty": "DataError: ",
 "ECDH.raw.otherPoint": "DataError: ",
 "ECDH.spki.ok": "ok:P-521:derive=66",
 "ECDH.spki.garbage": "DataError: ",
 "ECDH.spki.truncated": "DataError: ",
 "ECDH.spki.trailing": "DataError: ",
 "ECDH.spki.curveOther": "DataError: ",
 "ECDH.spki.noCurve": "DataError: ",
 "ECDH.spki.unusedBits": "DataError: ",
 "ECDH.spki.offCurve": "DataError: ",
 "ECDH.spki.compressed": "ok:P-521:derive=66",
 "ECDH.spki.ecdhOid": "DataError: ",
 "ECDH.spki.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.pkcs8.ok": "ok:P-521:derive=66",
 "ECDH.pkcs8.noPublic": "ok:P-521:derive=66",
 "ECDH.pkcs8.innerCurve": "ok:P-521:derive=66",
 "ECDH.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDH.pkcs8.outerOther": "DataError: ",
 "ECDH.pkcs8.scalarZero": "DataError: ",
 "ECDH.pkcs8.scalarN": "DataError: ",
 "ECDH.pkcs8.scalarShort": "ok:P-521:derive=66",
 "ECDH.pkcs8.scalarLong": "ok:P-521:derive=66",
 "ECDH.pkcs8.pubMismatch": "DataError: ",
 "ECDH.pkcs8.pubOffCurve": "DataError: ",
 "ECDH.pkcs8.garbage": "DataError: ",
 "ECDH.pkcs8.trailing": "DataError: ",
 "ECDH.pkcs8.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.jwkPub.ok": "ok:P-521:derive=66",
 "ECDH.jwkPub.crvOther": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDH.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDH.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDH.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 65 bytes but should be 66",
 "ECDH.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 67 bytes but should be 66",
 "ECDH.jwkPub.offCurve": "DataError: ",
 "ECDH.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDH.jwkPub.otherCoords": "DataError: The JWK's \"x\" member defines an octet string of length 32 bytes but should be 66",
 "ECDH.jwkPub.algES256": "ok:P-521:derive=66",
 "ECDH.jwkPub.algES384": "ok:P-521:derive=66",
 "ECDH.jwkPub.useEnc": "ok:P-521:derive=66",
 "ECDH.jwkPub.useSig": "ok:P-521:derive=66",
 "ECDH.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 66",
 "ECDH.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDH.jwkPriv.ok": "ok:private:derive=66",
 "ECDH.jwkPriv.dZero": "OperationError: ",
 "ECDH.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 65 bytes but should be 66",
 "ECDH.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDH.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDH.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 66",
 "ECDH.jwkPriv.offCurve": "DataError: ",
 "interop.dh": "0043231e6a62171a7974d4a8bb47893ed7d505b40687e6c2da295fc65b09ea82f4e0ac73d0f220a42e5181e0cd8a082f098407fe5fd4de66487cf7b1b7035cd19576",
 "interop.chromeSigVerifies": "true",
 "gen.ecdsa": "132:true:P-521:66:66",
 "gen.ecdh": "66"
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_ec_import_malformed_p384_like_chrome() {
    // The P-256 matrix (webcrypto_ec_import_malformed_like_chrome) again on
    // P-384, with P-256 as the "other curve": same variants, same use of each
    // imported key, expected = Chrome 157's output for this exact script
    // (headless, file://). Faces are the P-256 ones with 48-byte lengths and
    // ES384 as the accepted jwk alg.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const D = {"v": {"raw": {"ok": "BBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbA==", "compressed": "AhPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylA==", "short": "BBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2p", "long": "BBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbAA=", "prefix05": "BRPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbA==", "offCurve": "BBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbQ==", "xGeP": "BP/////////////////////////////////////////+/////wAAAAAAAAABAAAAAOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbA==", "infinity": "AA==", "empty": "", "otherPoint": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0="}, "spki": {"ok": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals", "garbage": "CwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5P", "truncated": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlLw==", "trailing": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofalsAA==", "curveOther": "MHkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals", "noCurve": "MG8wCQYHKoZIzj0CAQNiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw=", "unusedBits": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgEEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals", "offCurve": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofalt", "compressed": "MEYwEAYHKoZIzj0CAQYFK4EEACIDMgACE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU", "ecdhOid": "MHQwDgYFK4EEAQwGBSuBBAAiA2IABBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbA==", "otherKey": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQ=="}, "pkcs8": {"ok": "MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw=", "noPublic": "ME4CAQAwEAYHKoZIzj0CAQYFK4EEACIENzA1AgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8=", "innerCurve": "MIG/AgEAMBAGByqGSM49AgEGBSuBBAAiBIGnMIGkAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+gBwYFK4EEACKhZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw=", "innerCurveMismatch": "MIHCAgEAMBAGByqGSM49AgEGBSuBBAAiBIGqMIGnAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+gCgYIKoZIzj0DAQehZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw=", "outerOther": "MIG5AgEAMBMGByqGSM49AgEGCCqGSM49AwEHBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw=", "scalarZero": "ME4CAQAwEAYHKoZIzj0CAQYFK4EEACIENzA1AgEBBDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "scalarN": "ME4CAQAwEAYHKoZIzj0CAQYFK4EEACIENzA1AgEBBDD////////////////////////////////HY02B9Dct31gaDbJIsKd67OwZaszFKXM=", "scalarShort": "ME0CAQAwEAYHKoZIzj0CAQYFK4EEACIENjA0AgEBBC8AAAAAKkxujwsdP1p8nhs9X3qcHjtdf5ocPlt9nxo8XnudHzpcfpsdP1p8nhs9Xw==", "scalarLong": "ME8CAQAwEAYHKoZIzj0CAQYFK4EEACIEODA2AgEBBDEAAAAAAAAqTG6PCx0/WnyeGz1fepweO11/mhw+W32fGjxee50fOlx+mx0/WnyeGz1f", "pubMismatch": "MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAToa5leNQ5NMSqVprfV2M3NtGgvWIgAoc1sly4dyypaw42QBGlRsDzLOxeaif2oixtIW58sH0Sw3UHObjw6aQu/qr7B4f9K77JGH92mYpvsvUPPdME9jBoWJWEiQ/AEYUg=", "pubOffCurve": "MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qW0=", "garbage": "AwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZ", "trailing": "MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWwA", "otherKey": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN"}}, "j": {"x": "E9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ-tjBTa57unKU", "y": "49gDBFlC6GqphjYrv1YormnvF-mFXEL_Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals", "d": "AAAAAAAqTG6PCx0_WnyeGz1fepweO11_mhw-W32fGjxee50fOlx-mx0_WnyeGz1f", "x3": "3ekkyaO3PdweD_LJdGppCjgug4xpSwnfzlxsZ8mUckc", "y3": "J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0", "d3": "HTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4", "xShort": "3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpQ", "yLong": "AOPYAwRZQuhqqYY2K79WKK5p7xfphVxC_13aN5avJ5js-a9AgUcRbA6qpS9eaH2pbA", "yOff": "49gDBFlC6GqphjYrv1YormnvF-mFXEL_Xdo3lq8nmOz5r0CBRxFsDqqlL15ofalt", "dZero": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "dShort": "AAAAACpMbo8LHT9afJ4bPV96nB47XX-aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV8", "dOther": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADA5"}};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const msg = enc.encode('diting-ec');
  const EC = (name) => ({ name, namedCurve: 'P-384' });
  const SIG = { name: 'ECDSA', hash: 'SHA-384' };
  const okPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDSA'), true, ['sign']);
  const okPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDSA'), true, ['verify']);
  const okSig = await S.sign(SIG, okPriv, msg);
  const dhPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDH'), true, ['deriveBits']);
  const dhPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDH'), true, []);
  const usePub = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, k, okSig, msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: k }, dhPriv, 384)).byteLength;
  const usePriv = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, okPub, await S.sign(SIG, k, msg), msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: dhPub }, k, 384)).byteLength;
  for (const name of ['ECDSA', 'ECDH']) {
    const pubUs = name === 'ECDSA' ? ['verify'] : [], privUs = name === 'ECDSA' ? ['sign'] : ['deriveBits'];
    for (const fmt of ['raw', 'spki']) for (const [k, v] of Object.entries(D.v[fmt])) {
      r[name + '.' + fmt + '.' + k] = await face(async () => { const key = await S.importKey(fmt, b64(v), EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    }
    for (const [k, v] of Object.entries(D.v.pkcs8)) {
      r[name + '.pkcs8.' + k] = await face(async () => { const key = await S.importKey('pkcs8', b64(v), EC(name), true, privUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePriv(key, name)); });
    }
    const J = D.j, P = { kty: 'EC', crv: 'P-384', x: J.x, y: J.y };
    const W = (o, e) => Object.assign({}, o, e), drop = (o, m) => { const c = Object.assign({}, o); delete c[m]; return c; };
    const pubJ = { ok: P, crvOther: W(P, { crv: 'P-256' }), crvBogus: W(P, { crv: 'P-999' }), noCrv: drop(P, 'crv'), noX: drop(P, 'x'), noY: drop(P, 'y'),
      xShort: W(P, { x: J.xShort }), yLong: W(P, { y: J.yLong }), offCurve: W(P, { y: J.yOff }), ktyRSA: W(P, { kty: 'RSA' }), otherCoords: W(P, { x: J.x3, y: J.y3 }),
      algES256: W(P, { alg: 'ES256' }), algES384: W(P, { alg: 'ES384' }), useEnc: W(P, { use: 'enc' }), useSig: W(P, { use: 'sig' }), xEmpty: W(P, { x: '' }), xPadded: W(P, { x: J.x + '=' }) };
    for (const [k, j] of Object.entries(pubJ)) r[name + '.jwkPub.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    const Q = W(P, { d: J.d });
    const privJ = { ok: Q, dZero: W(Q, { d: J.dZero }), dShort: W(Q, { d: J.dShort }), dMismatch: W(Q, { d: J.dOther }), noD: drop(Q, 'd'), dEmpty: W(Q, { d: '' }), offCurve: W(Q, { y: J.yOff }) };
    for (const [k, j] of Object.entries(privJ)) r[name + '.jwkPriv.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, privUs); return 'ok:' + key.type + ':' + (await (key.type === 'private' ? usePriv(key, name) : usePub(key, name))); });
  }
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "ECDSA.raw.ok": "ok:P-384:verify=true",
 "ECDSA.raw.compressed": "ok:P-384:verify=true",
 "ECDSA.raw.short": "DataError: ",
 "ECDSA.raw.long": "DataError: ",
 "ECDSA.raw.prefix05": "DataError: ",
 "ECDSA.raw.offCurve": "DataError: ",
 "ECDSA.raw.xGeP": "DataError: ",
 "ECDSA.raw.infinity": "DataError: ",
 "ECDSA.raw.empty": "DataError: ",
 "ECDSA.raw.otherPoint": "DataError: ",
 "ECDSA.spki.ok": "ok:P-384:verify=true",
 "ECDSA.spki.garbage": "DataError: ",
 "ECDSA.spki.truncated": "DataError: ",
 "ECDSA.spki.trailing": "DataError: ",
 "ECDSA.spki.curveOther": "DataError: ",
 "ECDSA.spki.noCurve": "DataError: ",
 "ECDSA.spki.unusedBits": "DataError: ",
 "ECDSA.spki.offCurve": "DataError: ",
 "ECDSA.spki.compressed": "ok:P-384:verify=true",
 "ECDSA.spki.ecdhOid": "DataError: ",
 "ECDSA.spki.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.pkcs8.ok": "ok:P-384:verify=true",
 "ECDSA.pkcs8.noPublic": "ok:P-384:verify=true",
 "ECDSA.pkcs8.innerCurve": "ok:P-384:verify=true",
 "ECDSA.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDSA.pkcs8.outerOther": "DataError: ",
 "ECDSA.pkcs8.scalarZero": "DataError: ",
 "ECDSA.pkcs8.scalarN": "DataError: ",
 "ECDSA.pkcs8.scalarShort": "ok:P-384:verify=true",
 "ECDSA.pkcs8.scalarLong": "ok:P-384:verify=true",
 "ECDSA.pkcs8.pubMismatch": "DataError: ",
 "ECDSA.pkcs8.pubOffCurve": "DataError: ",
 "ECDSA.pkcs8.garbage": "DataError: ",
 "ECDSA.pkcs8.trailing": "DataError: ",
 "ECDSA.pkcs8.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.jwkPub.ok": "ok:P-384:verify=true",
 "ECDSA.jwkPub.crvOther": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDSA.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDSA.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDSA.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 47 bytes but should be 48",
 "ECDSA.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 49 bytes but should be 48",
 "ECDSA.jwkPub.offCurve": "DataError: ",
 "ECDSA.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDSA.jwkPub.otherCoords": "DataError: The JWK's \"x\" member defines an octet string of length 32 bytes but should be 48",
 "ECDSA.jwkPub.algES256": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "ECDSA.jwkPub.algES384": "ok:P-384:verify=true",
 "ECDSA.jwkPub.useEnc": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "ECDSA.jwkPub.useSig": "ok:P-384:verify=true",
 "ECDSA.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 48",
 "ECDSA.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDSA.jwkPriv.ok": "ok:private:verify=true",
 "ECDSA.jwkPriv.dZero": "OperationError: ",
 "ECDSA.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 47 bytes but should be 48",
 "ECDSA.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDSA.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDSA.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 48",
 "ECDSA.jwkPriv.offCurve": "DataError: ",
 "ECDH.raw.ok": "ok:P-384:derive=48",
 "ECDH.raw.compressed": "ok:P-384:derive=48",
 "ECDH.raw.short": "DataError: ",
 "ECDH.raw.long": "DataError: ",
 "ECDH.raw.prefix05": "DataError: ",
 "ECDH.raw.offCurve": "DataError: ",
 "ECDH.raw.xGeP": "DataError: ",
 "ECDH.raw.infinity": "DataError: ",
 "ECDH.raw.empty": "DataError: ",
 "ECDH.raw.otherPoint": "DataError: ",
 "ECDH.spki.ok": "ok:P-384:derive=48",
 "ECDH.spki.garbage": "DataError: ",
 "ECDH.spki.truncated": "DataError: ",
 "ECDH.spki.trailing": "DataError: ",
 "ECDH.spki.curveOther": "DataError: ",
 "ECDH.spki.noCurve": "DataError: ",
 "ECDH.spki.unusedBits": "DataError: ",
 "ECDH.spki.offCurve": "DataError: ",
 "ECDH.spki.compressed": "ok:P-384:derive=48",
 "ECDH.spki.ecdhOid": "DataError: ",
 "ECDH.spki.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.pkcs8.ok": "ok:P-384:derive=48",
 "ECDH.pkcs8.noPublic": "ok:P-384:derive=48",
 "ECDH.pkcs8.innerCurve": "ok:P-384:derive=48",
 "ECDH.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDH.pkcs8.outerOther": "DataError: ",
 "ECDH.pkcs8.scalarZero": "DataError: ",
 "ECDH.pkcs8.scalarN": "DataError: ",
 "ECDH.pkcs8.scalarShort": "ok:P-384:derive=48",
 "ECDH.pkcs8.scalarLong": "ok:P-384:derive=48",
 "ECDH.pkcs8.pubMismatch": "DataError: ",
 "ECDH.pkcs8.pubOffCurve": "DataError: ",
 "ECDH.pkcs8.garbage": "DataError: ",
 "ECDH.pkcs8.trailing": "DataError: ",
 "ECDH.pkcs8.otherKey": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.jwkPub.ok": "ok:P-384:derive=48",
 "ECDH.jwkPub.crvOther": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDH.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDH.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDH.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 47 bytes but should be 48",
 "ECDH.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 49 bytes but should be 48",
 "ECDH.jwkPub.offCurve": "DataError: ",
 "ECDH.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDH.jwkPub.otherCoords": "DataError: The JWK's \"x\" member defines an octet string of length 32 bytes but should be 48",
 "ECDH.jwkPub.algES256": "ok:P-384:derive=48",
 "ECDH.jwkPub.algES384": "ok:P-384:derive=48",
 "ECDH.jwkPub.useEnc": "ok:P-384:derive=48",
 "ECDH.jwkPub.useSig": "ok:P-384:derive=48",
 "ECDH.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 48",
 "ECDH.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDH.jwkPriv.ok": "ok:private:derive=48",
 "ECDH.jwkPriv.dZero": "OperationError: ",
 "ECDH.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 47 bytes but should be 48",
 "ECDH.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDH.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDH.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 48",
 "ECDH.jwkPriv.offCurve": "DataError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_results_settle_at_a_task_boundary() {
    // Our SubtleCrypto ops are mostly synchronous, and a promise resolved by
    // one is a microtask: a page's chain of them ran as one continuous wall,
    // which the eval patience watchdog beheads past 500ms (#69 family — the
    // P-384 import battery above died at 457ms of crypto). Chrome posts each
    // result back as a task. Forty P-384 signatures (~0.9s in a debug build)
    // must all land, with no watchdog fire.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder();
        const p = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-384' }, true, ['sign', 'verify']);
        let ok = 0;
        for (let i = 0; i < 40; i++) {
            const sig = await S.sign({ name: 'ECDSA', hash: 'SHA-384' }, p.privateKey, enc.encode('m' + i));
            if (sig.byteLength === 96) ok++;
        }
        return ok;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!(40));
    assert_eq!(rt.watchdog_fired_total(), 0, "no wall, no fire");
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_ec_import_malformed_like_chrome() {
    // ECDSA and ECDH P-256 imports in all four formats, each variant wrong in
    // one place (fixed test keys, generated for this test only): raw
    // compressed / short / long / bad prefix / off-curve / x >= p / identity /
    // a P-384 point; SPKI garbage / truncated / trailing / P-384 OID / no
    // curve / unused bits / off-curve / compressed / id-ecDH / a real P-384
    // key; PKCS#8 without the public key / inner curve (matching or not) /
    // outer P-384 / zero or n scalar / short or zero-padded scalar /
    // mismatched or off-curve embedded public key / garbage / trailing / a
    // real P-384 key; jwk crv / x / y / d faces, lengths, off-curve, d of zero
    // or of another key, alg, use, kty, padding. Each imported key is used
    // (verify a fixed signature / sign then verify / ECDH deriveBits).
    // Expected = Chrome 157's output for this exact script (headless,
    // file://). We used to refuse compressed and zero-padded keys, accept
    // missing crv / wrong alg or use / mismatched d / off-curve private jwks,
    // throw plain Errors, and say "must contain 'undefined'".
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const D = {"v": {"raw": {"ok": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0=", "compressed": "A93pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJH", "short": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmQ==", "long": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0A", "prefix05": "Bd3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0=", "offCurve": "BN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmYw=", "xGeP": "BP////8AAAABAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0=", "infinity": "AA==", "empty": "", "p384Point": "BBPcZrRZqXt31Ao0uRqFF0ybFABRQIoSyBYY+51ytzTKKUi5t663UPrYwU2ue7pylOPYAwRZQuhqqYY2K79WKK5p7xfphVxC/13aN5avJ5js+a9AgUcRbA6qpS9eaH2pbA=="}, "spki": {"ok": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQ==", "garbage": "CwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5P", "truncated": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQ=", "trailing": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQA=", "curveP384": "MFYwEAYHKoZIzj0CAQYFK4EEACIDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQ==", "noCurve": "ME8wCQYHKoZIzj0CAQNCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "unusedBits": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgEE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjQ==", "offCurve": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckcn0wriGk6XimGdnKz1HsqtAqEsaJK8kh6FMcQK5JWZjA==", "compressed": "MDkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDIgAD3ekkyaO3PdweD/LJdGppCjgug4xpSwnfzlxsZ8mUckc=", "ecdhOid": "MFcwEQYFK4EEAQwGCCqGSM49AwEHA0IABN3pJMmjtz3cHg/yyXRqaQo4LoOMaUsJ385cbGfJlHJHJ9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0=", "p384Key": "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEE9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ+tjBTa57unKU49gDBFlC6GqphjYrv1YormnvF+mFXEL/Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals"}, "pkcs8": {"ok": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "noPublic": "MEECAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJzAlAgEBBCAdPFp+my9MbY4KGzxdfp8KK0xtjg8aO1x9nh8qS2yNDg==", "innerCurve": "MIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6gCgYIKoZIzj0DAQehRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "innerCurveMismatch": "MIGQAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHYwdAIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6gBwYFK4EEACKhRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "outerP384": "MIGEAgEAMBAGByqGSM49AgEGBSuBBAAiBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "scalarZero": "MEECAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJzAlAgEBBCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==", "scalarN": "MEECAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJzAlAgEBBCD/////AAAAAP//////////vOb6racXnoTzucrC/GMlUQ==", "scalarShort": "MEACAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEJjAkAgEBBB88Wn6bL0xtjgobPF1+nworTG2ODxo7XH2eHypLbI0O", "scalarLong": "MEICAQAwEwYHKoZIzj0CAQYIKoZIzj0DAQcEKDAmAgEBBCEAHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4=", "pubMismatch": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAAQm7869DunjSmaRh+GLOpEisvczlFtknMn5+SHp+drYEpAji96cx7szDRUMZ3BN0lrnBVIFdEtvMb9AcHRYctDm", "pubOffCurve": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmM", "garbage": "AwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZ", "trailing": "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgHTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ6hRANCAATd6STJo7c93B4P8sl0amkKOC6DjGlLCd/OXGxnyZRyRyfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmNAA==", "p384Key": "MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAAAAAAACpMbo8LHT9afJ4bPV96nB47XX+aHD5bfZ8aPF57nR86XH6bHT9afJ4bPV+hZANiAAQT3Ga0Wal7d9QKNLkahRdMmxQAUUCKEsgWGPudcrc0yilIubeut1D62MFNrnu6cpTj2AMEWULoaqmGNiu/Viiuae8X6YVcQv9d2jeWryeY7PmvQIFHEWwOqqUvXmh9qWw="}}, "j": {"x": "3ekkyaO3PdweD_LJdGppCjgug4xpSwnfzlxsZ8mUckc", "y": "J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0", "d": "HTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4", "x3": "E9xmtFmpe3fUCjS5GoUXTJsUAFFAihLIFhj7nXK3NMopSLm3rrdQ-tjBTa57unKU", "y3": "49gDBFlC6GqphjYrv1YormnvF-mFXEL_Xdo3lq8nmOz5r0CBRxFsDqqlL15ofals", "d3": "AAAAAAAqTG6PCx0_WnyeGz1fepweO11_mhw-W32fGjxee50fOlx-mx0_WnyeGz1f", "xShort": "6STJo7c93B4P8sl0amkKOC6DjGlLCd_OXGxnyZRyRw", "yLong": "ACfTCuIaTpeKYZ2crPUeyq0CoSxokrySHoUxxArklZmN", "yOff": "J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmYw", "dZero": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "dShort": "PFp-my9MbY4KGzxdfp8KK0xtjg8aO1x9nh8qS2yNDg", "dOther": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMDk"}};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const msg = enc.encode('diting-ec');
  const EC = (name) => ({ name, namedCurve: 'P-256' });
  const SIG = { name: 'ECDSA', hash: 'SHA-256' };
  const okPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDSA'), true, ['sign']);
  const okPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDSA'), true, ['verify']);
  const okSig = await S.sign(SIG, okPriv, msg);
  const dhPriv = await S.importKey('pkcs8', b64(D.v.pkcs8.ok), EC('ECDH'), true, ['deriveBits']);
  const dhPub = await S.importKey('raw', b64(D.v.raw.ok), EC('ECDH'), true, []);
  const usePub = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, k, okSig, msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: k }, dhPriv, 256)).byteLength;
  const usePriv = async (k, name) => name === 'ECDSA' ? 'verify=' + (await S.verify(SIG, okPub, await S.sign(SIG, k, msg), msg)) : 'derive=' + (await S.deriveBits({ name: 'ECDH', public: dhPub }, k, 256)).byteLength;
  for (const name of ['ECDSA', 'ECDH']) {
    const pubUs = name === 'ECDSA' ? ['verify'] : [], privUs = name === 'ECDSA' ? ['sign'] : ['deriveBits'];
    for (const fmt of ['raw', 'spki']) for (const [k, v] of Object.entries(D.v[fmt])) {
      r[name + '.' + fmt + '.' + k] = await face(async () => { const key = await S.importKey(fmt, b64(v), EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    }
    for (const [k, v] of Object.entries(D.v.pkcs8)) {
      r[name + '.pkcs8.' + k] = await face(async () => { const key = await S.importKey('pkcs8', b64(v), EC(name), true, privUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePriv(key, name)); });
    }
    const J = D.j, P = { kty: 'EC', crv: 'P-256', x: J.x, y: J.y };
    const W = (o, e) => Object.assign({}, o, e), drop = (o, m) => { const c = Object.assign({}, o); delete c[m]; return c; };
    const pubJ = { ok: P, crvP384: W(P, { crv: 'P-384' }), crvBogus: W(P, { crv: 'P-999' }), noCrv: drop(P, 'crv'), noX: drop(P, 'x'), noY: drop(P, 'y'),
      xShort: W(P, { x: J.xShort }), yLong: W(P, { y: J.yLong }), offCurve: W(P, { y: J.yOff }), ktyRSA: W(P, { kty: 'RSA' }), p384Coords: W(P, { x: J.x3, y: J.y3 }),
      algES256: W(P, { alg: 'ES256' }), algES384: W(P, { alg: 'ES384' }), useEnc: W(P, { use: 'enc' }), useSig: W(P, { use: 'sig' }), xEmpty: W(P, { x: '' }), xPadded: W(P, { x: J.x + '=' }) };
    for (const [k, j] of Object.entries(pubJ)) r[name + '.jwkPub.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, pubUs); return 'ok:' + key.algorithm.namedCurve + ':' + (await usePub(key, name)); });
    const Q = W(P, { d: J.d });
    const privJ = { ok: Q, dZero: W(Q, { d: J.dZero }), dShort: W(Q, { d: J.dShort }), dMismatch: W(Q, { d: J.dOther }), noD: drop(Q, 'd'), dEmpty: W(Q, { d: '' }), offCurve: W(Q, { y: J.yOff }) };
    for (const [k, j] of Object.entries(privJ)) r[name + '.jwkPriv.' + k] = await face(async () => { const key = await S.importKey('jwk', j, EC(name), true, privUs); return 'ok:' + key.type + ':' + (await (key.type === 'private' ? usePriv(key, name) : usePub(key, name))); });
  }
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "ECDSA.raw.ok": "ok:P-256:verify=true",
 "ECDSA.raw.compressed": "ok:P-256:verify=true",
 "ECDSA.raw.short": "DataError: ",
 "ECDSA.raw.long": "DataError: ",
 "ECDSA.raw.prefix05": "DataError: ",
 "ECDSA.raw.offCurve": "DataError: ",
 "ECDSA.raw.xGeP": "DataError: ",
 "ECDSA.raw.infinity": "DataError: ",
 "ECDSA.raw.empty": "DataError: ",
 "ECDSA.raw.p384Point": "DataError: ",
 "ECDSA.spki.ok": "ok:P-256:verify=true",
 "ECDSA.spki.garbage": "DataError: ",
 "ECDSA.spki.truncated": "DataError: ",
 "ECDSA.spki.trailing": "DataError: ",
 "ECDSA.spki.curveP384": "DataError: ",
 "ECDSA.spki.noCurve": "DataError: ",
 "ECDSA.spki.unusedBits": "DataError: ",
 "ECDSA.spki.offCurve": "DataError: ",
 "ECDSA.spki.compressed": "ok:P-256:verify=true",
 "ECDSA.spki.ecdhOid": "DataError: ",
 "ECDSA.spki.p384Key": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.pkcs8.ok": "ok:P-256:verify=true",
 "ECDSA.pkcs8.noPublic": "ok:P-256:verify=true",
 "ECDSA.pkcs8.innerCurve": "ok:P-256:verify=true",
 "ECDSA.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDSA.pkcs8.outerP384": "DataError: ",
 "ECDSA.pkcs8.scalarZero": "DataError: ",
 "ECDSA.pkcs8.scalarN": "DataError: ",
 "ECDSA.pkcs8.scalarShort": "ok:P-256:verify=false",
 "ECDSA.pkcs8.scalarLong": "ok:P-256:verify=true",
 "ECDSA.pkcs8.pubMismatch": "DataError: ",
 "ECDSA.pkcs8.pubOffCurve": "DataError: ",
 "ECDSA.pkcs8.garbage": "DataError: ",
 "ECDSA.pkcs8.trailing": "DataError: ",
 "ECDSA.pkcs8.p384Key": "DataError: The imported EC key specifies a different curve than requested",
 "ECDSA.jwkPub.ok": "ok:P-256:verify=true",
 "ECDSA.jwkPub.crvP384": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDSA.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDSA.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDSA.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDSA.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 31 bytes but should be 32",
 "ECDSA.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 33 bytes but should be 32",
 "ECDSA.jwkPub.offCurve": "DataError: ",
 "ECDSA.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDSA.jwkPub.p384Coords": "DataError: The JWK's \"x\" member defines an octet string of length 48 bytes but should be 32",
 "ECDSA.jwkPub.algES256": "ok:P-256:verify=true",
 "ECDSA.jwkPub.algES384": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "ECDSA.jwkPub.useEnc": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "ECDSA.jwkPub.useSig": "ok:P-256:verify=true",
 "ECDSA.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 32",
 "ECDSA.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDSA.jwkPriv.ok": "ok:private:verify=true",
 "ECDSA.jwkPriv.dZero": "OperationError: ",
 "ECDSA.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 31 bytes but should be 32",
 "ECDSA.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDSA.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDSA.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 32",
 "ECDSA.jwkPriv.offCurve": "DataError: ",
 "ECDH.raw.ok": "ok:P-256:derive=32",
 "ECDH.raw.compressed": "ok:P-256:derive=32",
 "ECDH.raw.short": "DataError: ",
 "ECDH.raw.long": "DataError: ",
 "ECDH.raw.prefix05": "DataError: ",
 "ECDH.raw.offCurve": "DataError: ",
 "ECDH.raw.xGeP": "DataError: ",
 "ECDH.raw.infinity": "DataError: ",
 "ECDH.raw.empty": "DataError: ",
 "ECDH.raw.p384Point": "DataError: ",
 "ECDH.spki.ok": "ok:P-256:derive=32",
 "ECDH.spki.garbage": "DataError: ",
 "ECDH.spki.truncated": "DataError: ",
 "ECDH.spki.trailing": "DataError: ",
 "ECDH.spki.curveP384": "DataError: ",
 "ECDH.spki.noCurve": "DataError: ",
 "ECDH.spki.unusedBits": "DataError: ",
 "ECDH.spki.offCurve": "DataError: ",
 "ECDH.spki.compressed": "ok:P-256:derive=32",
 "ECDH.spki.ecdhOid": "DataError: ",
 "ECDH.spki.p384Key": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.pkcs8.ok": "ok:P-256:derive=32",
 "ECDH.pkcs8.noPublic": "ok:P-256:derive=32",
 "ECDH.pkcs8.innerCurve": "ok:P-256:derive=32",
 "ECDH.pkcs8.innerCurveMismatch": "DataError: ",
 "ECDH.pkcs8.outerP384": "DataError: ",
 "ECDH.pkcs8.scalarZero": "DataError: ",
 "ECDH.pkcs8.scalarN": "DataError: ",
 "ECDH.pkcs8.scalarShort": "ok:P-256:derive=32",
 "ECDH.pkcs8.scalarLong": "ok:P-256:derive=32",
 "ECDH.pkcs8.pubMismatch": "DataError: ",
 "ECDH.pkcs8.pubOffCurve": "DataError: ",
 "ECDH.pkcs8.garbage": "DataError: ",
 "ECDH.pkcs8.trailing": "DataError: ",
 "ECDH.pkcs8.p384Key": "DataError: The imported EC key specifies a different curve than requested",
 "ECDH.jwkPub.ok": "ok:P-256:derive=32",
 "ECDH.jwkPub.crvP384": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.crvBogus": "DataError: The JWK's \"crv\" member specifies a different curve than requested",
 "ECDH.jwkPub.noCrv": "DataError: The required JWK member \"crv\" was missing",
 "ECDH.jwkPub.noX": "DataError: The required JWK member \"x\" was missing",
 "ECDH.jwkPub.noY": "DataError: The required JWK member \"y\" was missing",
 "ECDH.jwkPub.xShort": "DataError: The JWK's \"x\" member defines an octet string of length 31 bytes but should be 32",
 "ECDH.jwkPub.yLong": "DataError: The JWK's \"y\" member defines an octet string of length 33 bytes but should be 32",
 "ECDH.jwkPub.offCurve": "DataError: ",
 "ECDH.jwkPub.ktyRSA": "DataError: The JWK \"kty\" member was not \"EC\"",
 "ECDH.jwkPub.p384Coords": "DataError: The JWK's \"x\" member defines an octet string of length 48 bytes but should be 32",
 "ECDH.jwkPub.algES256": "ok:P-256:derive=32",
 "ECDH.jwkPub.algES384": "ok:P-256:derive=32",
 "ECDH.jwkPub.useEnc": "ok:P-256:derive=32",
 "ECDH.jwkPub.useSig": "ok:P-256:derive=32",
 "ECDH.jwkPub.xEmpty": "DataError: The JWK's \"x\" member defines an octet string of length 0 bytes but should be 32",
 "ECDH.jwkPub.xPadded": "DataError: The JWK member \"x\" could not be base64url decoded or contained padding",
 "ECDH.jwkPriv.ok": "ok:private:derive=32",
 "ECDH.jwkPriv.dZero": "OperationError: ",
 "ECDH.jwkPriv.dShort": "DataError: The JWK's \"d\" member defines an octet string of length 31 bytes but should be 32",
 "ECDH.jwkPriv.dMismatch": "DataError: The imported EC key is invalid",
 "ECDH.jwkPriv.noD": "SyntaxError: Cannot create a key using the specified key usages.",
 "ECDH.jwkPriv.dEmpty": "DataError: The JWK's \"d\" member defines an octet string of length 0 bytes but should be 32",
 "ECDH.jwkPriv.offCurve": "DataError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_jwk_integer_members_like_chrome() {
    // RSA jwk members are minimal big-endian unsigned integers. Public keys
    // with invalid n/e (even, 1, zero, oversize e; e = n; even, zero, 511-bit
    // n) and malformed encodings (empty, leading zero, padded, standard
    // base64), imported as RSA-PSS / RSASSA / OAEP; then every private member
    // (d, p, q, dp, dq, qi, n) with a leading zero or empty. Fixed 1024-bit
    // test key. Expected = Chrome 157's output for this exact script
    // (headless, file://): invalid values are an empty OperationError,
    // encodings get their own DataError text. We passed rsa's wording
    // through, read "" as a base64 failure, and accepted leading zeros.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle;
  const V = {"ok": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB"}, "eEven": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAC"}, "eOne": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQ"}, "eThree": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "Aw"}, "eHuge": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAAAAAB"}, "e33bit": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAAAAE"}, "eZero": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AA"}, "eEmpty": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": ""}, "eLeadZero": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AAEAAQ"}, "eqN": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc"}, "nEven": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMg", "e": "AQAB"}, "nZero": {"n": "AA", "e": "AQAB"}, "nEmpty": {"n": "", "e": "AQAB"}, "nLeadZero": {"n": "AM_9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv_sR_oKIWu3bLmWGCLis3H_9HXD_DU0RiqL03yP6caec-cDaBtrVsE88MaYR_DgMT2WCUbMAsK9jmczH", "e": "AQAB"}, "nPadded": {"n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc=", "e": "AQAB"}, "nStdB64": {"n": "z/1dpivZhivMVP0wpXd7Syp6oYK+EhI+pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV+/+xH+goha7dsuZYYIuKzcf/0dcP8NTRGKovTfI/pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB"}, "n511": {"n": "Z_6u0xXswxXmKn6YUru9pZU9UMFfCQkfUuE4suneRDzA4AZ3Lwyk70UJeeWCWBvE3noFxuHsqehHmUvUHwBE8Q", "e": "AQAB"}};
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  for (const [tag, alg, us] of [['pss', { name: 'RSA-PSS', hash: 'SHA-256' }, ['verify']], ['pkcs1', { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, ['verify']], ['oaep', { name: 'RSA-OAEP', hash: 'SHA-256' }, ['encrypt']]]) {
    for (const [k, m] of Object.entries(V)) {
      r[tag + '.' + k] = await face(async () => {
        const pk = await S.importKey('jwk', Object.assign({ kty: 'RSA' }, m), alg, true, us);
        const j = await S.exportKey('jwk', pk);
        return 'ok:' + pk.algorithm.modulusLength + ':' + pk.algorithm.publicExponent.join('.') + ':' + (j.n === m.n) + ':' + (j.e === m.e);
      });
    }
  }
  const P = {"lz.d": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "ADOc1QtyYDVaUwdYGSVGhBMABJRlxFLpsiAiI0-4flWDiX4l91Tkg580tkVHQO-VqJl6Gg_g5OJWo-JTHg3cxlqLgGD839UHCBJsGGiKuEqCt1GqdGtnb3Xi2fhcPUCeDaOwhf5m11_1J9mnMTEA_6gme2oGD2UKVqge2pDo77yB", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "empty.d": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "lz.p": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "ANwLd_bt7H2kOcMdkTlQUCcZf2K_Rt_3e3p4VxAlQ_hpZCwmA4eiuPChPLwHMsMpODlkDAM_SFq8OggHyGwCuAc", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "empty.p": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "lz.q": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "APH5oTL4-Is8pu6YOd-jicL1ArPQuQuZUlbTBn-9yrrw2ZJblohhbF0xT_nJeN72f1kg7I8n9dZRCcBXrlqLlUE", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "empty.q": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "lz.dp": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "AD4tw3V0X-pU-LmI5luStUOHMfbeZBGvpiXCUZ_PV_ifF15eU4sFUUs11KqxQgfh4vBctARK4pA5NhDE2j6jOgs", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "empty.dp": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "lz.dq": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "ADu8Z6pAwoMFJyTR2b-3DMRfXhpsFbKiWCxtDjas8QZzfRLfn2rtuiXLjbXZoghRDCFBuoSg1dThzql4O-fIe8E", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "empty.dq": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "lz.qi": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "AMx_KXW8pcargKvl0sCCyMDuJYTguWAMmDWYKTUumfAZ0sSJzcB15R-Kw_YNFn2aIP9HAcoD29S9HEKPcn_nZII", "ext": true}, "empty.qi": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "", "ext": true}, "lz.n": {"kty": "RSA", "n": "AM_9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv_sR_oKIWu3bLmWGCLis3H_9HXD_DU0RiqL03yP6caec-cDaBtrVsE88MaYR_DgMT2WCUbMAsK9jmczH", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}};
  for (const [k, j] of Object.entries(P)) {
    try { await S.importKey('jwk', j, { name: 'RSA-PSS', hash: 'SHA-256' }, true, ['sign']); r['priv.' + k] = 'ok'; }
    catch (e) { r['priv.' + k] = e.name + ': ' + e.message; }
  }
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "pss.ok": "ok:1024:1.0.1:true:true",
 "pss.eEven": "OperationError: ",
 "pss.eOne": "OperationError: ",
 "pss.eThree": "ok:1024:3:true:true",
 "pss.eHuge": "OperationError: ",
 "pss.e33bit": "ok:1024:1.0.0.0.1:true:true",
 "pss.eZero": "OperationError: ",
 "pss.eEmpty": "DataError: The JWK \"e\" member was empty.",
 "pss.eLeadZero": "DataError: The JWK \"e\" member contained a leading zero.",
 "pss.eqN": "OperationError: ",
 "pss.nEven": "OperationError: ",
 "pss.nZero": "OperationError: ",
 "pss.nEmpty": "DataError: The JWK \"n\" member was empty.",
 "pss.nLeadZero": "DataError: The JWK \"n\" member contained a leading zero.",
 "pss.nPadded": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "pss.nStdB64": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "pss.n511": "OperationError: ",
 "pkcs1.ok": "ok:1024:1.0.1:true:true",
 "pkcs1.eEven": "OperationError: ",
 "pkcs1.eOne": "OperationError: ",
 "pkcs1.eThree": "ok:1024:3:true:true",
 "pkcs1.eHuge": "OperationError: ",
 "pkcs1.e33bit": "ok:1024:1.0.0.0.1:true:true",
 "pkcs1.eZero": "OperationError: ",
 "pkcs1.eEmpty": "DataError: The JWK \"e\" member was empty.",
 "pkcs1.eLeadZero": "DataError: The JWK \"e\" member contained a leading zero.",
 "pkcs1.eqN": "OperationError: ",
 "pkcs1.nEven": "OperationError: ",
 "pkcs1.nZero": "OperationError: ",
 "pkcs1.nEmpty": "DataError: The JWK \"n\" member was empty.",
 "pkcs1.nLeadZero": "DataError: The JWK \"n\" member contained a leading zero.",
 "pkcs1.nPadded": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "pkcs1.nStdB64": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "pkcs1.n511": "OperationError: ",
 "oaep.ok": "ok:1024:1.0.1:true:true",
 "oaep.eEven": "OperationError: ",
 "oaep.eOne": "OperationError: ",
 "oaep.eThree": "ok:1024:3:true:true",
 "oaep.eHuge": "OperationError: ",
 "oaep.e33bit": "ok:1024:1.0.0.0.1:true:true",
 "oaep.eZero": "OperationError: ",
 "oaep.eEmpty": "DataError: The JWK \"e\" member was empty.",
 "oaep.eLeadZero": "DataError: The JWK \"e\" member contained a leading zero.",
 "oaep.eqN": "OperationError: ",
 "oaep.nEven": "OperationError: ",
 "oaep.nZero": "OperationError: ",
 "oaep.nEmpty": "DataError: The JWK \"n\" member was empty.",
 "oaep.nLeadZero": "DataError: The JWK \"n\" member contained a leading zero.",
 "oaep.nPadded": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "oaep.nStdB64": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "oaep.n511": "OperationError: ",
 "priv.lz.d": "DataError: The JWK \"d\" member contained a leading zero.",
 "priv.empty.d": "DataError: The JWK \"d\" member was empty.",
 "priv.lz.p": "DataError: The JWK \"p\" member contained a leading zero.",
 "priv.empty.p": "DataError: The JWK \"p\" member was empty.",
 "priv.lz.q": "DataError: The JWK \"q\" member contained a leading zero.",
 "priv.empty.q": "DataError: The JWK \"q\" member was empty.",
 "priv.lz.dp": "DataError: The JWK \"dp\" member contained a leading zero.",
 "priv.empty.dp": "DataError: The JWK \"dp\" member was empty.",
 "priv.lz.dq": "DataError: The JWK \"dq\" member contained a leading zero.",
 "priv.empty.dq": "DataError: The JWK \"dq\" member was empty.",
 "priv.lz.qi": "DataError: The JWK \"qi\" member contained a leading zero.",
 "priv.empty.qi": "DataError: The JWK \"qi\" member was empty.",
 "priv.lz.n": "DataError: The JWK \"n\" member contained a leading zero."
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_spki_malformed_like_chrome() {
    // SPKI RSA public keys built by hand from a fixed 1024-bit test key, each
    // wrong in one place — garbage, truncated, trailing byte, EC / RSASSA-PSS
    // OIDs, absent or non-NULL parameters, unused bits, inner PKCS#1
    // truncated / trailing / extra field, even or tiny or huge e, e = n, even
    // or zero n, negative or non-minimal INTEGERs — imported as RSA-PSS (and
    // verified against a Chrome-made signature), RSASSA and OAEP. Expected =
    // Chrome 157's output for this exact script (headless, file://): every
    // refusal is an empty DataError. We accepted absent/non-NULL parameters
    // and surfaced the rest as OperationErrors with decoder text.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const D = {"v": {"ok": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQAB", "garbage": "CQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERQ==", "truncated": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5k=", "trailing": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQABAA==", "oidEC": "MIGlMBMGByqGSM49AgEGCCqGSM49AwEHA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQAB", "oidPSS": "MIGdMAsGCSqGSIb3DQEBCgOBjQAwgYkCgYEAz/1dpivZhivMVP0wpXd7Syp6oYK+EhI+pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV+/+xH+goha7dsuZYYIuKzcf/0dcP8NTRGKovTfI/pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMcCAwEAAQ==", "noNullParams": "MIGdMAsGCSqGSIb3DQEBAQOBjQAwgYkCgYEAz/1dpivZhivMVP0wpXd7Syp6oYK+EhI+pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV+/+xH+goha7dsuZYYIuKzcf/0dcP8NTRGKovTfI/pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMcCAwEAAQ==", "paramsInt": "MIGgMA4GCSqGSIb3DQEBAQIBAAOBjQAwgYkCgYEAz/1dpivZhivMVP0wpXd7Syp6oYK+EhI+pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV+/+xH+goha7dsuZYYIuKzcf/0dcP8NTRGKovTfI/pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMcCAwEAAQ==", "unusedBits": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNATCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQAB", "innerTruncated": "MIGcMA0GCSqGSIb3DQEBAQUAA4GKADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwID", "innerTrailing": "MIGhMA0GCSqGSIb3DQEBAQUAA4GPADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQABAAA=", "innerExtraField": "MIGiMA0GCSqGSIb3DQEBAQUAA4GQADCBjAKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQABAgEA", "eEven": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQAC", "eOne": "MIGdMA0GCSqGSIb3DQEBAQUAA4GLADCBhwKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIBAQ==", "nEven": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMyAIDAQAB", "nZero": "MBwwDQYJKoZIhvcNAQEBBQADCwAwCAIBAAIDAQAB", "nNegative": "MIGeMA0GCSqGSIb3DQEBAQUAA4GMADCBiAKBgM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAE=", "nNonMinimal": "MIGgMA0GCSqGSIb3DQEBAQUAA4GOADCBigKBggAAz/1dpivZhivMVP0wpXd7Syp6oYK+EhI+pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV+/+xH+goha7dsuZYYIuKzcf/0dcP8NTRGKovTfI/pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMcCAwEAAQ==", "eNonMinimal": "MIGgMA0GCSqGSIb3DQEBAQUAA4GOADCBigKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIEAAEAAQ==", "eHuge": "MIGiMA0GCSqGSIb3DQEBAQUAA4GQADCBjAKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIGAQAAAAAB", "eqN": "MIIBIDANBgkqhkiG9w0BAQEFAAOCAQ0AMIIBCAKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxw=="}, "pssSig": "62a9dfaa7fcc556957c7b056ae40c4178130d17dd05e3605932fd5c281d5a327ec80ba158bb97797c5d267ca0ad1345199d8506efb71b4ebbde7a444ca8d1160d2e12e5446d464735adc398eea6931ee38946cb5652fcdee75417c9c1f365fa713bf6948b20667b1e3e4c2855186b0d9e1c40ff7799351d50a4e2a09462fda29"};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const unhex = (h) => Uint8Array.from(h.match(/../g), x => parseInt(x, 16));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const msg = enc.encode('diting-pss');
  for (const [tag, alg, us] of [['pss', { name: 'RSA-PSS', hash: 'SHA-256' }, ['verify']], ['pkcs1', { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, ['verify']], ['oaep', { name: 'RSA-OAEP', hash: 'SHA-256' }, ['encrypt']]]) {
    for (const [k, der] of Object.entries(D.v)) {
      r[tag + '.' + k] = await face(async () => {
        const pk = await S.importKey('spki', b64(der), alg, true, us);
        let extra = '';
        if (tag === 'pss') extra = ':' + (await S.verify({ name: 'RSA-PSS', saltLength: 32 }, pk, unhex(D.pssSig), msg));
        return 'ok:' + pk.algorithm.modulusLength + ':' + pk.algorithm.publicExponent.join('.') + extra;
      });
    }
  }
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "pss.ok": "ok:1024:1.0.1:true",
 "pss.garbage": "DataError: ",
 "pss.truncated": "DataError: ",
 "pss.trailing": "DataError: ",
 "pss.oidEC": "DataError: ",
 "pss.oidPSS": "DataError: ",
 "pss.noNullParams": "DataError: ",
 "pss.paramsInt": "DataError: ",
 "pss.unusedBits": "DataError: ",
 "pss.innerTruncated": "DataError: ",
 "pss.innerTrailing": "DataError: ",
 "pss.innerExtraField": "DataError: ",
 "pss.eEven": "DataError: ",
 "pss.eOne": "DataError: ",
 "pss.nEven": "DataError: ",
 "pss.nZero": "DataError: ",
 "pss.nNegative": "DataError: ",
 "pss.nNonMinimal": "DataError: ",
 "pss.eNonMinimal": "DataError: ",
 "pss.eHuge": "DataError: ",
 "pss.eqN": "DataError: ",
 "pkcs1.ok": "ok:1024:1.0.1",
 "pkcs1.garbage": "DataError: ",
 "pkcs1.truncated": "DataError: ",
 "pkcs1.trailing": "DataError: ",
 "pkcs1.oidEC": "DataError: ",
 "pkcs1.oidPSS": "DataError: ",
 "pkcs1.noNullParams": "DataError: ",
 "pkcs1.paramsInt": "DataError: ",
 "pkcs1.unusedBits": "DataError: ",
 "pkcs1.innerTruncated": "DataError: ",
 "pkcs1.innerTrailing": "DataError: ",
 "pkcs1.innerExtraField": "DataError: ",
 "pkcs1.eEven": "DataError: ",
 "pkcs1.eOne": "DataError: ",
 "pkcs1.nEven": "DataError: ",
 "pkcs1.nZero": "DataError: ",
 "pkcs1.nNegative": "DataError: ",
 "pkcs1.nNonMinimal": "DataError: ",
 "pkcs1.eNonMinimal": "DataError: ",
 "pkcs1.eHuge": "DataError: ",
 "pkcs1.eqN": "DataError: ",
 "oaep.ok": "ok:1024:1.0.1",
 "oaep.garbage": "DataError: ",
 "oaep.truncated": "DataError: ",
 "oaep.trailing": "DataError: ",
 "oaep.oidEC": "DataError: ",
 "oaep.oidPSS": "DataError: ",
 "oaep.noNullParams": "DataError: ",
 "oaep.paramsInt": "DataError: ",
 "oaep.unusedBits": "DataError: ",
 "oaep.innerTruncated": "DataError: ",
 "oaep.innerTrailing": "DataError: ",
 "oaep.innerExtraField": "DataError: ",
 "oaep.eEven": "DataError: ",
 "oaep.eOne": "DataError: ",
 "oaep.nEven": "DataError: ",
 "oaep.nZero": "DataError: ",
 "oaep.nNegative": "DataError: ",
 "oaep.nNonMinimal": "DataError: ",
 "oaep.eNonMinimal": "DataError: ",
 "oaep.eHuge": "DataError: ",
 "oaep.eqN": "DataError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_pkcs8_crt_consistency_like_chrome() {
    // PKCS#8 RSA private keys built by hand from a fixed 1024-bit test key,
    // each wrong in one place: dp/dq/qi/d/n/e, qi = 0, dp + (p-1) (right
    // modulo p-1 but unreduced), p/q swapped with and without their CRT
    // members, version 1 with no other primes, garbage, truncated, trailing
    // byte. Imported as RSA-PSS and RSASSA (sign, then verify with the
    // jwk public key) plus one OAEP. Expected = Chrome 157's output for this
    // exact script (headless, file://): every refusal is an empty DataError.
    // rsa's own decode recomputed dp/dq/qi, so wrong CRT members imported
    // fine here, and its errors surfaced as OperationErrors with decoder text.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle, enc = new TextEncoder();
  const D = {"pub": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB"}, "v": {"ok": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "dpWrong": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "dqWrong": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "qiWrong": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgw==", "qiZero": "MIICNgIBADANBgkqhkiG9w0BAQEFAASCAiAwggIcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAgEA", "dWrong": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gwJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "nWrong": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczJAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "eWrong": "MIICdAIBADANBgkqhkiG9w0BAQEFAASCAl4wggJaAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgEDAoGAM5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h+VYOJfiX3VOSDnzS2RUdA75WomXoaD+Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ+Fw9QJ4No7CF/mbXX/Un2acxMQD/qCZ7agYPZQpWqB7akOjvvIECQQDcC3f27ex9pDnDHZE5UFAnGX9iv0bf93t6eFcQJUP4aWQsJgOHorjwoTy8BzLDKTg5ZAwDP0havDoIB8hsArgHAkEA8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP+cl43vZ/WSDsjyf11lEJwFeuWouVQQJAPi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa+mJcJRn89X+J8XXl5TiwVRSzXUqrFCB+Hi8Fy0BErikDk2EMTaPqM6CwJAO7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et+fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQJBAMx/KXW8pcargKvl0sCCyMDuJYTguWAMmDWYKTUumfAZ0sSJzcB15R+Kw/YNFn2aIP9HAcoD29S9HEKPcn/nZII=", "swapPQ": "MIICdQIBADANBgkqhkiG9w0BAQEFAASCAl8wggJbAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBAPH5oTL4+Is8pu6YOd+jicL1ArPQuQuZUlbTBn+9yrrw2ZJblohhbF0xT/nJeN72f1kg7I8n9dZRCcBXrlqLlUECQQDcC3f27ex9pDnDHZE5UFAnGX9iv0bf93t6eFcQJUP4aWQsJgOHorjwoTy8BzLDKTg5ZAwDP0havDoIB8hsArgHAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkARGP7trJeeYareLUJA/RPMhZGhJp1Tl1sxT0O7z6mH8s8wbmi6x8ieGjmPvSZr8T/POFQC8cap6pZ+azfqkcaK", "swapPQraw": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBAPH5oTL4+Is8pu6YOd+jicL1ArPQuQuZUlbTBn+9yrrw2ZJblohhbF0xT/nJeN72f1kg7I8n9dZRCcBXrlqLlUECQQDcC3f27ex9pDnDHZE5UFAnGX9iv0bf93t6eFcQJUP4aWQsJgOHorjwoTy8BzLDKTg5ZAwDP0havDoIB8hsArgHAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "dpPlusPm1": "MIICdwIBADANBgkqhkiG9w0BAQEFAASCAmEwggJdAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkEBGjk7bGJMZ/kyfKZ3lOMFaqCxWZ2q8achoDqor/Sb8Qh7ioRXEqgKO9cRZrh0ywsbKcDAB4oq6vVwGMyiqqXyEQJAO7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et+fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQJBAMx/KXW8pcargKvl0sCCyMDuJYTguWAMmDWYKTUumfAZ0sSJzcB15R+Kw/YNFn2aIP9HAcoD29S9HEKPcn/nZII=", "version1": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEBAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg==", "garbage": "BwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg=", "truncated": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb", "trailing": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkggA="}};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const msg = enc.encode('crt');
  for (const alg of [{ name: 'RSA-PSS', hash: 'SHA-256' }, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }]) {
    const tag = alg.name === 'RSA-PSS' ? 'pss' : 'pkcs1';
    const pk = await S.importKey('jwk', D.pub, alg, true, ['verify']);
    const sp = alg.name === 'RSA-PSS' ? { name: 'RSA-PSS', saltLength: 32 } : alg;
    for (const [k, der] of Object.entries(D.v)) {
      r[tag + '.' + k] = await face(async () => {
        const sk = await S.importKey('pkcs8', b64(der), alg, true, ['sign']);
        const sig = await S.sign(sp, sk, msg);
        const j = await S.exportKey('jwk', sk);
        return 'ok:' + sk.algorithm.modulusLength + ':' + (await S.verify(sp, pk, sig, msg)) + ':' + (j.dp === undefined ? '-' : 'jwk');
      });
    }
  }
  r['oaep.dpWrong'] = await face(async () => { await S.importKey('pkcs8', b64(D.v.dpWrong), { name: 'RSA-OAEP', hash: 'SHA-256' }, true, ['decrypt']); });
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "pss.ok": "ok:1024:true:jwk",
 "pss.dpWrong": "DataError: ",
 "pss.dqWrong": "DataError: ",
 "pss.qiWrong": "DataError: ",
 "pss.qiZero": "DataError: ",
 "pss.dWrong": "DataError: ",
 "pss.nWrong": "DataError: ",
 "pss.eWrong": "DataError: ",
 "pss.swapPQ": "ok:1024:true:jwk",
 "pss.swapPQraw": "DataError: ",
 "pss.dpPlusPm1": "DataError: ",
 "pss.version1": "DataError: ",
 "pss.garbage": "DataError: ",
 "pss.truncated": "DataError: ",
 "pss.trailing": "DataError: ",
 "pkcs1.ok": "ok:1024:true:jwk",
 "pkcs1.dpWrong": "DataError: ",
 "pkcs1.dqWrong": "DataError: ",
 "pkcs1.qiWrong": "DataError: ",
 "pkcs1.qiZero": "DataError: ",
 "pkcs1.dWrong": "DataError: ",
 "pkcs1.nWrong": "DataError: ",
 "pkcs1.eWrong": "DataError: ",
 "pkcs1.swapPQ": "ok:1024:true:jwk",
 "pkcs1.swapPQraw": "DataError: ",
 "pkcs1.dpPlusPm1": "DataError: ",
 "pkcs1.version1": "DataError: ",
 "pkcs1.garbage": "DataError: ",
 "pkcs1.truncated": "DataError: ",
 "pkcs1.trailing": "DataError: ",
 "oaep.dpWrong": "DataError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_pss_jwk_like_chrome() {
    // RSA-PSS jwk import/export against Chrome 157 (headless, file://; the
    // expected object is its output for this exact script, fixed 1024-bit
    // test key): export member order, alg PS1..PS512, key_ops, ext, byte-exact
    // members; import faces for alg families, use, key_ops (subset, dup),
    // ext, kty, missing/garbled members, missing CRT members, usages. Two
    // faces were ours alone until this pinned them: the non-extractable
    // export message, and a jwk whose dp/dq/qi don't match d/p/q (Chrome
    // refuses with an empty DataError; we recomputed and accepted).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {  const r = {};

  const S = crypto.subtle;
  const K = {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true};
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : (typeof v === 'string' ? v : 'ok:' + JSON.stringify(v)); } catch (e) { return e.name + ': ' + e.message; } };
  const PSS = (h) => ({ name: 'RSA-PSS', hash: h });
  const pub = { kty: 'RSA', n: K.n, e: K.e };
  const priv = Object.assign({}, K); delete priv.ext;
  const W = (base, extra) => Object.assign({}, base, extra);
  const drop = (base, m) => { const o = Object.assign({}, base); delete o[m]; return o; };
  const imp = (jwk, h, ex, us) => face(async () => { const k = await S.importKey('jwk', jwk, PSS(h), ex, us); return k.type + '/' + k.extractable + '/' + k.usages + '/' + k.algorithm.hash.name + '/' + k.algorithm.modulusLength; });
  // export shapes
  const skE = await S.importKey('jwk', priv, PSS('SHA-256'), true, ['sign']);
  const pkE = await S.importKey('jwk', pub, PSS('SHA-384'), true, ['verify']);
  r.privExport = await face(async () => { const j = await S.exportKey('jwk', skE); return JSON.stringify({ keys: Object.keys(j), alg: j.alg, key_ops: j.key_ops, ext: j.ext, same: ['n','e','d','p','q','dp','dq','qi'].every(m => j[m] === K[m]) }); });
  r.pubExport = await face(async () => { const j = await S.exportKey('jwk', pkE); return JSON.stringify({ keys: Object.keys(j), alg: j.alg, key_ops: j.key_ops, ext: j.ext, same: j.n === K.n && j.e === K.e }); });
  r.pubFromPriv = await face(async () => { const j = await S.exportKey('jwk', await S.importKey('jwk', W(pub, {}), PSS('SHA-512'), true, [])); return JSON.stringify({ keys: Object.keys(j), alg: j.alg, key_ops: j.key_ops }); });
  r.privNoExtract = await face(async () => { const k = await S.importKey('jwk', priv, PSS('SHA-256'), false, ['sign']); return S.exportKey('jwk', k).then(() => 'exported'); });
  // import faces
  for (const h of ['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512']) r['algOk.' + h] = await imp(W(pub, { alg: 'PS' + (h === 'SHA-1' ? '1' : h.slice(4)) }), h, true, ['verify']);
  r.noAlg = await imp(pub, 'SHA-256', true, ['verify']);
  r.algRS = await imp(W(pub, { alg: 'RS256' }), 'SHA-256', true, ['verify']);
  r.algOAEP = await imp(W(pub, { alg: 'RSA-OAEP-256' }), 'SHA-256', true, ['verify']);
  r.algBogus = await imp(W(pub, { alg: 'PS999' }), 'SHA-256', true, ['verify']);
  r.algLower = await imp(W(pub, { alg: 'ps256' }), 'SHA-256', true, ['verify']);
  r.useSig = await imp(W(pub, { use: 'sig' }), 'SHA-256', true, ['verify']);
  r.useEnc = await imp(W(pub, { use: 'enc' }), 'SHA-256', true, ['verify']);
  r.keyOpsOk = await imp(W(pub, { key_ops: ['verify'] }), 'SHA-256', true, ['verify']);
  r.keyOpsMiss = await imp(W(pub, { key_ops: ['sign'] }), 'SHA-256', true, ['verify']);
  r.keyOpsSuper = await imp(W(priv, { key_ops: ['sign', 'verify'] }), 'SHA-256', true, ['sign']);
  r.keyOpsDup = await imp(W(pub, { key_ops: ['verify', 'verify'] }), 'SHA-256', true, ['verify']);
  r.extFalse = await imp(W(pub, { ext: false }), 'SHA-256', true, ['verify']);
  r.extFalseOk = await imp(W(pub, { ext: false }), 'SHA-256', false, ['verify']);
  r.ktyEC = await imp(W(pub, { kty: 'EC' }), 'SHA-256', true, ['verify']);
  r.noN = await imp(drop(pub, 'n'), 'SHA-256', true, ['verify']);
  r.noE = await imp(drop(pub, 'e'), 'SHA-256', true, ['verify']);
  r.badB64 = await imp(W(pub, { n: K.n.slice(0, -2) + '+/' }), 'SHA-256', true, ['verify']);
  r.nNum = await imp(W(pub, { n: 12345 }), 'SHA-256', true, ['verify']);
  for (const m of ['p', 'q', 'dp', 'dq', 'qi']) r['privNo.' + m] = await imp(drop(priv, m), 'SHA-256', true, ['sign']);
  r.privOnlyD = await imp({ kty: 'RSA', n: K.n, e: K.e, d: K.d }, 'SHA-256', true, ['sign']);
  r.privVerifyUsage = await imp(priv, 'SHA-256', true, ['verify']);
  r.pubSignUsage = await imp(pub, 'SHA-256', true, ['sign']);
  r.privEmptyUsage = await imp(priv, 'SHA-256', true, []);
  r.pubEmptyUsage = await imp(pub, 'SHA-256', true, []);
  r.oth = await imp(W(priv, { oth: [] }), 'SHA-256', true, ['sign']);
  r.badDp = await imp(W(priv, { dp: K.dq }), 'SHA-256', true, ['sign']);
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "privExport": "{\"keys\":[\"alg\",\"d\",\"dp\",\"dq\",\"e\",\"ext\",\"key_ops\",\"kty\",\"n\",\"p\",\"q\",\"qi\"],\"alg\":\"PS256\",\"key_ops\":[\"sign\"],\"ext\":true,\"same\":true}",
 "pubExport": "{\"keys\":[\"alg\",\"e\",\"ext\",\"key_ops\",\"kty\",\"n\"],\"alg\":\"PS384\",\"key_ops\":[\"verify\"],\"ext\":true,\"same\":true}",
 "pubFromPriv": "{\"keys\":[\"alg\",\"e\",\"ext\",\"key_ops\",\"kty\",\"n\"],\"alg\":\"PS512\",\"key_ops\":[]}",
 "privNoExtract": "InvalidAccessError: Failed to execute 'exportKey' on 'SubtleCrypto': key is not extractable",
 "algOk.SHA-1": "public/true/verify/SHA-1/1024",
 "algOk.SHA-256": "public/true/verify/SHA-256/1024",
 "algOk.SHA-384": "public/true/verify/SHA-384/1024",
 "algOk.SHA-512": "public/true/verify/SHA-512/1024",
 "noAlg": "public/true/verify/SHA-256/1024",
 "algRS": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "algOAEP": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "algBogus": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "algLower": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "useSig": "public/true/verify/SHA-256/1024",
 "useEnc": "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "keyOpsOk": "public/true/verify/SHA-256/1024",
 "keyOpsMiss": "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested",
 "keyOpsSuper": "private/true/sign/SHA-256/1024",
 "keyOpsDup": "DataError: The \"key_ops\" member of the JWK dictionary contains duplicate usages.",
 "extFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "extFalseOk": "public/false/verify/SHA-256/1024",
 "ktyEC": "DataError: The JWK \"kty\" member was not \"RSA\"",
 "noN": "DataError: The required JWK member \"n\" was missing",
 "noE": "DataError: The required JWK member \"e\" was missing",
 "badB64": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "nNum": "DataError: The JWK member \"n\" could not be base64url decoded or contained padding",
 "privNo.p": "DataError: The required JWK member \"p\" was missing",
 "privNo.q": "DataError: The required JWK member \"q\" was missing",
 "privNo.dp": "DataError: The required JWK member \"dp\" was missing",
 "privNo.dq": "DataError: The required JWK member \"dq\" was missing",
 "privNo.qi": "DataError: The required JWK member \"qi\" was missing",
 "privOnlyD": "DataError: The required JWK member \"p\" was missing",
 "privVerifyUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "pubSignUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "privEmptyUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "pubEmptyUsage": "public/true//SHA-256/1024",
 "oth": "private/true/sign/SHA-256/1024",
 "badDp": "DataError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_pss_like_chrome() {
    // RSA-PSS used to fall through to the OAEP face (encrypt/decrypt usages,
    // RSA-OAEP jwk algs, no sign/verify at all). With a fixed 1024-bit test
    // key (generated for this test only) the script walks sign/verify at
    // salt 0/20/32/94 (94 = emLen - hLen - 2, the SHA-256 ceiling) and past
    // it, saltLength conversion faces, key/usage/algorithm mismatches, jwk
    // alg mapping (PS1..PS512), spki/pkcs8 OIDs and keygen. Expected =
    // Chrome 157's output for this exact script (headless, file://). Then
    // five signatures Chrome itself produced over the same message (every
    // hash, salts 0..62) must verify here; our signatures were checked to
    // verify in Chrome the same way when this landed.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
  const S = crypto.subtle, enc = new TextEncoder();
  const K = {"jwk": {"kty": "RSA", "n": "z_1dpivZhivMVP0wpXd7Syp6oYK-EhI-pcJxZdO8iHmBwAzuXhlJ3ooS88sEsDeJvPQLjcPZU9CPMpeoPgCJ48SnFV-_-xH-goha7dsuZYYIuKzcf_0dcP8NTRGKovTfI_pxp5z5wNoG2tWwTzwxphH8OAxPZYJRswCwr2OZzMc", "e": "AQAB", "d": "M5zVC3JgNVpTB1gZJUaEEwAElGXEUumyICIjT7h-VYOJfiX3VOSDnzS2RUdA75WomXoaD-Dk4laj4lMeDdzGWouAYPzf1QcIEmwYaIq4SoK3Uap0a2dvdeLZ-Fw9QJ4No7CF_mbXX_Un2acxMQD_qCZ7agYPZQpWqB7akOjvvIE", "p": "3At39u3sfaQ5wx2ROVBQJxl_Yr9G3_d7enhXECVD-GlkLCYDh6K48KE8vAcywyk4OWQMAz9IWrw6CAfIbAK4Bw", "q": "8fmhMvj4izym7pg536OJwvUCs9C5C5lSVtMGf73KuvDZkluWiGFsXTFP-cl43vZ_WSDsjyf11lEJwFeuWouVQQ", "dp": "Pi3DdXRf6lT4uYjmW5K1Q4cx9t5kEa-mJcJRn89X-J8XXl5TiwVRSzXUqrFCB-Hi8Fy0BErikDk2EMTaPqM6Cw", "dq": "O7xnqkDCgwUnJNHZv7cMxF9eGmwVsqJYLG0ONqzxBnN9Et-fau26JcuNtdmiCFEMIUG6hKDV1OHOqXg758h7wQ", "qi": "zH8pdbylxquAq-XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog_0cBygPb1L0cQo9yf-dkgg", "ext": true}, "spki": "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDP/V2mK9mGK8xU/TCld3tLKnqhgr4SEj6lwnFl07yIeYHADO5eGUneihLzywSwN4m89AuNw9lT0I8yl6g+AInjxKcVX7/7Ef6CiFrt2y5lhgi4rNx//R1w/w1NEYqi9N8j+nGnnPnA2gba1bBPPDGmEfw4DE9lglGzALCvY5nMxwIDAQAB", "pkcs8": "MIICdgIBADANBgkqhkiG9w0BAQEFAASCAmAwggJcAgEAAoGBAM/9XaYr2YYrzFT9MKV3e0sqeqGCvhISPqXCcWXTvIh5gcAM7l4ZSd6KEvPLBLA3ibz0C43D2VPQjzKXqD4AiePEpxVfv/sR/oKIWu3bLmWGCLis3H/9HXD/DU0RiqL03yP6caec+cDaBtrVsE88MaYR/DgMT2WCUbMAsK9jmczHAgMBAAECgYAznNULcmA1WlMHWBklRoQTAASUZcRS6bIgIiNPuH5Vg4l+JfdU5IOfNLZFR0DvlaiZehoP4OTiVqPiUx4N3MZai4Bg/N/VBwgSbBhoirhKgrdRqnRrZ2914tn4XD1Ang2jsIX+Ztdf9SfZpzExAP+oJntqBg9lClaoHtqQ6O+8gQJBANwLd/bt7H2kOcMdkTlQUCcZf2K/Rt/3e3p4VxAlQ/hpZCwmA4eiuPChPLwHMsMpODlkDAM/SFq8OggHyGwCuAcCQQDx+aEy+PiLPKbumDnfo4nC9QKz0LkLmVJW0wZ/vcq68NmSW5aIYWxdMU/5yXje9n9ZIOyPJ/XWUQnAV65ai5VBAkA+LcN1dF/qVPi5iOZbkrVDhzH23mQRr6YlwlGfz1f4nxdeXlOLBVFLNdSqsUIH4eLwXLQESuKQOTYQxNo+ozoLAkA7vGeqQMKDBSck0dm/twzEX14abBWyolgsbQ42rPEGc30S359q7boly4212aIIUQwhQbqEoNXU4c6peDvnyHvBAkEAzH8pdbylxquAq+XSwILIwO4lhOC5YAyYNZgpNS6Z8BnSxInNwHXlH4rD9g0WfZog/0cBygPb1L0cQo9yf+dkgg=="};
  const hex = (b) => Array.from(new Uint8Array(b), x => x.toString(16).padStart(2, '0')).join('');
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : (typeof v === 'string' ? v : 'ok:' + JSON.stringify(v)); } catch (e) { return e.name + ': ' + e.message; } };
  const PSS = (h) => ({ name: 'RSA-PSS', hash: h });
  const P = (s) => ({ name: 'RSA-PSS', saltLength: s });
  const pubJwk = { kty: 'RSA', n: K.jwk.n, e: K.jwk.e, ext: true };
  const sk = await S.importKey('jwk', K.jwk, PSS('SHA-256'), true, ['sign']);
  const pk = await S.importKey('jwk', pubJwk, PSS('SHA-256'), true, ['verify']);
  const msg = enc.encode('diting-pss');
  const r = {};
  r.skAlg = JSON.stringify(sk.algorithm.name + '/' + sk.algorithm.hash.name + '/' + sk.algorithm.modulusLength + '/' + sk.usages);
  for (const s of [0, 20, 32, 94]) r['sign' + s] = await face(async () => { const sig = await S.sign(P(s), sk, msg); return 'ok:' + sig.byteLength + ':' + (await S.verify(P(s), pk, sig, msg)) + ':' + (await S.verify(P(s === 20 ? 32 : 20), pk, sig, msg)); });
  r.sign95 = await face(() => S.sign(P(95), sk, msg));
  r.signBig = await face(() => S.sign(P(4096), sk, msg));
  r.saltNeg = await face(() => S.sign(P(-1), sk, msg));
  r.saltNaN = await face(() => S.sign(P(NaN), sk, msg));
  r.saltMissing = await face(() => S.sign({ name: 'RSA-PSS' }, sk, msg));
  r.verifyMissing = await face(() => S.verify({ name: 'RSA-PSS' }, pk, new Uint8Array(128), msg));
  r.verifyNeg = await face(() => S.verify(P(-1), pk, new Uint8Array(128), msg));
  r.verify95 = await face(async () => String(await S.verify(P(95), pk, new Uint8Array(128), msg)));
  r.verifyShortSig = await face(async () => String(await S.verify(P(32), pk, new Uint8Array(5), msg)));
  r.signWithPub = await face(() => S.sign(P(32), pk, msg));
  r.verifyWithPriv = await face(() => S.verify(P(32), sk, new Uint8Array(128), msg));
  r.wrongAlgName = await face(() => S.sign('RSASSA-PKCS1-v1_5', sk, msg));
  r.jwkAlgs = await face(async () => { const out = []; for (const h of ['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512']) { const k = await S.importKey('jwk', pubJwk, PSS(h), true, ['verify']); out.push((await S.exportKey('jwk', k)).alg); } return out.join(','); });
  r.jwkAlgMismatch = await face(() => S.importKey('jwk', Object.assign({}, pubJwk, { alg: 'PS384' }), PSS('SHA-256'), true, ['verify']));
  r.jwkAlgRS = await face(() => S.importKey('jwk', Object.assign({}, pubJwk, { alg: 'RS256' }), PSS('SHA-256'), true, ['verify']));
  r.spkiOid = await face(async () => { const h = hex(await S.exportKey('spki', pk)); return h.includes('2a864886f70d010101') ? 'rsaEncryption' : h.includes('2a864886f70d01010a') ? 'rsassaPss' : '?'; });
  r.pkcs8Oid = await face(async () => { const h = hex(await S.exportKey('pkcs8', sk)); return h.includes('2a864886f70d010101') ? 'rsaEncryption' : '?'; });
  r.genUsageBad = await face(() => S.generateKey({ name: 'RSA-PSS', modulusLength: 1024, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, ['encrypt']));
  r.genEmpty = await face(() => S.generateKey({ name: 'RSA-PSS', modulusLength: 1024, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, []));
  r.gen = await face(async () => { const p = await S.generateKey({ name: 'RSA-PSS', modulusLength: 1024, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-384' }, true, ['sign', 'verify']); const sig = await S.sign(P(48), p.privateKey, msg); return p.publicKey.type + '/' + p.publicKey.usages + '/' + p.privateKey.usages + '/' + p.publicKey.algorithm.hash.name + '/' + (await S.verify(P(48), p.publicKey, sig, msg)); });
  r.importSpkiSign = await face(async () => { await S.importKey('spki', await S.exportKey('spki', pk), PSS('SHA-256'), true, ['sign']); });
  r.importPkcs8Verify = await face(async () => { await S.importKey('pkcs8', await S.exportKey('pkcs8', sk), PSS('SHA-256'), true, ['verify']); });
  r.crossPkcs1Key = await face(async () => { const k = await S.importKey('jwk', pubJwk, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, true, ['verify']); return String(await S.verify(P(32), k, new Uint8Array(128), msg)); });
  // Chrome 157's own PSS signatures over msg with this key (PSS is salted,
  // so bytes can't be pinned — cross-verification is the parity check).
  const CHROME_SIGS = {"SHA-1/20":"04451a823df49ff5a1f42274ad657b7f246b350eb3dd17ba8680e0136d66214229048f246ad96995c79ef0a052f36c37d0831c0daeccb6e1c17ad11ca255e0a0d36e645f84e1c45ea7badab78cc45659099b9f8914dd1c9e7dc94589d342d259cca53df8fb4e8f757f6bf2e33b6d6b34983d2f8035409a835e38fe6619405062","SHA-256/32":"62a9dfaa7fcc556957c7b056ae40c4178130d17dd05e3605932fd5c281d5a327ec80ba158bb97797c5d267ca0ad1345199d8506efb71b4ebbde7a444ca8d1160d2e12e5446d464735adc398eea6931ee38946cb5652fcdee75417c9c1f365fa713bf6948b20667b1e3e4c2855186b0d9e1c40ff7799351d50a4e2a09462fda29","SHA-256/0":"a94242e8e16f824b38d8b0151c06d07aa3062cf1ba9b777f91f91da204f5b8f753967d02980dedaed50f1af318cc32b2a45f664b479feb082e4f3c2933246c4977bba816bd02456e1337d2d4a17ce241dccbc8807b9b84fa3d939e840f6c9a13292e86cb52be5bf0ba53eab3f017492920ab39af3a44105bd444e69c8a8b7e60","SHA-384/48":"8a45ba878bc6ecec2aaa3ed97d209c2414fbf73221a40c817cb955ce6a20aef78b094fd427e792ab378414b073fd0d1edb4cca6494fca9e11cb831bdcc68fdd41d2b1932f24eeb2bfaba32cd5f2f7f356621f9ab6d13d910a3d78ec6917dbacb09c2d88b68cd8bb08c24f1be97cdb2ec272dd5acb08427a535a92190ad4446d1","SHA-512/62":"1ada5ac67b7b3b352c8244dd7d8d1126cded3d8f2c9602f250155c46f2348e4f98ff54cef657f5372ea8e304792b08864c724a8dfb96d99724a0b4a8188361c29afff5450cd63e0c597a174490a7c38fafd9c320e810558b819655b66406fa76ef7a709cbd68e6e8c5b89b4415cb2a1242087369a5662f8561e0491495068bb3"};
  const unhex = (h) => Uint8Array.from(h.match(/../g), x => parseInt(x, 16));
  for (const [k, sig] of Object.entries(CHROME_SIGS)) {
    const [h, s] = k.split('/');
    const v = await S.importKey('jwk', pubJwk, PSS(h), true, ['verify']);
    r['chromeSig.' + k] = String(await S.verify(P(+s), v, unhex(sig), msg));
  }
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "skAlg": "\"RSA-PSS/SHA-256/1024/sign\"",
 "sign0": "ok:128:true:false",
 "sign20": "ok:128:true:false",
 "sign32": "ok:128:true:false",
 "sign94": "ok:128:true:false",
 "sign95": "OperationError: ",
 "signBig": "OperationError: ",
 "saltNeg": "TypeError: Failed to execute 'sign' on 'SubtleCrypto': RsaPssParams: saltLength: Outside of numeric range",
 "saltNaN": "TypeError: Failed to execute 'sign' on 'SubtleCrypto': RsaPssParams: saltLength: Is not a number",
 "saltMissing": "TypeError: Failed to execute 'sign' on 'SubtleCrypto': RsaPssParams: saltLength: Missing required property",
 "verifyMissing": "TypeError: Failed to execute 'verify' on 'SubtleCrypto': RsaPssParams: saltLength: Missing required property",
 "verifyNeg": "TypeError: Failed to execute 'verify' on 'SubtleCrypto': RsaPssParams: saltLength: Outside of numeric range",
 "verify95": "false",
 "verifyShortSig": "false",
 "signWithPub": "InvalidAccessError: key.usages does not permit this operation",
 "verifyWithPriv": "InvalidAccessError: key.usages does not permit this operation",
 "wrongAlgName": "InvalidAccessError: key.algorithm does not match that of operation",
 "jwkAlgs": "PS1,PS256,PS384,PS512",
 "jwkAlgMismatch": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "jwkAlgRS": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "spkiOid": "rsaEncryption",
 "pkcs8Oid": "rsaEncryption",
 "genUsageBad": "SyntaxError: Cannot create a key using the specified key usages.",
 "genEmpty": "SyntaxError: Usages cannot be empty when creating a key.",
 "gen": "public/verify/sign/SHA-384/true",
 "importSpkiSign": "SyntaxError: Cannot create a key using the specified key usages.",
 "importPkcs8Verify": "SyntaxError: Cannot create a key using the specified key usages.",
 "crossPkcs1Key": "InvalidAccessError: key.algorithm does not match that of operation",
 "chromeSig.SHA-1/20": "true",
 "chromeSig.SHA-256/32": "true",
 "chromeSig.SHA-256/0": "true",
 "chromeSig.SHA-384/48": "true",
 "chromeSig.SHA-512/62": "true"
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_rsa_512_to_1023_bits_like_chrome() {
    // rsa's default floor is 1024 bits; Chrome 157 (BoringSSL) takes RSA from
    // 512. For 256/384/504/512/520/768-bit keys (fixed test keys, generated
    // for this test only) the script imports jwk private / jwk public / spki /
    // pkcs8, signs+verifies and runs an OAEP-SHA-1 round trip; then keygen at
    // seven sizes. Expected = Chrome 157's output for this exact script
    // (headless, file://): under 512 an empty DataError (OperationError for a
    // jwk public key); keygen rounds down to a multiple of 128 and reports
    // what it made (520 -> 512, 1016 -> 896, 1032 -> 1024). We used to refuse
    // every 512-1023 key, accept sub-512 private keys, and generate 1032 as
    // 1032.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
  const S = crypto.subtle, enc = new TextEncoder();
  const KEYS = {"256": {"jwk": {"kty": "RSA", "n": "wUe4fClKoVWmx6YxVIaAD7khSVvGa3JNd-xHb7agjE0", "e": "AQAB", "d": "rPshc44JCwd7Mqfc8odFyDxDK99x_vcTQm_9kpI0xgk", "p": "91DAPoOv9bnWQP0CbzVFKw", "q": "yBE302CuKUg2tsl39KkoZw", "dp": "vvtjyaLF0fsVYhA_vgMkCw", "dq": "rs2EdlaZtQ4YaBqdiZIm7w", "qi": "VMKANKb0NCqbVqGwa7aPeQ", "ext": true}, "spki": "MDwwDQYJKoZIhvcNAQEBBQADKwAwKAIhAMFHuHwpSqFVpsemMVSGgA+5IUlbxmtyTXfsR2+2oIxNAgMBAAE=", "pkcs8": "MIHEAgEAMA0GCSqGSIb3DQEBAQUABIGvMIGsAgEAAiEAwUe4fClKoVWmx6YxVIaAD7khSVvGa3JNd+xHb7agjE0CAwEAAQIhAKz7IXOOCQsHezKn3PKHRcg8Qyvfcf73E0Jv/ZKSNMYJAhEA91DAPoOv9bnWQP0CbzVFKwIRAMgRN9NgrilINrbJd/SpKGcCEQC++2PJosXR+xViED++AyQLAhEArs2EdlaZtQ4YaBqdiZIm7wIQVMKANKb0NCqbVqGwa7aPeQ=="}, "384": {"jwk": {"kty": "RSA", "n": "p7J4Q6nunb4V-TELvwnnj2TKmEqa_Ii5g7oHyeCeC6vvOR2Zu--K7oUu17qwsD7H", "e": "AQAB", "d": "PEvDSjDzhuwt4mdsHchl5f0Liy5oA5z04fshpijtzp-3AD9IMaEmNb-cRSvfcgxh", "p": "1uWtx4rgcft6jRgPJ7cttiE-gMqeq_W1", "q": "x8Wq3e8kHkHivyk2ySKYKKO14fPNB_AL", "dp": "OvaeLgge-WdlAt29GsqaEkUg3zm5O7mZ", "dq": "uFkbRTQSE23fTWI98TvgR9hS1lDBympj", "qi": "f7qg732sbsyzCNXFkBGoX2MTzXNIKxpk", "ext": true}, "spki": "MEwwDQYJKoZIhvcNAQEBBQADOwAwOAIxAKeyeEOp7p2+FfkxC78J549kyphKmvyIuYO6B8ngngur7zkdmbvviu6FLte6sLA+xwIDAQAB", "pkcs8": "MIIBCgIBADANBgkqhkiG9w0BAQEFAASB9TCB8gIBAAIxAKeyeEOp7p2+FfkxC78J549kyphKmvyIuYO6B8ngngur7zkdmbvviu6FLte6sLA+xwIDAQABAjA8S8NKMPOG7C3iZ2wdyGXl/QuLLmgDnPTh+yGmKO3On7cAP0gxoSY1v5xFK99yDGECGQDW5a3HiuBx+3qNGA8nty22IT6Ayp6r9bUCGQDHxard7yQeQeK/KTbJIpgoo7Xh880H8AsCGDr2ni4IHvlnZQLdvRrKmhJFIN85uTu5mQIZALhZG0U0EhNt301iPfE74EfYUtZQwcpqYwIYf7qg732sbsyzCNXFkBGoX2MTzXNIKxpk"}, "504": {"jwk": {"kty": "RSA", "n": "56aQAxwTwiL8FXXvkNp28yqLhOLYIk32kNy-Het5uCuKO5NO1QnTCL8wCR4z6ndsTD5rPy7JrVdhs2UG72Bl", "e": "AQAB", "d": "cA1UufxZ3l133JWopUPXp_YbeNMVwzo9G4N7fxOJVCsYYOX20YxV85cQ-gJyIQhA9eB4Dwcgt4D4Z8LI4upB", "p": "D6yomJT2XQm8NGpONLlBMQlflPf0Ua5w3IGbKOApLBE", "q": "DsdkEwslKAFyvzW2O6EvdCA4W5ZcMS6YmWa9KJAxkxU", "dp": "Cwt68allwEFv4O9XlEdF7KvSMkJWYogs9vRMxQzPdkE", "dq": "DCpAzBQuf6HBjHdwC-GAKDylZ257cNiEMDGLnyoiRIk", "qi": "A0QuFuWubZ9yKMMto53YsKZ3yewurgvkZ_8S1Mt2X9Y", "ext": true}, "spki": "MFswDQYJKoZIhvcNAQEBBQADSgAwRwJAAOemkAMcE8Ii/BV175DadvMqi4Ti2CJN9pDcvh3rebgrijuTTtUJ0wi/MAkeM+p3bEw+az8uya1XYbNlBu9gZQIDAQAB", "pkcs8": "MIIBTwIBADANBgkqhkiG9w0BAQEFAASCATkwggE1AgEAAkAA56aQAxwTwiL8FXXvkNp28yqLhOLYIk32kNy+Het5uCuKO5NO1QnTCL8wCR4z6ndsTD5rPy7JrVdhs2UG72BlAgMBAAECP3ANVLn8Wd5dd9yVqKVD16f2G3jTFcM6PRuDe38TiVQrGGDl9tGMVfOXEPoCciEIQPXgeA8HILeA+GfCyOLqQQIgD6yomJT2XQm8NGpONLlBMQlflPf0Ua5w3IGbKOApLBECIA7HZBMLJSgBcr81tjuhL3QgOFuWXDEumJlmvSiQMZMVAiALC3rxqWXAQW/g71eUR0Xsq9IyQlZiiCz29EzFDM92QQIgDCpAzBQuf6HBjHdwC+GAKDylZ257cNiEMDGLnyoiRIkCIANELhblrm2fcijDLaOd2LCmd8nsLq4L5Gf/EtTLdl/W"}, "512": {"jwk": {"kty": "RSA", "n": "reMJvT0oiWNpoOLCJIuiOoep59SHfoRbMtUTbgVGmDcGA7nOtHaqMBwq-7iVzW7KpOFYU0anaVOanU4oGp2Y0w", "e": "AQAB", "d": "TfX33N95uDdZGWfxUT9FieZfN_Cimweud9jucCpvXzi2ToTcW8bFnb7h2R0XSJEDpnCTaxg6gL6o61P2ucCNWQ", "p": "5mAbYI8N8Ju1QYT6cFfD0GVG1hpttkegl2g1g_bzvM0", "q": "wTpuX4DoJpzFOEb_qTjaFZEkXj4CISfAx_9J6zFArB8", "dp": "0zbOIutFhpHbzuRSkGmG-CvClE3KxNJK5EYAuqrRsQk", "dq": "KrIPi1i_czC2v02o1ipyvcs5Xv2SmOLgzRBSs_5JDP8", "qi": "F7GU9FQVM1yf5K5JJ1vRYGgCk5TRcDEM5kxHBv4LrEo", "ext": true}, "spki": "MFwwDQYJKoZIhvcNAQEBBQADSwAwSAJBAK3jCb09KIljaaDiwiSLojqHqefUh36EWzLVE24FRpg3BgO5zrR2qjAcKvu4lc1uyqThWFNGp2lTmp1OKBqdmNMCAwEAAQ==", "pkcs8": "MIIBVAIBADANBgkqhkiG9w0BAQEFAASCAT4wggE6AgEAAkEAreMJvT0oiWNpoOLCJIuiOoep59SHfoRbMtUTbgVGmDcGA7nOtHaqMBwq+7iVzW7KpOFYU0anaVOanU4oGp2Y0wIDAQABAkBN9ffc33m4N1kZZ/FRP0WJ5l838KKbB6532O5wKm9fOLZOhNxbxsWdvuHZHRdIkQOmcJNrGDqAvqjrU/a5wI1ZAiEA5mAbYI8N8Ju1QYT6cFfD0GVG1hpttkegl2g1g/bzvM0CIQDBOm5fgOgmnMU4Rv+pONoVkSRePgIhJ8DH/0nrMUCsHwIhANM2ziLrRYaR287kUpBphvgrwpRNysTSSuRGALqq0bEJAiAqsg+LWL9zMLa/TajWKnK9yzle/ZKY4uDNEFKz/kkM/wIgF7GU9FQVM1yf5K5JJ1vRYGgCk5TRcDEM5kxHBv4LrEo="}, "520": {"jwk": {"kty": "RSA", "n": "5Fl4bwZ7eLsMGZtiPEwgnhx8zxIpGeFbkZrt_uTbTemoNfPqK1uGh8DVeB5DkDBSj4fnQY_LKGJyuRX80fp4gSM", "e": "AQAB", "d": "eeMgG_NT2N_O16RPTWw9ROhj3owNEFlwbUpLm3FBdlcgYASQK77Dv0QIX8KTfdc2X0PY0IGwnwDGvDWOIm96rQE", "p": "D__dVIZOajaHjTJZqYDC-ue5vG2t48vjyMcCpT-o9AXj", "q": "DkW2dAm54Pt487ToVYBxIbBOq_HafVjTVemdH7DlZHvB", "dp": "CpQuf0XysU3u4NBq8qW9xnLTn6CRuq5EpavYtvTZvMfr", "dq": "BeO2msO4tKKREuBAT6glByXeEQ5Ifxk8fGNet0kkIAcB", "qi": "BOGQt_wZ46DTARjodJQkgCW5KG_ooyASvDkiaO5n5vIW", "ext": true}, "spki": "MF0wDQYJKoZIhvcNAQEBBQADTAAwSQJCAORZeG8Ge3i7DBmbYjxMIJ4cfM8SKRnhW5Ga7f7k203pqDXz6itbhofA1XgeQ5AwUo+H50GPyyhicrkV/NH6eIEjAgMBAAE=", "pkcs8": "MIIBWAIBADANBgkqhkiG9w0BAQEFAASCAUIwggE+AgEAAkIA5Fl4bwZ7eLsMGZtiPEwgnhx8zxIpGeFbkZrt/uTbTemoNfPqK1uGh8DVeB5DkDBSj4fnQY/LKGJyuRX80fp4gSMCAwEAAQJBeeMgG/NT2N/O16RPTWw9ROhj3owNEFlwbUpLm3FBdlcgYASQK77Dv0QIX8KTfdc2X0PY0IGwnwDGvDWOIm96rQECIQ//3VSGTmo2h40yWamAwvrnubxtrePL48jHAqU/qPQF4wIhDkW2dAm54Pt487ToVYBxIbBOq/HafVjTVemdH7DlZHvBAiEKlC5/RfKxTe7g0Grypb3GctOfoJG6rkSlq9i29Nm8x+sCIQXjtprDuLSikRLgQE+oJQcl3hEOSH8ZPHxjXrdJJCAHAQIhBOGQt/wZ46DTARjodJQkgCW5KG/ooyASvDkiaO5n5vIW"}, "768": {"jwk": {"kty": "RSA", "n": "pnAdqi8K0-Mke6AFu7jwZq2acKLphr9via_ag2y0C-n6trgG3zqdZT-iZ33zYUM2M2-m76ZPYrhQIGDhYh6Z8n4jxIy3Mz5u5gBeiJnSjlw74vl5ZCVMn_LoXCtjEtTZ", "e": "AQAB", "d": "BmemHJh22DSTgNZlohW52pmvqcn7Jp5TUn8b75bNTvAQH9vxF8IPsIfEXiswpHuLmIdqn5NqmPLfbpqevM2_d4-VewaGgzlPWmcajsggQrfdU3aDdzvPsyLgl1hZrZ2p", "p": "ysRG94jeCkMKgaJYklwejrj42PGapLaccS--S_3Sc0-kr8UmuE_IweauHHkR-17r", "q": "0iI5S8Cl45N7eMaEz03svc0utzpSLYIUQwe_gzBMJme9O1dZmW7mB5thNG9r5JJL", "dp": "N1jmSjEo8sIj_wiyhpjwmmKxQn6IiMdz4o12s0WAiYtG19fH3u2gThVKl2kQ2H2r", "dq": "iQ7dhDVhfjBh5oLFeVmAQsQj5krZ8ujXQmk5Vt4XrS0Y1i3BOTyGOQv1uPe7zdhF", "qi": "bVLuC9SSFltiLK2BfiPNDYWQerA0Gy0fq-x-K5BWb9Rh-HSzgdyrz2uuIhiIOZfN", "ext": true}, "spki": "MHwwDQYJKoZIhvcNAQEBBQADawAwaAJhAKZwHaovCtPjJHugBbu48GatmnCi6Ya/b4mv2oNstAvp+ra4Bt86nWU/omd982FDNjNvpu+mT2K4UCBg4WIemfJ+I8SMtzM+buYAXoiZ0o5cO+L5eWQlTJ/y6FwrYxLU2QIDAQAB", "pkcs8": "MIIB5AIBADANBgkqhkiG9w0BAQEFAASCAc4wggHKAgEAAmEApnAdqi8K0+Mke6AFu7jwZq2acKLphr9via/ag2y0C+n6trgG3zqdZT+iZ33zYUM2M2+m76ZPYrhQIGDhYh6Z8n4jxIy3Mz5u5gBeiJnSjlw74vl5ZCVMn/LoXCtjEtTZAgMBAAECYAZnphyYdtg0k4DWZaIVudqZr6nJ+yaeU1J/G++WzU7wEB/b8RfCD7CHxF4rMKR7i5iHap+Tapjy326anrzNv3ePlXsGhoM5T1pnGo7IIEK33VN2g3c7z7Mi4JdYWa2dqQIxAMrERveI3gpDCoGiWJJcHo64+NjxmqS2nHEvvkv90nNPpK/FJrhPyMHmrhx5Efte6wIxANIiOUvApeOTe3jGhM9N7L3NLrc6Ui2CFEMHv4MwTCZnvTtXWZlu5gebYTRva+SSSwIwN1jmSjEo8sIj/wiyhpjwmmKxQn6IiMdz4o12s0WAiYtG19fH3u2gThVKl2kQ2H2rAjEAiQ7dhDVhfjBh5oLFeVmAQsQj5krZ8ujXQmk5Vt4XrS0Y1i3BOTyGOQv1uPe7zdhFAjBtUu4L1JIWW2IsrYF+I80NhZB6sDQbLR+r7H4rkFZv1GH4dLOB3KvPa64iGIg5l80="}};
  const b64 = (s) => Uint8Array.from(atob(s), c => c.charCodeAt(0));
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : (typeof v === 'string' ? v : 'ok:' + JSON.stringify(v)); } catch (e) { return e.name + ': ' + e.message; } };
  const SS = { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, OA = { name: 'RSA-OAEP', hash: 'SHA-1' };
  const r = {};
  for (const bits of Object.keys(KEYS)) {
    const K = KEYS[bits];
    const pubJwk = { kty: 'RSA', n: K.jwk.n, e: K.jwk.e, ext: true };
    r[bits + '.jwkPriv'] = await face(async () => { await S.importKey('jwk', K.jwk, SS, true, ['sign']); });
    r[bits + '.jwkPub'] = await face(async () => { await S.importKey('jwk', pubJwk, SS, true, ['verify']); });
    r[bits + '.spki'] = await face(async () => { const k = await S.importKey('spki', b64(K.spki), SS, true, ['verify']); return 'ok:' + k.algorithm.modulusLength; });
    r[bits + '.pkcs8'] = await face(async () => { const k = await S.importKey('pkcs8', b64(K.pkcs8), SS, true, ['sign']); return 'ok:' + k.algorithm.modulusLength; });
    r[bits + '.signVerify'] = await face(async () => {
      const sk = await S.importKey('jwk', K.jwk, SS, true, ['sign']);
      const pk = await S.importKey('jwk', pubJwk, SS, true, ['verify']);
      const sig = await S.sign(SS, sk, enc.encode('m'));
      return 'ok:' + sig.byteLength + ':' + (await S.verify(SS, pk, sig, enc.encode('m')));
    });
    r[bits + '.oaep'] = await face(async () => {
      const sk = await S.importKey('jwk', K.jwk, OA, true, ['decrypt']);
      const pk = await S.importKey('jwk', pubJwk, OA, true, ['encrypt']);
      const ct = await S.encrypt(OA, pk, enc.encode('hi'));
      return 'ok:' + new TextDecoder().decode(await S.decrypt(OA, sk, ct));
    });
  }
  for (const m of [504, 512, 520, 576, 640, 1016, 1032]) r['gen' + m] = await face(async () => {
    const p = await S.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: m, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, ['sign', 'verify']);
    const sig = await S.sign(SS, p.privateKey, enc.encode('m'));
    return 'ok:' + p.publicKey.algorithm.modulusLength + ':' + sig.byteLength + ':' + (await S.verify(SS, p.publicKey, sig, enc.encode('m')));
  });
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "256.jwkPriv": "DataError: ",
 "256.jwkPub": "OperationError: ",
 "256.spki": "DataError: ",
 "256.pkcs8": "DataError: ",
 "256.signVerify": "DataError: ",
 "256.oaep": "DataError: ",
 "384.jwkPriv": "DataError: ",
 "384.jwkPub": "OperationError: ",
 "384.spki": "DataError: ",
 "384.pkcs8": "DataError: ",
 "384.signVerify": "DataError: ",
 "384.oaep": "DataError: ",
 "504.jwkPriv": "DataError: ",
 "504.jwkPub": "OperationError: ",
 "504.spki": "DataError: ",
 "504.pkcs8": "DataError: ",
 "504.signVerify": "DataError: ",
 "504.oaep": "DataError: ",
 "512.jwkPriv": "ok",
 "512.jwkPub": "ok",
 "512.spki": "ok:512",
 "512.pkcs8": "ok:512",
 "512.signVerify": "ok:64:true",
 "512.oaep": "ok:hi",
 "520.jwkPriv": "ok",
 "520.jwkPub": "ok",
 "520.spki": "ok:520",
 "520.pkcs8": "ok:520",
 "520.signVerify": "ok:65:true",
 "520.oaep": "ok:hi",
 "768.jwkPriv": "ok",
 "768.jwkPub": "ok",
 "768.spki": "ok:768",
 "768.pkcs8": "ok:768",
 "768.signVerify": "ok:96:true",
 "768.oaep": "ok:hi",
 "gen504": "OperationError: ",
 "gen512": "ok:512:64:true",
 "gen520": "ok:512:64:true",
 "gen576": "ok:512:64:true",
 "gen640": "ok:640:80:true",
 "gen1016": "ok:896:112:true",
 "gen1032": "ok:1024:128:true"
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_numeric_members_enforce_range_like_chrome() {
    // Every [EnforceRange] integer member of the WebCrypto algorithm
    // dictionaries — RsaHashedKeyGenParams.modulusLength, Aes{KeyGen,Derived
    // Key}Params.length, Hmac{KeyGen,Import}Params.length, AeadParams.tagLength,
    // AesCtrParams.length — across ten values (negative, NaN, Infinity,
    // explicit undefined, null, a fraction, a numeric string, past the
    // octet/ushort/ulong ceilings) plus absence. The expected object is Chrome
    // 157's own output for this exact script (headless, file://), verbatim:
    // the conversion messages, each algorithm's range face after it, and the
    // wrap/derive method names. The reads used to be ad hoc (`>>> 0`,
    // `x || default`, no check at all on HMAC import length).
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
  const S = crypto.subtle, enc = new TextEncoder();
  const face = async (f) => { try { const v = await f(); return 'ok' + (v && v.byteLength !== undefined ? ':' + v.byteLength : v && v.algorithm ? ':' + JSON.stringify(v.algorithm.length ?? v.algorithm.modulusLength ?? '') : (v && v.privateKey ? ':pair' : '')); } catch (e) { return e.name + ': ' + e.message; } };
  const vals = [['neg', -1], ['nan', NaN], ['inf', Infinity], ['undef', undefined], ['nul', null], ['frac', 256.9], ['str', '256'], ['u16', 65536], ['u8', 256], ['u32', 4294967296]];
  const r = {};
  const run = async (tag, mk, absent) => {
    for (const [k, v] of vals) r[tag + '.' + k] = await face(() => mk(v));
    if (absent) r[tag + '.missing'] = await face(absent);
  };
  await run('aesGen', (v) => S.generateKey({ name: 'AES-GCM', length: v }, true, ['encrypt']), () => S.generateKey({ name: 'AES-GCM' }, true, ['encrypt']));
  await run('hmacGen', (v) => S.generateKey({ name: 'HMAC', hash: 'SHA-256', length: v }, true, ['sign']), null);
  const raw = new Uint8Array(32);
  await run('hmacImp', (v) => S.importKey('raw', raw, { name: 'HMAC', hash: 'SHA-256', length: v }, true, ['sign']), null);
  await run('rsaGen', (v) => S.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: v, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, ['sign']), () => S.generateKey({ name: 'RSASSA-PKCS1-v1_5', publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, ['sign']));
  const gk = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['encrypt', 'decrypt']);
  await run('gcmTag', (v) => S.encrypt({ name: 'AES-GCM', iv: new Uint8Array(12), tagLength: v }, gk, enc.encode('x')), null);
  const ck = await S.generateKey({ name: 'AES-CTR', length: 128 }, true, ['encrypt', 'decrypt']);
  await run('ctrLen', (v) => S.encrypt({ name: 'AES-CTR', counter: new Uint8Array(16), length: v }, ck, enc.encode('x')), () => S.encrypt({ name: 'AES-CTR', counter: new Uint8Array(16) }, ck, enc.encode('x')));
  r['ctrDec.neg'] = await face(() => S.decrypt({ name: 'AES-CTR', counter: new Uint8Array(16), length: -1 }, ck, new Uint8Array(1)));
  const base = await S.importKey('raw', enc.encode('pw'), 'PBKDF2', false, ['deriveKey']);
  const P = { name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('s'), iterations: 1 };
  await run('dkAes', (v) => S.deriveKey(P, base, { name: 'AES-GCM', length: v }, true, ['encrypt']), () => S.deriveKey(P, base, { name: 'AES-GCM' }, true, ['encrypt']));
  await run('dkHmac', (v) => S.deriveKey(P, base, { name: 'HMAC', hash: 'SHA-256', length: v }, true, ['sign']), null);
  for (const L of [192, 100, 512]) r['dkAes' + L] = await face(() => S.deriveKey(P, base, { name: 'AES-GCM', length: L }, true, ['encrypt']));
  for (const m of [256, 504]) r['rsa' + m] = await face(() => S.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: m, publicExponent: new Uint8Array([1,0,1]), hash: 'SHA-256' }, true, ['sign']));
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "aesGen.neg": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Outside of numeric range",
 "aesGen.nan": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Is not a number",
 "aesGen.inf": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Outside of numeric range",
 "aesGen.undef": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Is not a number",
 "aesGen.nul": "OperationError: AES key length must be 128 or 256 bits",
 "aesGen.frac": "ok:256",
 "aesGen.str": "ok:256",
 "aesGen.u16": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Outside of numeric range",
 "aesGen.u8": "ok:256",
 "aesGen.u32": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Outside of numeric range",
 "aesGen.missing": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': AesKeyGenParams: length: Missing required property",
 "hmacGen.neg": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': HmacKeyGenParams: length: Outside of numeric range",
 "hmacGen.nan": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': HmacKeyGenParams: length: Is not a number",
 "hmacGen.inf": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': HmacKeyGenParams: length: Outside of numeric range",
 "hmacGen.undef": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': HmacKeyGenParams: length: Is not a number",
 "hmacGen.nul": "OperationError: HMAC key length must not be zero",
 "hmacGen.frac": "ok:256",
 "hmacGen.str": "ok:256",
 "hmacGen.u16": "ok:65536",
 "hmacGen.u8": "ok:256",
 "hmacGen.u32": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': HmacKeyGenParams: length: Outside of numeric range",
 "hmacImp.neg": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "hmacImp.nan": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: length: Is not a number",
 "hmacImp.inf": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "hmacImp.undef": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: length: Is not a number",
 "hmacImp.nul": "DataError: The optional HMAC key length must be shorter than the key data, and by no more than 7 bits.",
 "hmacImp.frac": "ok:256",
 "hmacImp.str": "ok:256",
 "hmacImp.u16": "DataError: The optional HMAC key length must be shorter than the key data, and by no more than 7 bits.",
 "hmacImp.u8": "ok:256",
 "hmacImp.u32": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "rsaGen.neg": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Outside of numeric range",
 "rsaGen.nan": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Is not a number",
 "rsaGen.inf": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Outside of numeric range",
 "rsaGen.undef": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Is not a number",
 "rsaGen.nul": "OperationError: The modulus length must be a multiple of 8 bits and >= 256 and <= 8192",
 "rsaGen.frac": "OperationError: ",
 "rsaGen.str": "OperationError: ",
 "rsaGen.u16": "OperationError: The modulus length must be a multiple of 8 bits and >= 256 and <= 8192",
 "rsaGen.u8": "OperationError: ",
 "rsaGen.u32": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Outside of numeric range",
 "rsaGen.missing": "TypeError: Failed to execute 'generateKey' on 'SubtleCrypto': RsaHashedKeyGenParams: modulusLength: Missing required property",
 "gcmTag.neg": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.nan": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Is not a number",
 "gcmTag.inf": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.undef": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Is not a number",
 "gcmTag.nul": "OperationError: The tag length is invalid: Must be 32, 64, 96, 104, 112, 120, or 128 bits",
 "gcmTag.frac": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.str": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.u16": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.u8": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "gcmTag.u32": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AeadParams: tagLength: Outside of numeric range",
 "ctrLen.neg": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.nan": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Is not a number",
 "ctrLen.inf": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.undef": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Is not a number",
 "ctrLen.nul": "OperationError: The \"length\" member must be >= 1 and <= 128",
 "ctrLen.frac": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.str": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.u16": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.u8": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.u32": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "ctrLen.missing": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': AesCtrParams: length: Missing required property",
 "ctrDec.neg": "TypeError: Failed to execute 'decrypt' on 'SubtleCrypto': AesCtrParams: length: Outside of numeric range",
 "dkAes.neg": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Outside of numeric range",
 "dkAes.nan": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Is not a number",
 "dkAes.inf": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Outside of numeric range",
 "dkAes.undef": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Is not a number",
 "dkAes.nul": "OperationError: AES key length must be 128 or 256 bits",
 "dkAes.frac": "ok:256",
 "dkAes.str": "ok:256",
 "dkAes.u16": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Outside of numeric range",
 "dkAes.u8": "ok:256",
 "dkAes.u32": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Outside of numeric range",
 "dkAes.missing": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Missing required property",
 "dkHmac.neg": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "dkHmac.nan": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HmacImportParams: length: Is not a number",
 "dkHmac.inf": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "dkHmac.undef": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HmacImportParams: length: Is not a number",
 "dkHmac.nul": "TypeError: HMAC key length must not be zero",
 "dkHmac.frac": "ok:256",
 "dkHmac.str": "ok:256",
 "dkHmac.u16": "ok:65536",
 "dkHmac.u8": "ok:256",
 "dkHmac.u32": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HmacImportParams: length: Outside of numeric range",
 "dkAes192": "OperationError: 192-bit AES keys are not supported",
 "dkAes100": "OperationError: AES key length must be 128 or 256 bits",
 "dkAes512": "OperationError: AES key length must be 128 or 256 bits",
 "rsa256": "OperationError: ",
 "rsa504": "OperationError: "
}"#,
    )
    .unwrap();
    let got = result.value.unwrap();
    for (k, v) in expected.as_object().unwrap() {
        assert_eq!(got.get(k), Some(v), "face {k}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_pbkdf2_iterations_enforce_range_faces() {
    // Pbkdf2Params.iterations is a required [EnforceRange] unsigned long.
    // Every face below is Chrome 157's (headless probe, file://); we used to
    // `>>> 0` it, so -1 became 4294967295 and failed the iteration cap as an
    // OperationError. Members convert salt, iterations, hash — a bad hash
    // still reports the iterations error first.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder();
        const face = async (p) => { try { const v = await p; return 'ok:' + (v && v.byteLength); } catch (e) { return e.name + ': ' + e.message; } };
        const base = await S.importKey('raw', enc.encode('pw'), 'PBKDF2', false, ['deriveBits', 'deriveKey']);
        const P = (it) => ({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('s'), iterations: it });
        const r = {};
        for (const [k, it] of [['neg1', -1], ['big', 4294967296], ['frac', 1.7], ['negfrac', -0.5], ['nan', NaN], ['inf', Infinity],
                               ['str', '3'], ['badstr', 'x'], ['nul', null], ['bool', true], ['obj', {}], ['undef', undefined]]) {
            r[k] = await face(S.deriveBits(P(it), base, 256));
        }
        r.missing = await face(S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('s') }, base, 256));
        r.keyNeg = await face(S.deriveKey(P(-1), base, { name: 'AES-GCM', length: 128 }, true, ['encrypt']));
        r.order = await face(S.deriveBits({ name: 'PBKDF2', hash: 'NOPE', salt: enc.encode('s'), iterations: -1 }, base, 256));
        return r;
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let bits = |m: &str| {
        format!(
            "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': Pbkdf2Params: iterations: {m}"
        )
    };
    let zero = "OperationError: PBKDF2 requires iterations > 0";
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "neg1": bits("Outside of numeric range"),
            "big": bits("Outside of numeric range"),
            "frac": "ok:32",
            "negfrac": zero,
            "nan": bits("Is not a number"),
            "inf": bits("Outside of numeric range"),
            "str": "ok:32",
            "badstr": bits("Is not a number"),
            "nul": zero,
            "bool": "ok:32",
            "obj": bits("Is not a number"),
            "undef": bits("Is not a number"),
            "missing": bits("Missing required property"),
            "keyNeg": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': Pbkdf2Params: iterations: Outside of numeric range",
            "order": bits("Outside of numeric range"),
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_pbkdf2_zero_iterations_is_an_operation_error() {
    // Chrome 157 (headless probe, file://): zero iterations is an
    // OperationError checked before the length — so length 0 fails the same
    // way — and covers deriveKey; one iteration with length 0 is an empty
    // result. We used to derive happily with zero iterations.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => {
        const S = crypto.subtle, enc = new TextEncoder();
        const face = async (p) => { try { const v = await p; return 'ok:' + (v && v.byteLength); } catch (e) { return e.name + ': ' + e.message; } };
        const base = await S.importKey('raw', enc.encode('pw'), 'PBKDF2', false, ['deriveBits', 'deriveKey']);
        const P = (it) => ({ name: 'PBKDF2', hash: 'SHA-256', salt: enc.encode('s'), iterations: it });
        return {
            it0: await face(S.deriveBits(P(0), base, 256)),
            it0len0: await face(S.deriveBits(P(0), base, 0)),
            it1len0: await face(S.deriveBits(P(1), base, 0)),
            it0key: await face(S.deriveKey(P(0), base, { name: 'AES-GCM', length: 128 }, true, ['encrypt'])),
        };
    }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    assert_eq!(
        result.value.unwrap(),
        serde_json::json!({
            "it0": "OperationError: PBKDF2 requires iterations > 0",
            "it0len0": "OperationError: PBKDF2 requires iterations > 0",
            "it1len0": "ok:0",
            "it0key": "OperationError: PBKDF2 requires iterations > 0",
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_rsassa_pkcs1v15_sign_verify() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // #273: RSASSA-PKCS1-v1_5 (JWT RS256) faces pinned from Chrome 155.
    // The parity anchor is a Chrome-generated jwk and its deterministic
    // signature over a fixed payload — PKCS#1 v1.5 has no randomness, so
    // a byte-equal signature is a true cross-engine pin.
    let script = r#"async () => {
            const enc = new TextEncoder();
            const hex = (u8) => Array.from(u8, b => b.toString(16).padStart(2, '0')).join('');
            const face = async (p) => { try { await p; return 'no-throw'; } catch (e) { return e.name + ': ' + e.message; } };
            const S = crypto.subtle;
            const RS = { name: 'RSASSA-PKCS1-v1_5', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' };

            // Chrome-produced key + signature (file:// secure-context probe, 155.0.8059.85).
            const jwk = {
                alg: "RS256",
                d: "A4r4UbUtFzx_3Lvth6-pd7ql8KbX8Cz3nqBKyL0craxc573Pa6a4Rbz1OtwqHcwU9K1Tz0r6j3r5NPeXduDEJctQ3Zkp_tYFihnWE1l5tz3mb5yH3aeUQAbUsBtbUC9feWoKJGQ2rlKcAWYznD-RoUdvA6w-47W5N_ft98ys8OU",
                dp: "lMezCGqwOujUF8CxhvrrwT7_9UOFXVSD9NS5SuxUKK-z9O68EOpPYujkCuAWVOTZAyW1GpXFDa3qhTIBKOFbBw",
                dq: "AQn-NwSMVYvab0tmCoxzyZsdmBZo8KbjgzwAF8bLKwERgm2XUHD0deAltjWDCv9ego4Zv7sVNfaqeNTiZin9-Q",
                e: "AQAB", ext: true, key_ops: ["sign"], kty: "RSA",
                n: "x2NyImdHbX-jD3PNWkkFCET0vo0Li69-pYSKuUy0cFiAaFwlsYyRofXhZxSMr-m9_vaLSwccyiW-QXeTgPwolIJ9_uTR_eK8x7hfWNtaTba7va5u9BTKpgKI94ivic1mkSAEQILne5Oyhy94ZmkEP0_pL0V7sTTab6GZsOPQEpM",
                p: "_VxtcENSa-oznBECyPVJ4l0s1EX4JJ_ypuAJyhBDuTjwMUulWFIB1KvMpTwwKqWUW7Vd67Q-ymlocxUEDpI47w",
                q: "yXcahF1tIvtw9sj23wxfed4RkvAdldW13k5Ce16h7gBjmFAWvxFYfKxfs6weroNp-mc8dsiqcwfnYflbrn9YnQ",
                qi: "eypHU8uJ6aj3DgL1XOlmP-vSdfN8x0s2rer4ejYB1TZkhjoDnzfmTo7qlQRMtwem99HvgPaWNGloFD48c7aRKw",
            };
            const PAYLOAD = 'diting-parity-payload';
            const CHROME_SIG = 'ad6bdc117e0c285dccc4236dda58aafd1b5605a9abbf79da4fbf7bc949b970d5051d6bea1544a7f1182e0d11a3e9385912d0682f44c5a89c9d8d42300e16fdf7b22a3b427ea664719fe3b76d1b0c5624d60188ec2c9de1e6827147fb97f629f4fd8849ff8f7d6dbc5519c02603b6190c87a9576d2027da036f10c92032ea5e87';

            const key = await S.importKey('jwk', jwk, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, true, ['sign']);
            const sig = new Uint8Array(await S.sign('RSASSA-PKCS1-v1_5', key, enc.encode(PAYLOAD)));
            const parity = hex(sig) === CHROME_SIG;

            const pub = await S.importKey('jwk', { kty: 'RSA', n: jwk.n, e: jwk.e }, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, true, ['verify']);
            const vOk = await S.verify('RSASSA-PKCS1-v1_5', pub, sig, enc.encode(PAYLOAD));
            const vOther = await S.verify('RSASSA-PKCS1-v1_5', pub, sig, enc.encode('other'));
            const flipped = sig.slice(); flipped[7] ^= 1;
            const vFlip = await S.verify('RSASSA-PKCS1-v1_5', pub, flipped, enc.encode(PAYLOAD));

            // JWT shape: libraries verify header.payload with a jwk-imported key.
            const signingInput = 'eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJkaXRpbmcifQ';
            const jwtSig = new Uint8Array(await S.sign('RSASSA-PKCS1-v1_5', key, enc.encode(signingInput)));
            const jwtOk = await S.verify('RSASSA-PKCS1-v1_5', pub, jwtSig, enc.encode(signingInput));

            const pair = await S.generateKey(RS, true, ['sign', 'verify']);
            const genShape = [
                pair.publicKey.type, pair.publicKey.usages.join(','),
                pair.privateKey.type, pair.privateKey.usages.join(','),
                pair.publicKey.algorithm.name, pair.publicKey.algorithm.modulusLength,
                pair.publicKey.algorithm.hash.name,
            ];
            const jwkOut = await S.exportKey('jwk', key);
            const jwkOrder = Object.keys(jwkOut);
            const pubOut = await S.exportKey('jwk', pub);
            const algs = [];
            for (const h of ['SHA-1', 'SHA-384', 'SHA-512']) {
                const p = await S.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: 1024, publicExponent: new Uint8Array([1, 0, 1]), hash: h }, true, ['sign', 'verify']);
                algs.push((await S.exportKey('jwk', p.publicKey)).alg);
            }
            // A hash param on sign() is ignored — the key's hash decides.
            const sig2 = new Uint8Array(await S.sign({ name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-1' }, key, enc.encode(PAYLOAD)));
            const hashIgnored = hex(sig2) === CHROME_SIG;

            const errors = {
                signPub: await face(S.sign('RSASSA-PKCS1-v1_5', pair.publicKey, enc.encode('x'))),
                verifyPriv: await face(S.verify('RSASSA-PKCS1-v1_5', pair.privateKey, sig, enc.encode(PAYLOAD))),
                genEmpty: await face(S.generateKey(RS, true, [])),
                genEncrypt: await face(S.generateKey(RS, true, ['encrypt'])),
                importSpkiSign: await face(S.importKey('spki', await S.exportKey('spki', pub), { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, true, ['sign'])),
                jwkAlgMismatch: await face(S.importKey('jwk', pubOut, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-1' }, true, ['verify'])),
            };
            return { parity, sigLen: sig.length, vOk, vOther, vFlip, jwtOk, genShape, jwkOrder, pubAlg: pubOut.alg, algs, hashIgnored, errors };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(
        v["parity"],
        serde_json::json!(true),
        "signature bytes identical to Chrome's over the same key and payload"
    );
    assert_eq!(
        v["sigLen"],
        serde_json::json!(128),
        "1024-bit modulus signature"
    );
    assert_eq!(v["vOk"], serde_json::json!(true));
    assert_eq!(v["vOther"], serde_json::json!(false));
    assert_eq!(
        v["vFlip"],
        serde_json::json!(false),
        "one flipped byte must fail verification"
    );
    assert_eq!(
        v["jwtOk"],
        serde_json::json!(true),
        "JWT signing-input shape (header.payload) verifies"
    );
    assert_eq!(
        v["genShape"],
        serde_json::json!([
            "public",
            "verify",
            "private",
            "sign",
            "RSASSA-PKCS1-v1_5",
            1024,
            "SHA-256"
        ]),
        "generateKey splits usages verify/sign exactly like Chrome"
    );
    assert_eq!(
        v["jwkOrder"],
        serde_json::json!([
            "alg", "d", "dp", "dq", "e", "ext", "key_ops", "kty", "n", "p", "q", "qi"
        ]),
        "private jwk fields in Chrome's alphabetical order"
    );
    assert_eq!(v["pubAlg"], serde_json::json!("RS256"));
    assert_eq!(v["algs"], serde_json::json!(["RS1", "RS384", "RS512"]));
    assert_eq!(
        v["hashIgnored"],
        serde_json::json!(true),
        "sign()'s hash param is ignored, the key's hash decides"
    );
    let e = &v["errors"];
    assert_eq!(
        e["signPub"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation"),
        "Chrome's RSA sign failure wording (not the ECDSA one)"
    );
    assert_eq!(
        e["verifyPriv"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation")
    );
    assert_eq!(
        e["genEmpty"],
        serde_json::json!("SyntaxError: Usages cannot be empty when creating a key.")
    );
    assert_eq!(
        e["genEncrypt"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["importSpkiSign"],
        serde_json::json!("SyntaxError: Cannot create a key using the specified key usages.")
    );
    assert_eq!(
        e["jwkAlgMismatch"],
        serde_json::json!(
            "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call"
        )
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_webcrypto_jwk_oct_field_validation() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // #269: JWK import field validation for symmetric keys — kty/k
    // presence, strict base64url, alg/use/key_ops/ext consistency and
    // Chrome's exact wording, plus the oct jwk export shape.
    let script = r#"async () => {
            const enc = new TextEncoder();
            const face = async (p) => { try { const r = await p; return 'no-throw:' + (r && r.usages ? r.usages.join(',') : ''); } catch (e) { return e.name + ': ' + e.message; } };
            const S = crypto.subtle;
            const k16 = 'MTIzNDU2Nzg5MDEyMzQ1Ng';
            // 32-byte oct key as base64url (HS256's own length).
            const k32 = btoa(String.fromCharCode(...[...Array(32).keys()])).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
            const GCM = { name: 'AES-GCM', length: 128 };
            const imp = (jwk, alg, ext, usages) => S.importKey('jwk', jwk, alg, ext, usages);

            const ok = await imp({ kty: 'oct', k: k16 }, GCM, true, ['encrypt']);
            const errors = {
                kty: await face(imp({ kty: 'RSA', k: k16 }, GCM, true, ['encrypt'])),
                noK: await face(imp({ kty: 'oct' }, GCM, true, ['encrypt'])),
                badB64: await face(imp({ kty: 'oct', k: '!!!!' }, GCM, true, ['encrypt'])),
                padded: await face(imp({ kty: 'oct', k: k16 + '==' }, GCM, true, ['encrypt'])),
                algBad: await face(imp({ kty: 'oct', k: k16, alg: 'A128CBC' }, GCM, true, ['encrypt'])),
                opsConflict: await face(imp({ kty: 'oct', k: k16, key_ops: ['sign'] }, GCM, true, ['encrypt'])),
                opsSubset: await face(imp({ kty: 'oct', k: k16, key_ops: ['encrypt'] }, GCM, true, ['encrypt', 'decrypt'])),
                opsSuperset: await face(imp({ kty: 'oct', k: k16, key_ops: ['encrypt', 'decrypt'] }, GCM, true, ['encrypt'])),
                opsDup: await face(imp({ kty: 'oct', k: k16, key_ops: ['encrypt', 'encrypt'] }, GCM, true, ['encrypt'])),
                opsBogus: await face(imp({ kty: 'oct', k: k16, key_ops: ['bogus'] }, GCM, true, ['encrypt'])),
                useSig: await face(imp({ kty: 'oct', k: k16, use: 'sig' }, GCM, true, ['encrypt'])),
                useEnc: await face(imp({ kty: 'oct', k: k16, use: 'enc' }, GCM, true, ['encrypt'])),
                extFalse: await face(imp({ kty: 'oct', k: k16, ext: false }, GCM, true, ['encrypt'])),
                extFalseOk: await face(imp({ kty: 'oct', k: k16, ext: false }, GCM, false, ['encrypt'])),
                extTrue: await face(imp({ kty: 'oct', k: k16, ext: true }, GCM, false, ['encrypt'])),
                emptyUsages: await face(imp({ kty: 'oct', k: k16 }, GCM, true, [])),
                bogusUsage: await face(imp({ kty: 'oct', k: k16 }, GCM, true, ['nope'])),
                kwAlgBad: await face(imp({ kty: 'oct', k: k16, alg: 'A128GCM' }, { name: 'AES-KW', length: 128 }, true, ['wrapKey'])),
                hmacOk: await face(imp({ kty: 'oct', k: k32, alg: 'HS256' }, { name: 'HMAC', hash: 'SHA-256' }, true, ['sign'])),
                hmacBad: await face(imp({ kty: 'oct', k: k32, alg: 'HS512' }, { name: 'HMAC', hash: 'SHA-256' }, true, ['sign'])),
                hmacNoHash: await face(imp({ kty: 'oct', k: k16 }, { name: 'HMAC' }, true, ['sign'])),
                rawNoLen: await face(S.importKey('raw', enc.encode('0123456789abcdef'), { name: 'AES-GCM' }, true, ['encrypt'])),
                rawLenMismatch: await face(S.importKey('raw', enc.encode('0123456789abcdef'), { name: 'AES-GCM', length: 256 }, true, ['encrypt'])),
                rawEmptyUsages: await face(S.importKey('raw', enc.encode('0123456789abcdef'), GCM, true, [])),
                encNoUsage: await face((async () => {
                    const k = await S.importKey('raw', enc.encode('0123456789abcdef'), GCM, true, ['decrypt']);
                    return S.encrypt({ name: 'AES-GCM', iv: new Uint8Array(12) }, k, new Uint8Array(4));
                })()),
            };

            const g = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['encrypt']);
            const gj = await S.exportKey('jwk', g);
            const h = await S.generateKey({ name: 'HMAC', hash: 'SHA-384' }, true, ['sign', 'verify']);
            const hj = await S.exportKey('jwk', h);
            const cbc = await S.generateKey({ name: 'AES-CBC', length: 256 }, true, ['encrypt']);
            const cj = await S.exportKey('jwk', cbc);
            const jwkWrapRoundtrip = await (async () => {
                // jwk-format wrap under AES-GCM and unwrap back (the jwk+KW
                // path dies on the multiple-of-8 gate, so GCM carries it).
                const wrapKey = await S.generateKey({ name: 'AES-GCM', length: 128 }, true, ['wrapKey', 'unwrapKey']);
                const w = await S.wrapKey('jwk', g, wrapKey, { name: 'AES-GCM', iv: new Uint8Array(12) });
                const back = await S.unwrapKey('jwk', w, wrapKey, { name: 'AES-GCM', iv: new Uint8Array(12) }, GCM, true, ['encrypt']);
                return back.usages.join(',') + ':' + (await S.exportKey('raw', back)).byteLength;
            })();
            return {
                okUsages: ok.usages.join(','), okAlg: ok.algorithm.name, okLen: ok.algorithm.length, okExt: ok.extractable,
                gOrder: Object.keys(gj), gAlg: gj.alg,
                hOrder: Object.keys(hj), hAlg: hj.alg, hOps: hj.key_ops.join(','),
                cAlg: cj.alg, jwkWrapRoundtrip, errors,
            };
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let v = result.value.unwrap();
    assert_eq!(v["okUsages"], serde_json::json!("encrypt"));
    assert_eq!(v["okAlg"], serde_json::json!("AES-GCM"));
    assert_eq!(v["okLen"], serde_json::json!(128));
    assert_eq!(v["okExt"], serde_json::json!(true));
    assert_eq!(
        v["gOrder"],
        serde_json::json!(["alg", "ext", "k", "key_ops", "kty"])
    );
    assert_eq!(v["gAlg"], serde_json::json!("A128GCM"));
    assert_eq!(
        v["hOrder"],
        serde_json::json!(["alg", "ext", "k", "key_ops", "kty"])
    );
    assert_eq!(v["hAlg"], serde_json::json!("HS384"));
    assert_eq!(v["hOps"], serde_json::json!("sign,verify"));
    assert_eq!(v["cAlg"], serde_json::json!("A256CBC"));
    assert_eq!(
        v["jwkWrapRoundtrip"],
        serde_json::json!("encrypt:16"),
        "jwk-format wrap/unwrap under AES-GCM round-trips"
    );
    let e = &v["errors"];
    assert_eq!(
        e["kty"],
        serde_json::json!("DataError: The JWK \"kty\" member was not \"oct\"")
    );
    assert_eq!(
        e["noK"],
        serde_json::json!("DataError: The required JWK member \"k\" was missing")
    );
    assert_eq!(
        e["badB64"],
        serde_json::json!(
            "DataError: The JWK member \"k\" could not be base64url decoded or contained padding"
        )
    );
    assert_eq!(
        e["padded"],
        serde_json::json!(
            "DataError: The JWK member \"k\" could not be base64url decoded or contained padding"
        ),
        "explicit '=' padding rejected"
    );
    assert_eq!(
        e["algBad"],
        serde_json::json!(
            "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call"
        )
    );
    assert_eq!(
        e["opsConflict"],
        serde_json::json!(
            "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(
        e["opsSubset"],
        serde_json::json!(
            "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(
        e["opsSuperset"],
        serde_json::json!("no-throw:encrypt"),
        "a superset keeps the requested usages"
    );
    assert_eq!(
        e["opsDup"],
        serde_json::json!(
            "DataError: The \"key_ops\" member of the JWK dictionary contains duplicate usages."
        )
    );
    assert_eq!(
        e["opsBogus"],
        serde_json::json!(
            "DataError: The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(
        e["useSig"],
        serde_json::json!(
            "DataError: The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested"
        )
    );
    assert_eq!(e["useEnc"], serde_json::json!("no-throw:encrypt"));
    assert_eq!(
        e["extFalse"],
        serde_json::json!(
            "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call"
        )
    );
    assert_eq!(e["extFalseOk"], serde_json::json!("no-throw:encrypt"));
    assert_eq!(
        e["extTrue"],
        serde_json::json!("no-throw:encrypt"),
        "ext:true with extractable:false is fine"
    );
    assert_eq!(
        e["emptyUsages"],
        serde_json::json!("SyntaxError: Usages cannot be empty when creating a key.")
    );
    assert_eq!(
        e["bogusUsage"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': Invalid keyUsages argument"
        )
    );
    assert_eq!(
        e["kwAlgBad"],
        serde_json::json!(
            "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call"
        )
    );
    assert_eq!(e["hmacOk"], serde_json::json!("no-throw:sign"));
    assert_eq!(
        e["hmacBad"],
        serde_json::json!(
            "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call"
        )
    );
    assert_eq!(
        e["hmacNoHash"],
        serde_json::json!(
            "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Missing or not an AlgorithmIdentifier"
        )
    );
    assert_eq!(
        e["rawNoLen"],
        serde_json::json!("no-throw:encrypt"),
        "the length param is derived from the data, not required"
    );
    assert_eq!(
        e["rawLenMismatch"],
        serde_json::json!("no-throw:encrypt"),
        "Chrome ignores a mismatched length param on raw import"
    );
    assert_eq!(
        e["rawEmptyUsages"],
        serde_json::json!("SyntaxError: Usages cannot be empty when creating a key.")
    );
    assert_eq!(
        e["encNoUsage"],
        serde_json::json!("InvalidAccessError: key.usages does not permit this operation")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_get_random_values_and_uuid_from_csprng() {
    let mut rt = setup_runtime("<html><body></body></html>");
    // getRandomValues fills integer typed arrays, randomUUID returns a v4
    // UUID shape, and both reject/fill sensibly.
    let script = r#"() => {
            const u8 = new Uint8Array(32);
            crypto.getRandomValues(u8);
            const nonZero = u8.some(b => b !== 0);
            const uuid = crypto.randomUUID();
            const uuidOk = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(uuid);
            let typeErr = false;
            try { crypto.getRandomValues(new Float64Array(4)); } catch (e) { typeErr = e.name === 'TypeMismatchError'; }
            return [nonZero, uuidOk, typeErr];
        }"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, false)
        .await
        .unwrap();
    assert_eq!(result.value.unwrap(), serde_json::json!([true, true, true]));
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_kdf_params_like_chrome() {
    // HKDF/PBKDF2 against Chrome 157 (headless, file://): HkdfParams and
    // Pbkdf2Params members (required salt/info/hash, BufferSource and hash
    // faces), deriveBits lengths (null, not a multiple of 8, the HKDF
    // 255*HashLen bound, IDL truncation), base-key algorithm/usage checks and
    // their order, deriveKey key lengths, KDF import (raw only, extractable
    // false), export/wrap/generate refusals, and the canonical usage order
    // every CryptoKey reports. Expected = Chrome's output for this exact
    // script. We used to take a missing salt/info as empty, round a 7-bit
    // length up, skip the key checks, and keep usages in caller order.
    // (One face is left out: PBKDF2 iterations 0 with a null length hangs
    // Chrome 157 itself.)
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => { const r = {};
{
  const S = crypto.subtle;
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const hex = (b) => Array.from(new Uint8Array(b), v => v.toString(16).padStart(2, '0')).join('');
  const H = (o) => Object.assign({ name: 'HKDF', hash: 'SHA-256', salt: bytes(16, 1), info: bytes(5, 2) }, o);
  const P = (o) => Object.assign({ name: 'PBKDF2', hash: 'SHA-256', salt: bytes(16, 1), iterations: 3 }, o);
  const imp = (name, us, ext, fmt, data) => S.importKey(fmt || 'raw', data || bytes(20, 9), name, ext === undefined ? false : ext, us || ['deriveBits', 'deriveKey']);
  const hk = await imp('HKDF'), pk = await imp('PBKDF2');
  // import faces
  for (const [n, a] of [['hkdf', 'HKDF'], ['pbkdf2', 'PBKDF2']]) {
    r[n + '.imp.ext'] = await face(async () => { await imp(a, undefined, true); });
    r[n + '.imp.jwk'] = await face(async () => { await imp(a, undefined, false, 'jwk', { kty: 'oct', k: 'AAAA' }); });
    r[n + '.imp.spki'] = await face(async () => { await imp(a, undefined, false, 'spki', bytes(20, 1)); });
    r[n + '.imp.pkcs8'] = await face(async () => { await imp(a, undefined, false, 'pkcs8', bytes(20, 1)); });
    r[n + '.imp.signUsage'] = await face(async () => { await imp(a, ['sign']); });
    r[n + '.imp.noUsage'] = await face(async () => { const k = await imp(a, []); return JSON.stringify(k.usages) + k.extractable + JSON.stringify(k.algorithm) + k.type; });
    r[n + '.imp.empty'] = await face(async () => { const k = await imp(a, undefined, false, 'raw', new Uint8Array(0)); return k.type; });
    r[n + '.imp.shape'] = await face(async () => { const k = await imp(a, ['deriveKey', 'deriveBits', 'deriveKey']); return JSON.stringify(k.usages) + JSON.stringify(k.algorithm); });
    r[n + '.export'] = await face(async () => hex(await S.exportKey('raw', a === 'HKDF' ? hk : pk)));
    r[n + '.genKey'] = await face(async () => { await S.generateKey({ name: a }, false, ['deriveBits']); });
  }
  // HKDF deriveBits
  for (const h of ['SHA-1', 'SHA-256', 'SHA-384', 'SHA-512']) r['hkdf.h.' + h] = await face(async () => hex(await S.deriveBits(H({ hash: h }), hk, 256)));
  for (const L of [0, 8, 7, 9, 255 * 256, 255 * 256 + 8, null, undefined]) r['hkdf.len.' + L] = await face(async () => hex(await S.deriveBits(H({}), hk, L)).slice(0, 64));
  r['hkdf.noLen'] = await face(async () => hex(await S.deriveBits(H({}), hk)));
  r['hkdf.noSalt'] = await face(async () => { const a = H({}); delete a.salt; return hex(await S.deriveBits(a, hk, 128)); });
  r['hkdf.noInfo'] = await face(async () => { const a = H({}); delete a.info; return hex(await S.deriveBits(a, hk, 128)); });
  r['hkdf.noHash'] = await face(async () => { const a = H({}); delete a.hash; return hex(await S.deriveBits(a, hk, 128)); });
  r['hkdf.emptySalt'] = await face(async () => hex(await S.deriveBits(H({ salt: new Uint8Array(0), info: new Uint8Array(0) }), hk, 128)));
  r['hkdf.saltStr'] = await face(async () => hex(await S.deriveBits(H({ salt: 'abc' }), hk, 128)));
  r['hkdf.saltBuf'] = await face(async () => hex(await S.deriveBits(H({ salt: bytes(16, 1).buffer, info: new DataView(bytes(5, 2).buffer) }), hk, 128)));
  r['hkdf.hashObj'] = await face(async () => hex(await S.deriveBits(H({ hash: { name: 'sha-256' } }), hk, 128)));
  r['hkdf.hashBad'] = await face(async () => hex(await S.deriveBits(H({ hash: 'MD5' }), hk, 128)));
  r['hkdf.lenStr'] = await face(async () => hex(await S.deriveBits(H({}), hk, '128')));
  r['hkdf.lenNeg'] = await face(async () => hex(await S.deriveBits(H({}), hk, -8)));
  r['hkdf.lenFrac'] = await face(async () => hex(await S.deriveBits(H({}), hk, 16.7)));
  r['hkdf.wrongKey'] = await face(async () => hex(await S.deriveBits(H({}), pk, 128)));
  r['hkdf.usageKeyOnly'] = await face(async () => hex(await S.deriveBits(H({}), await imp('HKDF', ['deriveKey']), 128)));
  r['hkdf.usageBitsOnlyDK'] = await face(async () => { const k = await S.deriveKey(H({}), await imp('HKDF', ['deriveBits']), { name: 'AES-GCM', length: 128 }, true, ['encrypt']); return hex(await S.exportKey('raw', k)); });
  r['hkdf.emptyIkm'] = await face(async () => hex(await S.deriveBits(H({}), await imp('HKDF', undefined, false, 'raw', new Uint8Array(0)), 128)));
  // HKDF deriveKey
  for (const d of [{ name: 'AES-GCM', length: 128 }, { name: 'AES-CBC', length: 256 }, { name: 'AES-KW', length: 128 }, { name: 'AES-CTR', length: 192 }, { name: 'HMAC', hash: 'SHA-384' }, { name: 'HMAC', hash: 'SHA-1', length: 77 }, { name: 'HKDF' }, { name: 'PBKDF2' }, { name: 'AES-GCM' }, { name: 'ECDSA', namedCurve: 'P-256' }])
    r['hkdf.dk.' + JSON.stringify(d)] = await face(async () => { const k = await S.deriveKey(H({}), hk, d, true, d.name === 'HMAC' ? ['sign'] : d.name.startsWith('AES-KW') ? ['wrapKey'] : d.name === 'HKDF' || d.name === 'PBKDF2' ? ['deriveBits'] : ['encrypt']); return JSON.stringify(k.algorithm) + ':' + hex(await S.exportKey('raw', k)); });
  r['hkdf.dk.badUsage'] = await face(async () => { await S.deriveKey(H({}), hk, { name: 'AES-GCM', length: 128 }, true, ['sign']); });
  r['hkdf.dk.noUsage'] = await face(async () => { await S.deriveKey(H({}), hk, { name: 'AES-GCM', length: 128 }, true, []); });
  r['hkdf.dk.noSalt'] = await face(async () => { const a = H({}); delete a.salt; await S.deriveKey(a, hk, { name: 'AES-GCM', length: 128 }, true, ['encrypt']); });
  // PBKDF2 deriveBits
  for (const h of ['SHA-1', 'SHA-384', 'SHA-512']) r['pb.h.' + h] = await face(async () => hex(await S.deriveBits(P({ hash: h }), pk, 256)));
  for (const L of [0, 8, 7, 9, null, undefined]) r['pb.len.' + L] = await face(async () => hex(await S.deriveBits(P({}), pk, L)).slice(0, 64));
  r['pb.noLen'] = await face(async () => hex(await S.deriveBits(P({}), pk)));
  r['pb.noSalt'] = await face(async () => { const a = P({}); delete a.salt; return hex(await S.deriveBits(a, pk, 128)); });
  r['pb.noHash'] = await face(async () => { const a = P({}); delete a.hash; return hex(await S.deriveBits(a, pk, 128)); });
  r['pb.noIter'] = await face(async () => { const a = P({}); delete a.iterations; return hex(await S.deriveBits(a, pk, 128)); });
  r['pb.iter0'] = await face(async () => hex(await S.deriveBits(P({ iterations: 0 }), pk, 128)));
  r['pb.emptySalt'] = await face(async () => hex(await S.deriveBits(P({ salt: new Uint8Array(0) }), pk, 128)));
  r['pb.wrongKey'] = await face(async () => hex(await S.deriveBits(P({}), hk, 128)));
  r['pb.usageKeyOnly'] = await face(async () => hex(await S.deriveBits(P({}), await imp('PBKDF2', ['deriveKey']), 128)));
  r['pb.emptyPw'] = await face(async () => hex(await S.deriveBits(P({}), await imp('PBKDF2', undefined, false, 'raw', new Uint8Array(0)), 128)));
  r['pb.hashBad'] = await face(async () => hex(await S.deriveBits(P({ hash: 'MD5' }), pk, 128)));
  r['pb.dk.hmac'] = await face(async () => { const k = await S.deriveKey(P({}), pk, { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']); return JSON.stringify(k.algorithm) + ':' + hex(await S.exportKey('raw', k)); });
}
{
  const S = crypto.subtle;
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const hex = (b) => Array.from(new Uint8Array(b), v => v.toString(16).padStart(2, '0')).join('');
  const H = (o) => Object.assign({ name: 'HKDF', hash: 'SHA-256', salt: bytes(16, 1), info: bytes(5, 2) }, o);
  const imp = (name, us) => S.importKey('raw', bytes(20, 9), name, false, us);
  r['ord.aes'] = await face(async () => JSON.stringify((await S.importKey('raw', bytes(16, 1), 'AES-GCM', true, ['unwrapKey', 'decrypt', 'wrapKey', 'encrypt', 'decrypt'])).usages));
  r['ord.hmac'] = await face(async () => JSON.stringify((await S.importKey('raw', bytes(16, 1), { name: 'HMAC', hash: 'SHA-256' }, true, ['verify', 'sign', 'verify'])).usages));
  r['ord.gen'] = await face(async () => JSON.stringify((await S.generateKey({ name: 'AES-CBC', length: 128 }, true, ['decrypt', 'encrypt'])).usages));
  r['ord.ec'] = await face(async () => { const k = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['verify', 'sign']); return JSON.stringify(k.privateKey.usages) + JSON.stringify(k.publicKey.usages); });
  r['ord.ecdh'] = await face(async () => { const k = await S.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits', 'deriveKey']); return JSON.stringify(k.privateKey.usages) + JSON.stringify(k.publicKey.usages); });
  r['ord.hkdf'] = await face(async () => JSON.stringify((await imp('HKDF', ['deriveBits', 'deriveKey'])).usages));
  r['ord.dk'] = await face(async () => JSON.stringify((await S.deriveKey(H({}), await imp('HKDF', ['deriveKey']), { name: 'AES-GCM', length: 128 }, true, ['decrypt', 'encrypt', 'decrypt'])).usages));
  const pk = await imp('PBKDF2', ['deriveKey']);
  r['o.wrongKeyNoSalt'] = await face(async () => { const a = H({}); delete a.salt; await S.deriveBits(a, pk, 128); });
  r['o.wrongKeyNullLen'] = await face(async () => { await S.deriveBits(H({}), pk, null); });
  r['o.usageNullLen'] = await face(async () => { await S.deriveBits(H({}), await imp('HKDF', ['deriveKey']), null); });
  r['o.usageBadHash'] = await face(async () => { await S.deriveBits(H({ hash: 'MD5' }), await imp('HKDF', ['deriveKey']), 128); });
  r['o.noSaltNoInfo'] = await face(async () => { const a = H({}); delete a.salt; delete a.info; await S.deriveBits(a, pk, 128); });
  r['o.noHashNoSalt'] = await face(async () => { const a = H({}); delete a.salt; delete a.hash; await S.deriveBits(a, pk, 128); });
  r['o.hashNum'] = await face(async () => { await S.deriveBits(H({ hash: 5 }), pk, 128); });
  r['o.hashNoName'] = await face(async () => { await S.deriveBits(H({ hash: {} }), pk, 128); });
  r['o.saltNull'] = await face(async () => { await S.deriveBits(H({ salt: null }), pk, 128); });
  r['o.dkWrongKey'] = await face(async () => { await S.deriveKey(H({}), pk, { name: 'AES-GCM', length: 128 }, true, ['encrypt']); });
  r['o.dkUsageVsDerived'] = await face(async () => { await S.deriveKey(H({}), await imp('HKDF', ['deriveBits']), { name: 'AES-GCM' }, true, ['encrypt']); });
  r['o.dkUsageVsBadUsage'] = await face(async () => { await S.deriveKey(H({}), await imp('HKDF', ['deriveBits']), { name: 'AES-GCM', length: 128 }, true, ['sign']); });
  r['o.dkHmacNullLen'] = await face(async () => { const k = await S.deriveKey(H({}), await imp('HKDF', ['deriveKey']), { name: 'HMAC', hash: 'SHA-512' }, true, ['sign']); return JSON.stringify(k.algorithm); });
  r['o.dkHmac0'] = await face(async () => { await S.deriveKey(H({}), await imp('HKDF', ['deriveKey']), { name: 'HMAC', hash: 'SHA-256', length: 0 }, true, ['sign']); });
  r['o.ecdhWrongAlg'] = await face(async () => { await S.deriveBits({ name: 'ECDH', public: pk }, pk, 128); });
  r['o.importExtAndBadUsage'] = await face(async () => { await S.importKey('raw', bytes(4, 1), 'HKDF', true, ['sign']); });
  r['o.importJwkAndExt'] = await face(async () => { await S.importKey('jwk', { kty: 'oct', k: 'AA' }, 'HKDF', true, ['deriveBits']); });
  r['o.importJwkBadUsage'] = await face(async () => { await S.importKey('jwk', { kty: 'oct', k: 'AA' }, 'HKDF', false, ['sign']); });
  r['o.importExtNoUsage'] = await face(async () => { await S.importKey('raw', bytes(4, 1), 'PBKDF2', true, []); });
  r['o.pbLenNeg'] = await face(async () => { await S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: bytes(4, 1), iterations: 1 }, pk, -8); });
  r['o.pbLenFrac'] = await face(async () => hex(await S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: bytes(4, 1), iterations: 1 }, await imp('PBKDF2', ['deriveBits']), 16.7)));
  r['o.pbIter0WrongKey'] = await face(async () => { await S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: bytes(4, 1), iterations: 0 }, await imp('HKDF', ['deriveBits']), 8); });
  r['o.pbLen7Iter0'] = await face(async () => { await S.deriveBits({ name: 'PBKDF2', hash: 'SHA-256', salt: bytes(4, 1), iterations: 0 }, await imp('PBKDF2', ['deriveBits']), 7); });
  r['o.hkdfBig'] = await face(async () => { await S.deriveBits(H({ hash: 'SHA-1' }), await imp('HKDF', ['deriveBits']), 255 * 160 + 8); });
  r['o.hkdfBigOk'] = await face(async () => (await S.deriveBits(H({ hash: 'SHA-512' }), await imp('HKDF', ['deriveBits']), 255 * 512)).byteLength);
  for (const [n, h] of [['null', null], ['name5', { name: 5 }], ['under', 'sha_256'], ['lower', 'sha-384'], ['arr', ['SHA-256']]]) {
    r['hash.hkdf.' + n] = await face(async () => hex(await S.deriveBits(H({ hash: h }), await imp('HKDF', ['deriveBits']), 64)));
    r['hash.hmac.' + n] = await face(async () => JSON.stringify((await S.importKey('raw', bytes(16, 1), { name: 'HMAC', hash: h }, true, ['sign'])).algorithm));
  }
  r['o.wrapHkdf'] = await face(async () => { const kw = await S.importKey('raw', bytes(16, 1), 'AES-KW', true, ['wrapKey']); await S.wrapKey('raw', await imp('HKDF', ['deriveBits']), kw, 'AES-KW'); });
}
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "hkdf.imp.ext": "SyntaxError: KDF keys must set extractable=false",
 "hkdf.imp.jwk": "NotSupportedError: Unsupported import key format for algorithm",
 "hkdf.imp.spki": "NotSupportedError: Unsupported import key format for algorithm",
 "hkdf.imp.pkcs8": "NotSupportedError: Unsupported import key format for algorithm",
 "hkdf.imp.signUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "hkdf.imp.noUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "hkdf.imp.empty": "secret",
 "hkdf.imp.shape": "[\"deriveKey\",\"deriveBits\"]{\"name\":\"HKDF\"}",
 "hkdf.export": "NotSupportedError: Failed to execute 'exportKey' on 'SubtleCrypto': HKDF: Unsupported operation: exportKey",
 "hkdf.genKey": "NotSupportedError: Failed to execute 'generateKey' on 'SubtleCrypto': HKDF: Unsupported operation: generateKey",
 "pbkdf2.imp.ext": "SyntaxError: KDF keys must set extractable=false",
 "pbkdf2.imp.jwk": "NotSupportedError: Unsupported import key format for algorithm",
 "pbkdf2.imp.spki": "NotSupportedError: Unsupported import key format for algorithm",
 "pbkdf2.imp.pkcs8": "NotSupportedError: Unsupported import key format for algorithm",
 "pbkdf2.imp.signUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "pbkdf2.imp.noUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "pbkdf2.imp.empty": "secret",
 "pbkdf2.imp.shape": "[\"deriveKey\",\"deriveBits\"]{\"name\":\"PBKDF2\"}",
 "pbkdf2.export": "NotSupportedError: Failed to execute 'exportKey' on 'SubtleCrypto': PBKDF2: Unsupported operation: exportKey",
 "pbkdf2.genKey": "NotSupportedError: Failed to execute 'generateKey' on 'SubtleCrypto': PBKDF2: Unsupported operation: generateKey",
 "hkdf.h.SHA-1": "6895121c65e085c4e7e56db03fdff4d7d034bede0603f50bc94fe8b6b6629fdc",
 "hkdf.h.SHA-256": "e3e9d542ee1f13555ccffbc1a55f2a830a069c9351b76dd7a5721f4a462524d1",
 "hkdf.h.SHA-384": "0d114bebd72af68634db1e6172b3d33d99141ecfd8801fc920a569265e6137a3",
 "hkdf.h.SHA-512": "e0ea6e4e146ca9c50192deef2e524e389cd4f9e12d5c73e7e86964836381ab87",
 "hkdf.len.0": "",
 "hkdf.len.8": "e3",
 "hkdf.len.7": "OperationError: The length provided for HKDF is not a multiple of 8 bits.",
 "hkdf.len.9": "OperationError: The length provided for HKDF is not a multiple of 8 bits.",
 "hkdf.len.65280": "e3e9d542ee1f13555ccffbc1a55f2a830a069c9351b76dd7a5721f4a462524d1",
 "hkdf.len.65288": "OperationError: The length provided for HKDF is too large.",
 "hkdf.len.null": "OperationError: No length was specified for the HKDF Derive Bits operation.",
 "hkdf.len.undefined": "OperationError: No length was specified for the HKDF Derive Bits operation.",
 "hkdf.noLen": "OperationError: No length was specified for the HKDF Derive Bits operation.",
 "hkdf.noSalt": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: salt: Missing required property",
 "hkdf.noInfo": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: info: Missing required property",
 "hkdf.noHash": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Missing or not an AlgorithmIdentifier",
 "hkdf.emptySalt": "aef12e5860534b9931143e76260d0ee1",
 "hkdf.saltStr": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: salt: Not a BufferSource",
 "hkdf.saltBuf": "e3e9d542ee1f13555ccffbc1a55f2a83",
 "hkdf.hashObj": "e3e9d542ee1f13555ccffbc1a55f2a83",
 "hkdf.hashBad": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "hkdf.lenStr": "e3e9d542ee1f13555ccffbc1a55f2a83",
 "hkdf.lenNeg": "OperationError: The length provided for HKDF is too large.",
 "hkdf.lenFrac": "e3e9",
 "hkdf.wrongKey": "InvalidAccessError: key.algorithm does not match that of operation",
 "hkdf.usageKeyOnly": "InvalidAccessError: key.usages does not permit this operation",
 "hkdf.usageBitsOnlyDK": "InvalidAccessError: key.usages does not permit this operation",
 "hkdf.emptyIkm": "488b47fd63225bf11b2c77db975d66cb",
 "hkdf.dk.{\"name\":\"AES-GCM\",\"length\":128}": "{\"name\":\"AES-GCM\",\"length\":128}:e3e9d542ee1f13555ccffbc1a55f2a83",
 "hkdf.dk.{\"name\":\"AES-CBC\",\"length\":256}": "{\"name\":\"AES-CBC\",\"length\":256}:e3e9d542ee1f13555ccffbc1a55f2a830a069c9351b76dd7a5721f4a462524d1",
 "hkdf.dk.{\"name\":\"AES-KW\",\"length\":128}": "{\"name\":\"AES-KW\",\"length\":128}:e3e9d542ee1f13555ccffbc1a55f2a83",
 "hkdf.dk.{\"name\":\"AES-CTR\",\"length\":192}": "OperationError: 192-bit AES keys are not supported",
 "hkdf.dk.{\"name\":\"HMAC\",\"hash\":\"SHA-384\"}": "{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-384\"},\"length\":1024}:e3e9d542ee1f13555ccffbc1a55f2a830a069c9351b76dd7a5721f4a462524d104c260b222149cc75693b922f2f1793d8ef6f2075cf32a323a68bc318873d1539c932a4bcf262c31ccc6892141aecb4cb3500434bab361be8adb9032fed6932ae15b08aba326c98f3d1a3e28b0da21b270b37696ea3b43d08b5d8537315130ef",
 "hkdf.dk.{\"name\":\"HMAC\",\"hash\":\"SHA-1\",\"length\":77}": "OperationError: The length provided for HKDF is not a multiple of 8 bits.",
 "hkdf.dk.{\"name\":\"HKDF\"}": "OperationError: No length was specified for the HKDF Derive Bits operation.",
 "hkdf.dk.{\"name\":\"PBKDF2\"}": "OperationError: No length was specified for the HKDF Derive Bits operation.",
 "hkdf.dk.{\"name\":\"AES-GCM\"}": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Missing required property",
 "hkdf.dk.{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}": "NotSupportedError: Failed to execute 'deriveKey' on 'SubtleCrypto': ECDSA: Unsupported operation: get key length",
 "hkdf.dk.badUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "hkdf.dk.noUsage": "SyntaxError: Usages cannot be empty when creating a key.",
 "hkdf.dk.noSalt": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': HkdfParams: salt: Missing required property",
 "pb.h.SHA-1": "adf391e73bd88bb1eeff5974ca10035bc171a8e04572a7e7d7652c3b6d6771bc",
 "pb.h.SHA-384": "703384bbf13b00c6c8122e12b83bb003c4dee51da6a3e5cbc1b385b5c0ea0eac",
 "pb.h.SHA-512": "d06d297a3e9495f87bc83f3562ce7545476782fefeeacdc94f959f349257bafc",
 "pb.len.0": "",
 "pb.len.8": "00",
 "pb.len.7": "OperationError: Length for PBKDF2 key derivation must be a multiple of 8 bits.",
 "pb.len.9": "OperationError: Length for PBKDF2 key derivation must be a multiple of 8 bits.",
 "pb.len.null": "OperationError: No length was specified for the PBKDF2 Derive Bits operation.",
 "pb.len.undefined": "OperationError: No length was specified for the PBKDF2 Derive Bits operation.",
 "pb.noLen": "OperationError: No length was specified for the PBKDF2 Derive Bits operation.",
 "pb.noSalt": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': Pbkdf2Params: salt: Missing required property",
 "pb.noHash": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': Pbkdf2Params: hash: Missing or not an AlgorithmIdentifier",
 "pb.noIter": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': Pbkdf2Params: iterations: Missing required property",
 "pb.iter0": "OperationError: PBKDF2 requires iterations > 0",
 "pb.emptySalt": "e949e792ca0fef0d94f9988b1c9e1236",
 "pb.wrongKey": "InvalidAccessError: key.algorithm does not match that of operation",
 "pb.usageKeyOnly": "InvalidAccessError: key.usages does not permit this operation",
 "pb.emptyPw": "0789c444e286088ee981f5972b0e7caa",
 "pb.hashBad": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': Pbkdf2Params: hash: Algorithm: Unrecognized name",
 "pb.dk.hmac": "{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":512}:00849fb997b8a512c02a73f185419fb4537d73712806814a50990192d91f5ef916dfb0ad53dff98095644344d7c854ed0df060ff04b7216b4627c8eb4106b6a5",
 "ord.aes": "[\"encrypt\",\"decrypt\",\"wrapKey\",\"unwrapKey\"]",
 "ord.hmac": "[\"sign\",\"verify\"]",
 "ord.gen": "[\"encrypt\",\"decrypt\"]",
 "ord.ec": "[\"sign\"][\"verify\"]",
 "ord.ecdh": "[\"deriveKey\",\"deriveBits\"][]",
 "ord.hkdf": "[\"deriveKey\",\"deriveBits\"]",
 "ord.dk": "[\"encrypt\",\"decrypt\"]",
 "o.wrongKeyNoSalt": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: salt: Missing required property",
 "o.wrongKeyNullLen": "InvalidAccessError: key.algorithm does not match that of operation",
 "o.usageNullLen": "InvalidAccessError: key.usages does not permit this operation",
 "o.usageBadHash": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "o.noSaltNoInfo": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: salt: Missing required property",
 "o.noHashNoSalt": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Missing or not an AlgorithmIdentifier",
 "o.hashNum": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "o.hashNoName": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: name: Missing or not a string",
 "o.saltNull": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: salt: Not a BufferSource",
 "o.dkWrongKey": "InvalidAccessError: key.algorithm does not match that of operation",
 "o.dkUsageVsDerived": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': AesDerivedKeyParams: length: Missing required property",
 "o.dkUsageVsBadUsage": "InvalidAccessError: key.usages does not permit this operation",
 "o.dkHmacNullLen": "{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-512\"},\"length\":1024}",
 "o.dkHmac0": "TypeError: HMAC key length must not be zero",
 "o.ecdhWrongAlg": "InvalidAccessError: key.algorithm does not match that of operation",
 "o.importExtAndBadUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "o.importJwkAndExt": "NotSupportedError: Unsupported import key format for algorithm",
 "o.importJwkBadUsage": "NotSupportedError: Unsupported import key format for algorithm",
 "o.importExtNoUsage": "SyntaxError: KDF keys must set extractable=false",
 "o.pbLenNeg": "InvalidAccessError: key.usages does not permit this operation",
 "o.pbLenFrac": "1fe0",
 "o.pbIter0WrongKey": "InvalidAccessError: key.algorithm does not match that of operation",
 "o.pbLen7Iter0": "OperationError: Length for PBKDF2 key derivation must be a multiple of 8 bits.",
 "o.hkdfBig": "OperationError: The length provided for HKDF is too large.",
 "o.hkdfBigOk": "16320",
 "hash.hkdf.null": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "hash.hmac.null": "NotSupportedError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Algorithm: Unrecognized name",
 "hash.hkdf.name5": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "hash.hmac.name5": "NotSupportedError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Algorithm: Unrecognized name",
 "hash.hkdf.under": "NotSupportedError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: Unrecognized name",
 "hash.hmac.under": "NotSupportedError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Algorithm: Unrecognized name",
 "hash.hkdf.lower": "0d114bebd72af686",
 "hash.hmac.lower": "{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-384\"},\"length\":128}",
 "hash.hkdf.arr": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': HkdfParams: hash: Algorithm: name: Missing or not a string",
 "hash.hmac.arr": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': HmacImportParams: hash: Algorithm: name: Missing or not a string",
 "o.wrapHkdf": "NotSupportedError: HKDF: Unsupported operation: exportKey"
}"#,
    )
    .unwrap();
    assert_eq!(result.value.unwrap(), expected);
}

#[tokio::test(flavor = "current_thread")]
async fn webcrypto_wrap_and_operation_checks_like_chrome() {
    // wrapKey/unwrapKey against Chrome 157 (headless, file://): AES-KW/GCM/
    // CBC/CTR wrapping HMAC, AES and EC keys as raw/jwk/pkcs8, ciphertexts
    // byte for byte (so the jwk text is Chrome's too); then the checks every
    // operation runs and their order — argument types, the format enum, the
    // algorithm normalized per operation (unknown name vs unsupported
    // operation), the key's algorithm and its exact usage, the key type —
    // plus unwrap's decrypt-before-import and JWK-parse faces. Expected =
    // Chrome's output for this exact script. We used to let an encrypt key
    // wrap and a wrap key decrypt, skip the algorithm match, and speak our
    // own texts.
    let mut rt = setup_runtime("<html><body></body></html>");
    let script = r#"async () => { const r = {};
{
  const S = crypto.subtle;
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const hex = (b) => Array.from(new Uint8Array(b), v => v.toString(16).padStart(2, '0')).join('');
  const dec = (b) => new TextDecoder().decode(b);
  const W = {
    kw: await S.importKey('raw', bytes(16, 1), 'AES-KW', false, ['wrapKey', 'unwrapKey']),
    gcm: await S.importKey('raw', bytes(16, 2), 'AES-GCM', false, ['wrapKey', 'unwrapKey', 'encrypt', 'decrypt']),
    cbc: await S.importKey('raw', bytes(32, 3), 'AES-CBC', false, ['wrapKey', 'unwrapKey', 'decrypt']),
    ctr: await S.importKey('raw', bytes(16, 4), 'AES-CTR', false, ['wrapKey', 'unwrapKey']),
  };
  const P = {
    kw: 'AES-KW',
    gcm: { name: 'AES-GCM', iv: bytes(12, 5) },
    cbc: { name: 'AES-CBC', iv: bytes(16, 6) },
    ctr: { name: 'AES-CTR', counter: bytes(16, 7), length: 64 },
  };
  const T = {
    hmac: [await S.importKey('raw', bytes(32, 8), { name: 'HMAC', hash: 'SHA-256' }, true, ['sign', 'verify']), { name: 'HMAC', hash: 'SHA-256' }, ['verify', 'sign']],
    hmac20: [await S.importKey('raw', bytes(20, 8), { name: 'HMAC', hash: 'SHA-1' }, true, ['sign']), { name: 'HMAC', hash: 'SHA-1' }, ['sign']],
    aes: [await S.importKey('raw', bytes(16, 9), 'AES-GCM', true, ['encrypt', 'decrypt']), 'AES-GCM', ['decrypt', 'encrypt']],
    aes256: [await S.importKey('raw', bytes(32, 9), 'AES-CBC', true, ['encrypt']), 'AES-CBC', ['encrypt']],
  };
  const ec = await S.importKey('jwk', { kty: 'EC', crv: 'P-256', x: '3ekkyaO3PdweD_LJdGppCjgug4xpSwnfzlxsZ8mUckc', y: 'J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0', d: 'HTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4' }, { name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']);
  for (const w of Object.keys(W)) for (const t of Object.keys(T)) for (const fmt of ['raw', 'jwk']) {
    r[`${w}.${t}.${fmt}`] = await face(async () => {
      const [k, alg, us] = T[t];
      const c = await S.wrapKey(fmt, k, W[w], P[w]);
      const u = await S.unwrapKey(fmt, c, W[w], P[w], alg, true, us);
      return hex(c) + '|' + JSON.stringify(u.algorithm) + JSON.stringify(u.usages) + u.extractable + '|' + hex(await S.exportKey('raw', u));
    });
  }
  for (const w of Object.keys(W)) for (const fmt of ['pkcs8', 'jwk']) r[`${w}.ec.${fmt}`] = await face(async () => {
    const c = await S.wrapKey(fmt, ec, W[w], P[w]);
    const u = await S.unwrapKey(fmt, c, W[w], P[w], { name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign']);
    return (w === 'kw' || fmt === 'pkcs8' ? hex(c) : c.byteLength) + '|' + JSON.stringify(u.algorithm) + u.type + u.extractable + JSON.stringify(u.usages);
  });
  r['jwkText'] = await face(async () => dec(await S.decrypt(P.gcm, W.gcm, await S.wrapKey('jwk', T.hmac[0], W.gcm, P.gcm))));
  r['jwkTextEc'] = await face(async () => dec(await S.decrypt(P.gcm, W.gcm, await S.wrapKey('jwk', ec, W.gcm, P.gcm))));
  r['jwkTextAes'] = await face(async () => dec(await S.decrypt(P.gcm, W.gcm, await S.wrapKey('jwk', T.aes[0], W.gcm, P.gcm))));
  // refusals
  const ne = await S.importKey('raw', bytes(16, 9), 'AES-GCM', false, ['encrypt']);
  r['e.notExtractable'] = await face(async () => { await S.wrapKey('raw', ne, W.kw, 'AES-KW'); });
  r['e.noWrapUsage'] = await face(async () => { await S.wrapKey('raw', T.aes[0], await S.importKey('raw', bytes(16, 1), 'AES-GCM', false, ['encrypt']), P.gcm); });
  r['e.noUnwrapUsage'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), await S.importKey('raw', bytes(16, 1), 'AES-KW', false, ['wrapKey']), 'AES-KW', 'AES-GCM', true, ['encrypt']); });
  r['e.algMismatch'] = await face(async () => { await S.wrapKey('raw', T.aes[0], W.gcm, P.cbc); });
  r['e.encryptAlgAsWrap'] = await face(async () => { await S.wrapKey('raw', T.aes[0], W.kw, P.gcm); });
  r['e.hmacWrapAlg'] = await face(async () => { await S.wrapKey('raw', T.aes[0], T.hmac[0], { name: 'HMAC' }); });
  r['e.ecdsaWrapAlg'] = await face(async () => { await S.wrapKey('raw', T.aes[0], ec, { name: 'ECDSA', hash: 'SHA-256' }); });
  r['e.kwJwkNot8'] = await face(async () => { const k = await S.importKey('raw', bytes(16, 9), 'AES-CTR', true, ['encrypt']); return hex(await S.wrapKey('jwk', k, W.kw, 'AES-KW')); });
  r['e.kwRawNot8'] = await face(async () => { await S.wrapKey('raw', T.hmac20[0], W.kw, 'AES-KW'); });
  r['e.kwRaw20pad'] = await face(async () => hex(await S.wrapKey('raw', await S.importKey('raw', bytes(20, 8), { name: 'HMAC', hash: 'SHA-1' }, true, ['sign']), W.kw, 'AES-KW')));
  r['e.badFormat'] = await face(async () => { await S.wrapKey('spki', T.aes[0], W.kw, 'AES-KW'); });
  r['e.rawEc'] = await face(async () => { await S.wrapKey('raw', ec, W.gcm, P.gcm); });
  r['e.unwrapBadJson'] = await face(async () => { const c = await S.encrypt(P.gcm, W.gcm, new TextEncoder().encode('{not json')); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapJsonArray'] = await face(async () => { const c = await S.encrypt(P.gcm, W.gcm, new TextEncoder().encode('[1,2]')); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapJsonNum'] = await face(async () => { const c = await S.encrypt(P.gcm, W.gcm, new TextEncoder().encode('5')); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapJsonBom'] = await face(async () => { const j = '﻿' + JSON.stringify({ kty: 'oct', k: 'AAAAAAAAAAAAAAAAAAAAAA' }); const c = await S.encrypt(P.gcm, W.gcm, new TextEncoder().encode(j)); const u = await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); return JSON.stringify(u.algorithm); });
  r['e.unwrapJsonBadUtf8'] = await face(async () => { const c = await S.encrypt(P.gcm, W.gcm, Uint8Array.of(0x7b, 0xff, 0x7d)); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapTamper'] = await face(async () => { const c = new Uint8Array(await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm)); c[0] ^= 1; await S.unwrapKey('raw', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapBadUsage'] = await face(async () => { const c = await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm); await S.unwrapKey('raw', c, W.gcm, P.gcm, 'AES-GCM', true, ['sign']); });
  r['e.unwrapBadUsageTamper'] = await face(async () => { const c = new Uint8Array(await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm)); c[0] ^= 1; await S.unwrapKey('raw', c, W.gcm, P.gcm, 'AES-GCM', true, ['sign']); });
  r['e.unwrapKdfExt'] = await face(async () => { const c = await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm); await S.unwrapKey('raw', c, W.gcm, P.gcm, 'HKDF', true, ['deriveBits']); });
  r['e.unwrapKdf'] = await face(async () => { const c = await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm); const u = await S.unwrapKey('raw', c, W.gcm, P.gcm, 'HKDF', false, ['deriveBits']); return JSON.stringify(u.algorithm) + u.extractable; });
  r['e.unwrapAesWrongLen'] = await face(async () => { const c = await S.wrapKey('raw', T.hmac20[0], W.gcm, P.gcm); await S.unwrapKey('raw', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapJwkAlgMismatch'] = await face(async () => { const c = await S.wrapKey('jwk', T.aes[0], W.gcm, P.gcm); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-CBC', true, ['encrypt']); });
  r['e.unwrapJwkExtFalse'] = await face(async () => { const c = await S.encrypt(P.gcm, W.gcm, new TextEncoder().encode(JSON.stringify({ kty: 'oct', k: 'AAAAAAAAAAAAAAAAAAAAAA', ext: false }))); await S.unwrapKey('jwk', c, W.gcm, P.gcm, 'AES-GCM', true, ['encrypt']); });
  r['e.unwrapNonExtract'] = await face(async () => { const c = await S.wrapKey('raw', T.aes[0], W.gcm, P.gcm); const u = await S.unwrapKey('raw', c, W.gcm, P.gcm, 'AES-GCM', false, ['encrypt']); return u.extractable + JSON.stringify(u.usages); });
  r['e.unwrapUnknownUsage'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), W.kw, 'AES-KW', 'AES-GCM', true, ['bogus']); });
  r['e.wrapKeyNotKey'] = await face(async () => { await S.wrapKey('raw', {}, W.kw, 'AES-KW'); });
  r['e.unwrapKeyIsWrappingSelf'] = await face(async () => { const k = await S.importKey('raw', bytes(16, 1), 'AES-KW', true, ['wrapKey', 'unwrapKey']); return hex(await S.wrapKey('raw', k, k, 'AES-KW')); });
  r['e.gcmTagLenWrap'] = await face(async () => { const p = { name: 'AES-GCM', iv: bytes(12, 5), tagLength: 96 }; const c = await S.wrapKey('raw', T.aes[0], W.gcm, p); const u = await S.unwrapKey('raw', c, W.gcm, p, 'AES-GCM', true, ['encrypt']); return hex(c) + hex(await S.exportKey('raw', u)); });
  r['e.decryptWithWrapOnly'] = await face(async () => { await S.decrypt(P.ctr, W.ctr, bytes(16, 1)); });
}
{
  const S = crypto.subtle;
  const face = async (f) => { try { const v = await f(); return v === undefined ? 'ok' : String(v); } catch (e) { return e.name + ': ' + e.message; } };
  const bytes = (n, seed) => Uint8Array.from({ length: n }, (_, i) => (i * 37 + seed) & 255);
  const aes = await S.importKey('raw', bytes(16, 9), 'AES-GCM', true, ['encrypt', 'decrypt']);
  const hm = await S.importKey('raw', bytes(16, 9), { name: 'HMAC', hash: 'SHA-256' }, true, ['sign']);
  const kw = await S.importKey('raw', bytes(16, 1), 'AES-KW', false, ['wrapKey', 'unwrapKey']);
  const iv = { name: 'AES-GCM', iv: bytes(12, 1) };
  r['x.exportSpkiAes'] = await face(async () => { await S.exportKey('spki', aes); });
  r['x.exportPkcs8Hmac'] = await face(async () => { await S.exportKey('pkcs8', hm); });
  r['x.exportBogus'] = await face(async () => { await S.exportKey('bogus', hm); });
  r['x.encryptHmac'] = await face(async () => { await S.encrypt({ name: 'HMAC' }, hm, bytes(4, 1)); });
  r['x.decryptEcdsa'] = await face(async () => { await S.decrypt({ name: 'ECDSA' }, hm, bytes(4, 1)); });
  r['x.encryptMismatch'] = await face(async () => { await S.encrypt({ name: 'AES-CBC', iv: bytes(16, 1) }, aes, bytes(4, 1)); });
  r['x.encryptNotKey'] = await face(async () => { await S.encrypt(iv, {}, bytes(4, 1)); });
  r['x.signNotKey'] = await face(async () => { await S.sign('HMAC', {}, bytes(4, 1)); });
  r['x.exportNotKey'] = await face(async () => { await S.exportKey('raw', {}); });
  r['x.unwrapNotKey'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), {}, 'AES-KW', 'AES-GCM', true, ['encrypt']); });
  r['x.wrapWrappingNotKey'] = await face(async () => { await S.wrapKey('raw', aes, {}, 'AES-KW'); });
  r['x.wrapBadFormat'] = await face(async () => { await S.wrapKey('bogus', aes, kw, 'AES-KW'); });
  r['x.wrapNotExtractableAndNoUsage'] = await face(async () => { const ne = await S.importKey('raw', bytes(16, 9), 'AES-GCM', false, ['encrypt']); await S.wrapKey('raw', ne, await S.importKey('raw', bytes(16, 1), 'AES-KW', false, ['unwrapKey']), 'AES-KW'); });
  r['x.wrapSpkiAesThenNotExtract'] = await face(async () => { const ne = await S.importKey('raw', bytes(16, 9), 'AES-GCM', false, ['encrypt']); await S.wrapKey('spki', ne, kw, 'AES-KW'); });
  r['x.unwrapImportAlgBad'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), kw, 'AES-KW', 'FOO', true, ['encrypt']); });
  r['x.unwrapImportAlgVsUsage'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), await S.importKey('raw', bytes(16, 1), 'AES-KW', false, ['wrapKey']), 'AES-KW', 'FOO', true, ['encrypt']); });
  r['x.unwrapWithEncryptAlg'] = await face(async () => { await S.unwrapKey('raw', bytes(24, 1), kw, { name: 'HMAC' }, 'AES-GCM', true, ['encrypt']); });
  r['x.unwrapFormatBad'] = await face(async () => { await S.unwrapKey('spki', bytes(24, 1), kw, 'AES-KW', 'AES-GCM', true, ['encrypt']); });
  r['x.encryptUnknownAlg'] = await face(async () => { await S.encrypt({ name: 'FOO' }, aes, bytes(4, 1)); });
  r['x.wrapUnknownAlg'] = await face(async () => { await S.wrapKey('raw', aes, kw, 'FOO'); });
  const ecK = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify']);
  const dhK = await S.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']);
  const edK = await S.generateKey({ name: 'Ed25519' }, true, ['sign', 'verify']);
  r['t.ecSignPublic'] = await face(async () => { await S.sign({ name: 'ECDSA', hash: 'SHA-256' }, ecK.publicKey, bytes(4, 1)); });
  r['t.ecVerifyPrivate'] = await face(async () => { await S.verify({ name: 'ECDSA', hash: 'SHA-256' }, ecK.privateKey, bytes(64, 1), bytes(4, 1)); });
  r['t.ecdhPublicBase'] = await face(async () => { await S.deriveBits({ name: 'ECDH', public: dhK.publicKey }, dhK.publicKey, 256); });
  r['t.ecPkcs8Public'] = await face(async () => { await S.exportKey('pkcs8', ecK.publicKey); });
  r['t.edSignPublic'] = await face(async () => { await S.sign('Ed25519', edK.publicKey, bytes(4, 1)); });
  r['t.edVerifyPrivate'] = await face(async () => { await S.verify('Ed25519', edK.privateKey, bytes(64, 1), bytes(4, 1)); });
  r['t.edRawPrivate'] = await face(async () => { await S.exportKey('raw', edK.privateKey); });
  r['t.edPkcs8Public'] = await face(async () => { await S.exportKey('pkcs8', edK.publicKey); });
  r['t.edSpkiPrivate'] = await face(async () => { await S.exportKey('spki', edK.privateKey); });
  r['x.importBogusFmt'] = await face(async () => { await S.importKey('bogus', bytes(16, 1), 'AES-GCM', true, ['encrypt']); });
  r['x.importBogusFmtBadAlg'] = await face(async () => { await S.importKey('bogus', bytes(16, 1), 'FOO', true, ['encrypt']); });
  r['x.importBadAlg'] = await face(async () => { await S.importKey('raw', bytes(16, 1), 'FOO', true, ['encrypt']); });
  r['x.verifyNotKey'] = await face(async () => { await S.verify('HMAC', {}, bytes(4, 1), bytes(4, 1)); });
  r['x.deriveBitsNotKey'] = await face(async () => { await S.deriveBits({ name: 'HKDF' }, {}, 8); });
  r['x.deriveKeyNotKey'] = await face(async () => { await S.deriveKey({ name: 'HKDF' }, {}, 'AES-GCM', true, ['encrypt']); });
  r['x.encryptNotKeyBadAlg'] = await face(async () => { await S.encrypt({ name: 'FOO' }, {}, bytes(4, 1)); });
  r['x.wrapBadFmtNotKey'] = await face(async () => { await S.wrapKey('bogus', {}, kw, 'AES-KW'); });
  r['x.wrapBadFmtBadAlg'] = await face(async () => { await S.wrapKey('bogus', aes, kw, 'FOO'); });
  r['x.encryptEd25519'] = await face(async () => { await S.encrypt({ name: 'Ed25519' }, aes, bytes(4, 1)); });
  r['x.encryptSha'] = await face(async () => { await S.encrypt({ name: 'SHA-256' }, aes, bytes(4, 1)); });
  r['x.encryptLower'] = await face(async () => { await S.encrypt({ name: 'aes-cbc', iv: bytes(16, 1) }, aes, bytes(4, 1)); });
  r['x.wrapEcRawPublic'] = await face(async () => { const k = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']); return (await S.wrapKey('raw', k.publicKey, kw, 'AES-KW')).byteLength; });
  r['x.wrapEcSpkiPrivate'] = await face(async () => { const k = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']); await S.wrapKey('spki', k.privateKey, kw, 'AES-KW'); });
  r['x.exportEcSpkiPrivate'] = await face(async () => { const k = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']); await S.exportKey('spki', k.privateKey); });
  r['x.exportEcRawPrivate'] = await face(async () => { const k = await S.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign']); await S.exportKey('raw', k.privateKey); });
}
  return r;
}"#;
    let result = rt
        .call_function_on_remote(script, None, &[], true, true)
        .await
        .unwrap();
    let expected: serde_json::Value = serde_json::from_str(
        r#"{
 "kw.hmac.raw": "b204be7b7ced046ddbc3db76431f939d909cc0fe2589dac42ae6ef1e62248a0435b1bae55eb3e7e4|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "kw.hmac.jwk": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "kw.hmac20.raw": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "kw.hmac20.jwk": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "kw.aes.raw": "6f1509d9e8addf581ffe57f5ed58fbcb8eda6ac3c10dc39b|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "kw.aes.jwk": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "kw.aes256.raw": "9ecfa456eb35ca3335635f4725d4f7581d7146730773cbc10cdbf673650d25f10e73ab4de5e43692|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "kw.aes256.jwk": "9933dda2abe3e306ab823c6dfdadf7c7a416eeb6d589c0a2512d341df046ba3b11f83b296a3593e88c7ffa3e4b83a5b4049bca3c85e0aa16367f8eb6ffc11c8bd62e59c437b05e55eec81301f0f7f2f29d97e1e0a8aced3b8888218f0d9d7c73f2515980ee70aeb6181cc122a3a008a8a5fea78d33c627cb|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "gcm.hmac.raw": "f82cb1128bea120dd1bedee3ff6583b1b70920f69faac8aa9b8ca8a65b65ed7f3a85422bb1a376419f7f2b9c6a88a9ba|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "gcm.hmac.jwk": "8b2382097009ce24a9b896490daea1a08a0cf61349cf8c847e0540226d6691bf09f3716307f4ac01b6b6ab9236ffeee6bdd17788c1d198c6339a01adff1a036e689e5631b4337db547362f05c55867e1c6dfb9cdf8ff672e2ef08540f959eecbce702a76fca63e3fddd2e20ed47ba2623f8e2253acae2feae1de042172564fce541e27cc|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "gcm.hmac20.raw": "f82cb1128bea120dd1bedee3ff6583b1b70920f61cac0df11ae7121a51dd601b1c758c9f|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "gcm.hmac20.jwk": "8b2382097009ce24a9b8955e17aee8fa9b56b84501ce9bdd394240736d1ff0cd19a6177d70bb8947a69384b508f6c7f5bdf709adc9d6d2ca62d05083ce393e6653b53f3c97403b844f152f74cb1169ece092f39cb5a6482e20920d949b5157fc65f51d0b54cf5b0c83|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "gcm.aes.raw": "f92fb01d8ae9130ad0bddfdcfe6682b6e3db47043e82e832da96091645a4c3a1|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "gcm.aes.jwk": "8b2382097009ce24a0da96447ccfc0a0c356e7490799c485695c07656d3791c66881713266eba40792f0abb316d8e6eb95c25daaa0f7e08b6cde198dd21f0e7950e4275dee07268e5a027d5dc51f20fcfcd3bbc7aab11e5171bb895aa257f69fc4613732f89ed4728ecc4b4550fd593aed46a0eac5|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "gcm.aes256.raw": "f92fb01d8ae9130ad0bddfdcfe6682b6b60a21f99ea9c9ad9a8fa9b95a66ec783a4664cf0d7b0470c4bf38e97f1037b7|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "gcm.aes256.jwk": "8b2382097009ce24a0d9914a78cecea0c356e7490799c485695c07656d3791c66881713266eba40792f0abb316d8e6eb95c25daaa0f7e7c56d934a82df050b6d40a17c64805a0ab81e233978c51f20f3fcc996d1aab61e3606bb8740b807b5cddf201e3ca7ef176add83b4189535a2708d58a2b875ec8376fb44019964ea35dd|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "cbc.hmac.raw": "161a90f9194455663badb63eee909e9502db890682dacd72b4125c57b9d5a9e525a0742fc974456baa4105dcd55fe0ea|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "cbc.hmac.jwk": "51d0660d56614bfa60ce2f2542c6d19280aef36d7ce6d92dca5bd00bb9560a16e08e658705298fa4c615337f67df5f6c31a86199ab9d90fe62131c2dc9526c8f5ba622475d0b2913250da54c72d01b2c4306b163149ebf3fbc9bfad0bf2595389485344e84f820de6600aa41be356692025e0319d21dfceafa7aa0bbd744cb35|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "cbc.hmac20.raw": "161a90f9194455663badb63eee909e95169d46c513984550e9c68f71037c626d|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "cbc.hmac20.jwk": "f20bf0e39cb98cec979248899fa90c24fe854f42b06971ea886ae160431cc7fc0ab6b4d01c8d8883d7bf31cfbcfe0e4c486b11302356370239fc353a6c76838535896705e0a138a46c5748b5a72232f3e3860d54fef2ee6847a7ec021e5efaf3|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "cbc.aes.raw": "baaf8a3694c4e8997507e146fa7e50cf4dc9cbb302dbeb1ef756abdfeabeb4c4|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "cbc.aes.jwk": "3ec443aec94835713f46cd0c91e58607f16d2bd0ac8d765e4afe70a0f1a9a0df15ce32991743e6a37982bb710ff1ce4ca157b9f6a91ac66707a848e98c2598bc832a9ef61ebaaabcee2df764ed21d134a7300e892c5a51f122b16d262d7474c7c7f0d816915af196bfec4a25b5616c3e|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "cbc.aes256.raw": "baaf8a3694c4e8997507e146fa7e50cfc8324f360794c03d893f7a37a5e0a1bfb9298ac68b779fdfec3bbaad8e207335|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "cbc.aes256.jwk": "e9d98b341c836b7fc3a4bffd77740364c98628d3e1ac8d841275971e55f2b39a52f2878592642957b39ce91f2b12d39b22f4943acaf562e853387e31fc178250e723d469ea949a3194c715484cb769218487dff06baf39248511e7d223f4a78c1f7d57ee5e64cc6880f9323c276c7299f591280a6440074275edf48ea5c371ae|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "ctr.hmac.raw": "ad09b0cf4f711d4d03b9670b5d56fd22e2aaeb25853d2fec6574853526c1a575|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "ctr.hmac.jwk": "de0683d4b492c1647bbf2fa1af9ddf33dfaf3dc053586bc280fd6db110c2d9b5d9918562dc030160484456c0854fbb7a4e359fc54295b453c9d23691edc86a999ace4aed303c2f96899b96a8e43d43391a691c644e8834e4caf188782c460f35ad10b526e8eeddad9b6b43465c195a3c3a9fac0c|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-256\"},\"length\":256}[\"sign\",\"verify\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
 "ctr.hmac20.raw": "ad09b0cf4f711d4d03b9670b5d56fd22e2aaeb25|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "ctr.hmac20.jwk": "de0683d4b492c1647bbf2cb6b59d9669cef573961b597c9bc7ba6de010bbb8c7c9c4e37cab4c2426586179e7bb4692694e13e1e04a92fe5f989867bfdceb5791a1e523e0134f69a781b896d9ea744d343c24563503d11be4c4|{\"name\":\"HMAC\",\"hash\":{\"name\":\"SHA-1\"},\"length\":160}[\"sign\"]true|082d52779cc1e60b30557a9fc4e90e33587da2c7",
 "ctr.aes.raw": "ac0ab1c04e721c4a02ba66345c55fc25|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "ctr.aes.jwk": "de0683d4b492c16472dd2facdefcbe3396f52c9a1d0e23c397a42af61093d9ccb8e38533bd1c09666c0256e1a568b3776626b5e723b3cc1e96962eb1c0cd678ea2b43b816a0874ad94afc4f0e47a042420651e6e1cc64d9b95ba846277481761a701a862ec|{\"name\":\"AES-GCM\",\"length\":128}[\"encrypt\",\"decrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34",
 "ctr.aes256.raw": "ac0ab1c04e721c4a02ba66345c55fc25e3a9ea2a843e2eeb6477842a27c2a472|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "ctr.aes256.jwk": "de0683d4b492c16472de28a2dafdb03396f52c9a1d0e23c397a42af61093d9ccb8e38533bd1c09666c0256e1a568b3776626b5e723b3cb5097db7dbecdd7629ab2f160b80455589bd08e80d5e47a042b207f33781cc14dfce2ba8a786d185433bc40816cb3a7f4f89b3a15501d575a2e|{\"name\":\"AES-CBC\",\"length\":256}[\"encrypt\"]true|092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84",
 "kw.ec.pkcs8": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "kw.ec.jwk": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "gcm.ec.pkcs8": "c0806467162bc415e7ec8efa7342b080ee728a1bf5f330cc1828654d226cd8fe4bc326272fb2b44a4aea90a9e08493a2afc4a7cfdbf5cc274fe649b4d6de7e2368aa90086d264baf287fd0c0c3faa12fa46cd7b1280c48663493da0058f9a5f6a2dd8d4ce9e3aa878dfeb1a4fca39a43cb7061b3f47320e579e5bdc64fb6e7c52ffce290e2349d421617d6a696b4868db3c8e33cddbb66472f91|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "gcm.ec.jwk": "222|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "cbc.ec.pkcs8": "a4e3e92458247c0e710ee0b9e3477b87c5000f501f0036959ad251c8048705da5e5e2a1534155e935d685217e9cc03c92ee18ae51488e6a1d8a5c93d2f08768d44c1a80b917482cd1d1d67b4140b8f029bc50c4067a87ea270e43b75834464ec28e598458dde745c3873f76e305b31d4fe91bc3ff676aa994e4b14c3d679021383369c75574c6f9e48ac8fd025c8a583|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "cbc.ec.jwk": "208|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "ctr.ec.pkcs8": "95a565bad2b0cb5535eb3712d171ce13bbd141c8ef64d78ae6d048de5fc890f49ba1d226f445192bb4186dfb5334c63e5c204f8258b1e0b2b5ae7e88c40c17d49afa8cd4e929198ce6d2696de29f85f778da72189e7b1bacd092d7388de64408c1bd121cfdab4915cb4710ec74c1621dce61efec05c3708e3d67a83bdf190a54d8f25c0994bfaca33e4c|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "ctr.ec.jwk": "206|{\"name\":\"ECDSA\",\"namedCurve\":\"P-256\"}privatefalse[\"sign\"]",
 "jwkText": "{\"alg\":\"HS256\",\"ext\":true,\"k\":\"CC1Sd5zB5gswVXqfxOkOM1h9osfsETZbgKXK7xQ5XoM\",\"key_ops\":[\"sign\",\"verify\"],\"kty\":\"oct\"}",
 "jwkTextEc": "{\"crv\":\"P-256\",\"d\":\"HTxafpsvTG2OChs8XX6fCitMbY4PGjtcfZ4fKktsjQ4\",\"ext\":true,\"key_ops\":[\"sign\"],\"kty\":\"EC\",\"x\":\"3ekkyaO3PdweD_LJdGppCjgug4xpSwnfzlxsZ8mUckc\",\"y\":\"J9MK4hpOl4phnZys9R7KrQKhLGiSvJIehTHECuSVmY0\"}",
 "jwkTextAes": "{\"alg\":\"A128GCM\",\"ext\":true,\"k\":\"CS5TeJ3C5wwxVnugxeoPNA\",\"key_ops\":[\"encrypt\",\"decrypt\"],\"kty\":\"oct\"}",
 "e.notExtractable": "InvalidAccessError: key is not extractable",
 "e.noWrapUsage": "InvalidAccessError: key.usages does not permit this operation",
 "e.noUnwrapUsage": "InvalidAccessError: key.usages does not permit this operation",
 "e.algMismatch": "InvalidAccessError: key.algorithm does not match that of operation",
 "e.encryptAlgAsWrap": "InvalidAccessError: key.algorithm does not match that of operation",
 "e.hmacWrapAlg": "NotSupportedError: Failed to execute 'wrapKey' on 'SubtleCrypto': HMAC: Unsupported operation: wrapKey",
 "e.ecdsaWrapAlg": "NotSupportedError: Failed to execute 'wrapKey' on 'SubtleCrypto': ECDSA: Unsupported operation: wrapKey",
 "e.kwJwkNot8": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "e.kwRawNot8": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "e.kwRaw20pad": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "e.badFormat": "NotSupportedError: Unsupported export key format for algorithm",
 "e.rawEc": "InvalidAccessError: The key is not of the expected type",
 "e.unwrapBadJson": "DataError: JWK input could not be parsed to a JSON dictionary",
 "e.unwrapJsonArray": "DataError: JWK input could not be parsed to a JSON dictionary",
 "e.unwrapJsonNum": "DataError: JWK input could not be parsed to a JSON dictionary",
 "e.unwrapJsonBom": "{\"name\":\"AES-GCM\",\"length\":128}",
 "e.unwrapJsonBadUtf8": "DataError: JWK input could not be parsed to a JSON dictionary",
 "e.unwrapTamper": "OperationError: ",
 "e.unwrapBadUsage": "SyntaxError: Cannot create a key using the specified key usages.",
 "e.unwrapBadUsageTamper": "OperationError: ",
 "e.unwrapKdfExt": "SyntaxError: KDF keys must set extractable=false",
 "e.unwrapKdf": "{\"name\":\"HKDF\"}false",
 "e.unwrapAesWrongLen": "DataError: AES key data must be 128 or 256 bits",
 "e.unwrapJwkAlgMismatch": "DataError: The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call",
 "e.unwrapJwkExtFalse": "DataError: The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call",
 "e.unwrapNonExtract": "false[\"encrypt\"]",
 "e.unwrapUnknownUsage": "TypeError: Failed to execute 'unwrapKey' on 'SubtleCrypto': Invalid keyUsages argument",
 "e.wrapKeyNotKey": "TypeError: Failed to execute 'wrapKey' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "e.unwrapKeyIsWrappingSelf": "7303454cef329cc2660c7f1604c4ce1c61f31e8ebd2cba83",
 "e.gcmTagLenWrap": "f92fb01d8ae9130ad0bddfdcfe6682b6e3db47043e82e832da960916092e53789dc2e70c31567ba0c5ea0f34",
 "e.decryptWithWrapOnly": "InvalidAccessError: key.usages does not permit this operation",
 "x.exportSpkiAes": "NotSupportedError: Unsupported export key format for algorithm",
 "x.exportPkcs8Hmac": "NotSupportedError: Unsupported export key format for algorithm",
 "x.exportBogus": "TypeError: Failed to execute 'exportKey' on 'SubtleCrypto': Invalid keyFormat argument: bogus",
 "x.encryptHmac": "NotSupportedError: Failed to execute 'encrypt' on 'SubtleCrypto': HMAC: Unsupported operation: encrypt",
 "x.decryptEcdsa": "NotSupportedError: Failed to execute 'decrypt' on 'SubtleCrypto': ECDSA: Unsupported operation: decrypt",
 "x.encryptMismatch": "InvalidAccessError: key.algorithm does not match that of operation",
 "x.encryptNotKey": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.signNotKey": "TypeError: Failed to execute 'sign' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.exportNotKey": "TypeError: Failed to execute 'exportKey' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.unwrapNotKey": "TypeError: Failed to execute 'unwrapKey' on 'SubtleCrypto': parameter 3 is not of type 'CryptoKey'.",
 "x.wrapWrappingNotKey": "TypeError: Failed to execute 'wrapKey' on 'SubtleCrypto': parameter 3 is not of type 'CryptoKey'.",
 "x.wrapBadFormat": "TypeError: Failed to execute 'wrapKey' on 'SubtleCrypto': Invalid keyFormat argument: bogus",
 "x.wrapNotExtractableAndNoUsage": "InvalidAccessError: key.usages does not permit this operation",
 "x.wrapSpkiAesThenNotExtract": "InvalidAccessError: key is not extractable",
 "x.unwrapImportAlgBad": "NotSupportedError: Failed to execute 'unwrapKey' on 'SubtleCrypto': Algorithm: Unrecognized name",
 "x.unwrapImportAlgVsUsage": "NotSupportedError: Failed to execute 'unwrapKey' on 'SubtleCrypto': Algorithm: Unrecognized name",
 "x.unwrapWithEncryptAlg": "NotSupportedError: Failed to execute 'unwrapKey' on 'SubtleCrypto': HMAC: Unsupported operation: unwrapKey",
 "x.unwrapFormatBad": "OperationError: ",
 "x.encryptUnknownAlg": "NotSupportedError: Failed to execute 'encrypt' on 'SubtleCrypto': Algorithm: Unrecognized name",
 "x.wrapUnknownAlg": "NotSupportedError: Failed to execute 'wrapKey' on 'SubtleCrypto': Algorithm: Unrecognized name",
 "t.ecSignPublic": "InvalidAccessError: key.usages does not permit this operation",
 "t.ecVerifyPrivate": "InvalidAccessError: key.usages does not permit this operation",
 "t.ecdhPublicBase": "InvalidAccessError: key.usages does not permit this operation",
 "t.ecPkcs8Public": "InvalidAccessError: The key is not of the expected type",
 "t.edSignPublic": "InvalidAccessError: key.usages does not permit this operation",
 "t.edVerifyPrivate": "InvalidAccessError: key.usages does not permit this operation",
 "t.edRawPrivate": "InvalidAccessError: The key is not of the expected type",
 "t.edPkcs8Public": "InvalidAccessError: The key is not of the expected type",
 "t.edSpkiPrivate": "InvalidAccessError: The key is not of the expected type",
 "x.importBogusFmt": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': Invalid keyFormat argument: bogus",
 "x.importBogusFmtBadAlg": "TypeError: Failed to execute 'importKey' on 'SubtleCrypto': Invalid keyFormat argument: bogus",
 "x.importBadAlg": "NotSupportedError: Failed to execute 'importKey' on 'SubtleCrypto': Algorithm: Unrecognized name",
 "x.verifyNotKey": "TypeError: Failed to execute 'verify' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.deriveBitsNotKey": "TypeError: Failed to execute 'deriveBits' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.deriveKeyNotKey": "TypeError: Failed to execute 'deriveKey' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.encryptNotKeyBadAlg": "TypeError: Failed to execute 'encrypt' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.wrapBadFmtNotKey": "TypeError: Failed to execute 'wrapKey' on 'SubtleCrypto': parameter 2 is not of type 'CryptoKey'.",
 "x.wrapBadFmtBadAlg": "TypeError: Failed to execute 'wrapKey' on 'SubtleCrypto': Invalid keyFormat argument: bogus",
 "x.encryptEd25519": "NotSupportedError: Failed to execute 'encrypt' on 'SubtleCrypto': Ed25519: Unsupported operation: encrypt",
 "x.encryptSha": "NotSupportedError: Failed to execute 'encrypt' on 'SubtleCrypto': SHA-256: Unsupported operation: encrypt",
 "x.encryptLower": "InvalidAccessError: key.algorithm does not match that of operation",
 "x.wrapEcRawPublic": "DataError: The AES-KW input data length is invalid: not a multiple of 8 bytes",
 "x.wrapEcSpkiPrivate": "InvalidAccessError: The key is not of the expected type",
 "x.exportEcSpkiPrivate": "InvalidAccessError: The key is not of the expected type",
 "x.exportEcRawPrivate": "InvalidAccessError: The key is not of the expected type"
}"#,
    )
    .unwrap();
    assert_eq!(result.value.unwrap(), expected);
}
