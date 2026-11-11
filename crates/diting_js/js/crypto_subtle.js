(function () { // @@SCOPE
// crypto.subtle (SubtleCrypto / CryptoKey) WebCrypto shim.
// Split out of bootstrap.js wholesale (god-file ratchet, pure move): top level only
// declares bindings; every bootstrap helper it names (_OPS, DOMException,
// __diting_clone_hooks) is touched at call time. Basic crypto.getRandomValues /
// randomUUID stay in bootstrap — this file only installs .subtle onto that object.

if (!globalThis.crypto) globalThis.crypto = {};
if (!globalThis.crypto.subtle) {
  // Real WebCrypto for the algorithms sites actually use: HMAC, AES-GCM/CBC/CTR,
  // PBKDF2, HKDF, ECDSA P-256/P-384 sign/verify, and ECDH key agreement
  // (deriveBits/deriveKey), plus raw/JWK key handling. The crypto
  // itself runs in Rust ops (RustCrypto); this shim only marshals bytes and
  // normalizes algorithm parameters. Other public-key algorithms (RSA*)
  // throw NotSupportedError rather than returning fake data.
  const keyMaterial = new WeakMap();

  class CryptoKey {
    constructor() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); }
    get [Symbol.toStringTag]() { return "CryptoKey"; }
  }
  function makeKey(type, extractable, algorithm, usages, bytes) {
    const k = Object.create(CryptoKey.prototype);
    // (刀75) plain assignments: type/extractable/algorithm/usages are prototype accessors
    // (lift_own) over hidden slots, so the instance carries no own keys like Chrome's.
    const ks = __diting_liftSlot(k);
    ks.type = type;
    ks.extractable = !!extractable;
    ks.algorithm = algorithm;
    // Chrome keeps usages as a mask: deduplicated, in the IDL enum's order
    // whatever order the caller listed them (Chrome 157, every key family).
    ks.usages = Object.freeze(KNOWN_USAGES.filter((u) => (usages || []).includes(u)));
    keyMaterial.set(k, bytes);
    return k;
  }
  function keyBytes(key) {
    if (!(key instanceof CryptoKey) || !keyMaterial.has(key)) {
      throw new DOMException("Argument is not a valid CryptoKey", "InvalidAccessError");
    }
    return keyMaterial.get(key);
  }

  const toBytes = (data) => {
    if (data instanceof ArrayBuffer) return new Uint8Array(data);
    if (ArrayBuffer.isView(data)) return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    return new Uint8Array(data || []);
  };
  const bufferOf = (u8) => new Uint8Array(u8).buffer;

  // ECDSA (P-256/P-384). The Rust ops return/accept a flat payload of four
  // u16-BE length-prefixed sections in this order: [pkcs8, sec1 point,
  // scalar, spki]. A key's keyMaterial entry is {curve, pkcs8, sec1, scalar,
  // spki} with the private-only sections null on public keys and vice versa.
  const ECDSA_CURVES = { "P-256": 32, "P-384": 48, "P-521": 66 };
  // The jwk "alg" an ECDSA key of each curve must carry when it has one (ECDH
  // keys are not alg-checked — Chrome 157).
  const ECDSA_JWK_ALG = { "P-256": "ES256", "P-384": "ES384", "P-521": "ES512" };
  function ecdsaUnflat(buf) {
    const parts = [];
    let off = 0;
    for (let i = 0; i < 4; i++) {
      const len = (buf[off] << 8) | buf[off + 1];
      off += 2;
      parts.push(buf.subarray(off, off + len));
      off += len;
    }
    return parts;
  }
  function ecMaterial(key) {
    if (!(key instanceof CryptoKey) || !keyMaterial.has(key) || !keyMaterial.get(key) || !keyMaterial.get(key).curve) {
      throw new DOMException("Argument is not a valid CryptoKey", "InvalidAccessError");
    }
    return keyMaterial.get(key);
  }
  function requireEcdsaUsages(usages, allowed) {
    for (const u of usages) {
      if (!allowed.includes(u)) {
        // Chrome 157's text (the old one read "must contain 'undefined'" for
        // an ECDH public key, whose allowed set is empty).
        throw new DOMException("Cannot create a key using the specified key usages.", "SyntaxError");
      }
    }
  }
  // EC import refusals: the ops (ecdsa.rs) raise one sentinel per Chrome 157
  // face; anything else they raise is malformed input too.
  const EC_FACES = {
    "EC key data invalid": ["", "DataError"],
    "EC curve mismatch": ["The imported EC key specifies a different curve than requested", "DataError"],
    "EC scalar invalid": ["", "OperationError"],
    "EC pair mismatch": ["The imported EC key is invalid", "DataError"],
  };
  function ecImportOp(fn) {
    try { return fn(); }
    catch (e) {
      if (e instanceof DOMException) throw e;
      const face = EC_FACES[e && e.message] || ["", "DataError"];
      throw new DOMException(face[0], face[1]);
    }
  }
  // Usage sets per EC algorithm face: ECDH derives, ECDSA signs.
  function ecUsagesFor(name, type) {
    if (name === "ECDH") return type === "private" ? ["deriveKey", "deriveBits"] : [];
    return type === "private" ? ["sign"] : ["verify"];
  }

  const ALGO_CANON = {
    "AES-CTR": "AES-CTR", "AES-CBC": "AES-CBC", "AES-GCM": "AES-GCM", "AES-KW": "AES-KW",
    "HMAC": "HMAC", "PBKDF2": "PBKDF2", "HKDF": "HKDF",
    "RSASSA-PKCS1-V1_5": "RSASSA-PKCS1-v1_5", "RSA-PSS": "RSA-PSS", "RSA-OAEP": "RSA-OAEP",
    "ECDSA": "ECDSA", "ECDH": "ECDH",
  };
  function normalizeAlgo(algorithm) {
    const a = typeof algorithm === "string" ? { name: algorithm } : (algorithm || {});
    const upper = String(a.name || "").toUpperCase();
    const name = ALGO_CANON[upper] || upper;
    return Object.assign({}, a, { name });
  }
  // SubtleCrypto hashes for HMAC/PBKDF2/HKDF and digest (SHA-1/256/384/512).
  function normalizeHash(h) {
    const n = (typeof h === "string" ? h : (h && h.name) || "").toUpperCase().replace("_", "-");
    if (n === "SHA-1" || n === "SHA-256" || n === "SHA-384" || n === "SHA-512") return n;
    throw new DOMException("Unsupported hash algorithm: " + (h && (h.name || h)), "NotSupportedError");
  }
  // An [EnforceRange] integer member of an algorithm dictionary (octet,
  // unsigned short, unsigned long — `max` 255 / 65535 / 2^32-1), converted
  // with Chrome's own WebCrypto messages (Chrome 157, probed): absent ->
  // "Missing required property" (or undefined back for an optional member);
  // ToNumber NaN (an explicit undefined, a non-numeric string, an object) ->
  // "Is not a number"; +-Infinity or outside [0, max] after truncation ->
  // "Outside of numeric range". 256.9 truncates to 256, null to 0 (left to
  // each algorithm's own range rule). The old reads were ad hoc: `>>> 0`
  // turned -1 into 4294967295, `x || default` took 0 as "absent".
  const OCTET = 255, USHORT = 65535, ULONG = 4294967295;
  function enforceIntMember(dict, member, op, dictName, max, optional) {
    const pre = "Failed to execute '" + op + "' on 'SubtleCrypto': " + dictName + ": " + member + ": ";
    if (!(member in dict)) {
      if (optional) return undefined;
      throw new TypeError(pre + "Missing required property");
    }
    const n = Number(dict[member]);
    if (Number.isNaN(n)) throw new TypeError(pre + "Is not a number");
    const t = Math.trunc(n);
    if (!Number.isFinite(t) || t < 0 || t > max) throw new TypeError(pre + "Outside of numeric range");
    return t + 0;
  }
  const hashBlockSize = (hash) => (hash === "SHA-384" || hash === "SHA-512" ? 128 : 64);

  function b64urlToBytes(s) {
    s = String(s).replace(/-/g, "+").replace(/_/g, "/");
    while (s.length % 4) s += "=";
    const bin = atob(s);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }
  function bytesToB64url(bytes) {
    let bin = "";
    for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  // Run an op, converting a Rust-side failure (bad GCM tag, bad CBC padding)
  // into the OperationError the WebCrypto spec requires. DOMExceptions we raise
  // ourselves pass through unchanged.
  function runOp(fn) {
    try { return fn(); }
    catch (e) {
      if (e instanceof DOMException) throw e;
      // `|| e` (not `?? e`): an empty-string message must survive as empty —
      // Chrome's corrupt-unwrap/corrupt-OAEP OperationError carries no text,
      // and falling back to `String(e)` would surface "Error" instead.
      const msg = e && e.message !== undefined ? String(e.message) : String(e);
      throw new DOMException(msg, "OperationError");
    }
  }

  // The CPU-heavy ops (RSA keygen, private-key sign + OAEP, PBKDF2) run off the JS
  // thread like Chrome's (#69): as sync ops a caller's chain of them ran as
  // one wall, and the eval patience watchdog beheaded it. Same error face as
  // runOp: a Rust-side failure is an OperationError with its text.
  async function offThread(kind, hash, a, b, ...bufs) {
    let b64;
    try { b64 = await _OPS.op_subtle_async(kind, hash || "", a, b, ...bufs); }
    catch (e) { throw new DOMException(e && e.message !== undefined ? String(e.message) : String(e), "OperationError"); }
    const bin = atob(b64);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  }

  function keyAlgorithmFor(alg, bytes) {
    switch (alg.name) {
      case "HMAC":
        return { name: "HMAC", hash: { name: normalizeHash(alg.hash) }, length: typeof alg.length === "number" ? alg.length : bytes.length * 8 };
      case "AES-CTR": case "AES-CBC": case "AES-GCM": case "AES-KW":
        // Chrome 155 dropped 192-bit AES (BoringSSL): a 24-byte key is an
        // OperationError, raw and jwk alike (Chrome 157).
        if (bytes.length === 24) {
          throw new DOMException("192-bit AES keys are not supported", "OperationError");
        }
        if (bytes.length !== 16 && bytes.length !== 32) {
          throw new DOMException("AES key data must be 128 or 256 bits", "DataError");
        }
        return { name: alg.name, length: bytes.length * 8 };
      default:
        throw new DOMException("Unsupported key algorithm: " + alg.name, "NotSupportedError");
    }
  }

  // ---- Chrome-155-aligned shared validation (issue #269) ----
  // The usage faces below are pinned from live Chrome 155, not from the spec:
  // an unrecognized usage string is a TypeError; empty usages are a
  // SyntaxError except on public keys; out-of-family usages are a different
  // SyntaxError.
  const KNOWN_USAGES = ["encrypt", "decrypt", "sign", "verify", "deriveKey", "deriveBits", "wrapKey", "unwrapKey"];
  function checkUsageStrings(method, usages) {
    for (const u of usages || []) {
      if (!KNOWN_USAGES.includes(u)) {
        throw new TypeError("Failed to execute '" + method + "' on 'SubtleCrypto': Invalid keyUsages argument");
      }
    }
  }
  function checkKeyUsages(usages, allowed, isPublic) {
    if (!usages || usages.length === 0) {
      if (!isPublic) {
        throw new DOMException("Usages cannot be empty when creating a key.", "SyntaxError");
      }
      return;
    }
    for (const u of usages) {
      if (!allowed.includes(u)) {
        throw new DOMException("Cannot create a key using the specified key usages.", "SyntaxError");
      }
    }
  }
  function jwkRequire(jwk, member) {
    if (jwk[member] === undefined || jwk[member] === null) {
      throw new DOMException("The required JWK member \"" + member + "\" was missing", "DataError");
    }
  }
  // Strict base64url: Chrome rejects any '=' padding and non-alphabet bytes.
  function jwkMemberBytes(jwk, member) {
    const s = jwk[member];
    if (typeof s !== "string" || !/^[A-Za-z0-9_-]+$/.test(s) || s.length % 4 === 1) {
      throw new DOMException("The JWK member \"" + member + "\" could not be base64url decoded or contained padding", "DataError");
    }
    return b64urlToBytes(s);
  }
  function jwkValidateCommon(jwk, extractable, usages, expectedAlg) {
    if (expectedAlg && jwk.alg !== undefined && jwk.alg !== expectedAlg) {
      throw new DOMException("The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call", "DataError");
    }
    if (jwk.use !== undefined) {
      const want = (jwk.use === "sig" || jwk.use === "enc") ? jwk.use : null;
      for (const u of usages) {
        const cat = (u === "sign" || u === "verify") ? "sig" : "enc";
        if (cat !== want) {
          throw new DOMException("The JWK \"use\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested", "DataError");
        }
      }
    }
    if (Array.isArray(jwk.key_ops)) {
      const seen = new Set();
      for (const op of jwk.key_ops) {
        if (seen.has(op)) {
          throw new DOMException("The \"key_ops\" member of the JWK dictionary contains duplicate usages.", "DataError");
        }
        seen.add(op);
      }
      for (const u of usages) {
        if (!jwk.key_ops.includes(u)) {
          throw new DOMException("The JWK \"key_ops\" member was inconsistent with that specified by the Web Crypto call. The JWK usage must be a superset of those requested", "DataError");
        }
      }
    }
    if (jwk.ext === false && extractable) {
      // Chrome's message carries this typo verbatim ("inconsistent what that").
      throw new DOMException("The \"ext\" member of the JWK dictionary is inconsistent what that specified by the Web Crypto call", "DataError");
    }
  }
  // Chrome requires a hash member on HMAC and RSA-OAEP imports/generation and
  // words the missing/unrecognized cases differently per param tag.
  function requireHashParam(alg, method, paramTag) {
    const pre = "Failed to execute '" + method + "' on 'SubtleCrypto': " + paramTag + ": hash: ";
    const h = alg.hash;
    if (h === undefined) throw new TypeError(pre + "Missing or not an AlgorithmIdentifier");
    // An object needs a name; every other value, and the name itself, is
    // read as a string (hash: 5, {name: 5}, null are unrecognized names, not
    // missing ones; "sha_256" is no alias — Chrome 157).
    if (h !== null && typeof h === "object" && h.name === undefined) {
      throw new TypeError(pre + "Algorithm: name: Missing or not a string");
    }
    const n = String(h !== null && typeof h === "object" ? h.name : h).toUpperCase();
    if (n !== "SHA-1" && n !== "SHA-256" && n !== "SHA-384" && n !== "SHA-512") {
      throw new DOMException(pre + "Algorithm: Unrecognized name", "NotSupportedError");
    }
    return n;
  }

  // A required BufferSource member of an algorithm dictionary, in Chrome's
  // normalize faces (157): undefined is missing, null or a string is not a
  // BufferSource (the old reads took both as empty).
  function requireBufferMember(alg, member, method, paramTag) {
    const pre = "Failed to execute '" + method + "' on 'SubtleCrypto': " + paramTag + ": " + member + ": ";
    const v = alg[member];
    if (v === undefined) throw new TypeError(pre + "Missing required property");
    if (!(v instanceof ArrayBuffer) && !ArrayBuffer.isView(v)) throw new TypeError(pre + "Not a BufferSource");
    return toBytes(v);
  }
  const isKdf = (name) => name === "HKDF" || name === "PBKDF2";
  // HkdfParams / Pbkdf2Params, members in Chrome's read order (HKDF: hash,
  // salt, info; PBKDF2: salt, iterations, hash). Read before the base key is
  // looked at — a bad dictionary wins over a wrong key.
  function kdfParams(alg, method) {
    if (alg.name === "HKDF") {
      const hash = requireHashParam(alg, method, "HkdfParams");
      return { hash, salt: requireBufferMember(alg, "salt", method, "HkdfParams"), info: requireBufferMember(alg, "info", method, "HkdfParams") };
    }
    const salt = requireBufferMember(alg, "salt", method, "Pbkdf2Params");
    const iterations = enforceIntMember(alg, "iterations", method, "Pbkdf2Params", ULONG);
    return { salt, iterations, hash: requireHashParam(alg, method, "Pbkdf2Params") };
  }
  function requireBaseKey(baseKey, alg, usage) {
    keyBytes(baseKey);
    if (baseKey.algorithm.name !== alg.name) {
      throw new DOMException("key.algorithm does not match that of operation", "InvalidAccessError");
    }
    if (usage && !baseKey.usages.includes(usage)) {
      throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
    }
  }
  const HASH_LEN = { "SHA-1": 20, "SHA-256": 32, "SHA-384": 48, "SHA-512": 64 };
  // HKDF/PBKDF2 bits after the dictionary is read; `usage` is the one the
  // base key must carry (deriveBits, or deriveKey when deriveKey calls in).
  // Check order and texts are Chrome 157's: algorithm, usages, missing
  // length, multiple of 8, then the per-algorithm bound.
  async function kdfDeriveBits(alg, p, baseKey, length, usage) {
    requireBaseKey(baseKey, alg, usage);
    if (length == null) {
      throw new DOMException("No length was specified for the " + alg.name + " Derive Bits operation.", "OperationError");
    }
    length = length >>> 0; // the IDL `unsigned long`: 16.7 -> 16, -8 -> 2^32-8
    const bytes = keyBytes(baseKey);
    if (alg.name === "HKDF") {
      if (length % 8) throw new DOMException("The length provided for HKDF is not a multiple of 8 bits.", "OperationError");
      if (length / 8 > 255 * HASH_LEN[p.hash]) throw new DOMException("The length provided for HKDF is too large.", "OperationError");
      return bufferOf(runOp(() => _OPS.op_subtle_hkdf(p.hash, bytes, p.salt, p.info, length / 8)));
    }
    if (length % 8) throw new DOMException("Length for PBKDF2 key derivation must be a multiple of 8 bits.", "OperationError");
    if (p.iterations === 0) throw new DOMException("PBKDF2 requires iterations > 0", "OperationError");
    return bufferOf(await offThread("pbkdf2", p.hash, p.iterations, length / 8, bytes, p.salt));
  }

  // RSA-OAEP key material: a ten-section flat payload from the Rust ops
  // [pkcs8, spki, n, e, d, p, q, dp, dq, qi]; public-only imports carry
  // empty private sections. JWK alg names per OAEP hash (SHA-1 has no suffix).
  const RSA_OAEP_JWK_ALG = { "SHA-1": "RSA-OAEP", "SHA-256": "RSA-OAEP-256", "SHA-384": "RSA-OAEP-384", "SHA-512": "RSA-OAEP-512" };
  const RSA_OAEP_PUB_USAGES = ["encrypt", "wrapKey"];
  const RSA_OAEP_PRIV_USAGES = ["decrypt", "unwrapKey"];
  // #273: RSASSA-PKCS1-v1_5 (JWT RS256) — usages split sign/verify, JWK
  // algs RS1/RS256/RS384/RS512, pinned from Chrome 155.
  const RSASSA_JWK_ALG = { "SHA-1": "RS1", "SHA-256": "RS256", "SHA-384": "RS384", "SHA-512": "RS512" };
  // RSA-PSS keys are RSASSA-shaped (sign/verify halves, rsaEncryption OID on
  // spki/pkcs8 export — Chrome 157); only the jwk alg family differs.
  const RSA_PSS_JWK_ALG = { "SHA-1": "PS1", "SHA-256": "PS256", "SHA-384": "PS384", "SHA-512": "PS512" };
  const RSASSA_PUB_USAGES = ["verify"];
  const RSASSA_PRIV_USAGES = ["sign"];
  function rsaFace(name) {
    if (name === "RSASSA-PKCS1-v1_5") return { pub: RSASSA_PUB_USAGES, priv: RSASSA_PRIV_USAGES, alg: RSASSA_JWK_ALG };
    if (name === "RSA-PSS") return { pub: RSASSA_PUB_USAGES, priv: RSASSA_PRIV_USAGES, alg: RSA_PSS_JWK_ALG };
    return { pub: RSA_OAEP_PUB_USAGES, priv: RSA_OAEP_PRIV_USAGES, alg: RSA_OAEP_JWK_ALG };
  }
  function rsaUnflat(buf) {
    const parts = [];
    let off = 0;
    for (let i = 0; i < 10; i++) {
      const len = (buf[off] << 8) | buf[off + 1];
      off += 2;
      parts.push(len ? buf.subarray(off, off + len) : null);
      off += len;
    }
    return { pkcs8: parts[0], spki: parts[1], n: parts[2], e: parts[3], d: parts[4], p: parts[5], q: parts[6], dp: parts[7], dq: parts[8], qi: parts[9] };
  }
  function rsaMaterial(key) {
    if (!(key instanceof CryptoKey) || !keyMaterial.has(key) || !keyMaterial.get(key) || keyMaterial.get(key).spki == null) {
      throw new DOMException("Argument is not a valid CryptoKey", "InvalidAccessError");
    }
    return keyMaterial.get(key);
  }
  // RSA import (spki / pkcs8 / jwk), shared by the OAEP and RSASSA faces
  // (the key DER is algorithm-agnostic; only the usage sets and JWK alg
  // names differ). The jwk ladder follows Chrome's order: kty → usage set
  // (which depends on d's presence) → required members n,e,d,p,q,dp,dq,qi
  // → strict b64url → alg/use/key_ops/ext.
  // Under 512 bits Chrome 157 refuses an RSA import with an empty DataError —
  // or an empty OperationError for a jwk public key; the ops flag the size
  // with one sentinel message and each format maps it here.
  const RSA_TOO_SMALL = "RSA modulus too small";
  // Every PKCS#8 / SPKI RSA key Chrome would refuse (malformed, multi-prime,
  // inconsistent CRT members, non-NULL SPKI parameters, ...) is one empty
  // DataError there.
  const RSA_KEY_INVALID = "RSA key data invalid";
  function rsaImportOp(fn, smallFace) {
    try { return fn(); }
    catch (e) {
      if (e && e.message === RSA_TOO_SMALL) throw new DOMException("", smallFace);
      if (e && e.message === RSA_KEY_INVALID) throw new DOMException("", "DataError");
      throw e;
    }
  }
  // An RSA jwk member is a big-endian unsigned integer in minimal form:
  // Chrome 157 refuses an empty one and one with a leading zero byte, for
  // every member (n, e, d, p, q, dp, dq, qi) — probed.
  // A lone zero byte ("AA") is the value 0, not a leading zero — it fails
  // later as an invalid key, the way Chrome does.
  function rsaJwkUint(jwk, mem) {
    if (jwk[mem] === "") throw new DOMException("The JWK \"" + mem + "\" member was empty.", "DataError");
    const b = jwkMemberBytes(jwk, mem);
    if (b.length === 0) throw new DOMException("The JWK \"" + mem + "\" member was empty.", "DataError");
    if (b.length > 1 && b[0] === 0) throw new DOMException("The JWK \"" + mem + "\" member contained a leading zero.", "DataError");
    return b;
  }
  // Big-endian integers equal regardless of leading zero bytes.
  function sameUint(a, b) {
    let i = 0, j = 0;
    while (i < a.length && a[i] === 0) i++;
    while (j < b.length && b[j] === 0) j++;
    if (a.length - i !== b.length - j) return false;
    for (; i < a.length; i++, j++) if (a[i] !== b[j]) return false;
    return true;
  }
  function rsaImportKey(format, keyData, alg, extractable, usages) {
    checkUsageStrings("importKey", usages);
    const hash = requireHashParam(alg, "importKey", "RsaHashedImportParams");
    const face = rsaFace(alg.name);
    let m, type;
    if (format === "spki") {
      checkKeyUsages(usages, face.pub, true);
      m = rsaUnflat(runOp(() => rsaImportOp(() => _OPS.op_subtle_rsa_import_spki(toBytes(keyData)), "DataError")));
      type = "public";
    } else if (format === "pkcs8") {
      checkKeyUsages(usages, face.priv, false);
      m = rsaUnflat(runOp(() => rsaImportOp(() => _OPS.op_subtle_rsa_import_pkcs8(toBytes(keyData)), "DataError")));
      type = "private";
    } else if (format === "jwk") {
      const jwk = keyData;
      if (!jwk || jwk.kty !== "RSA") {
        throw new DOMException("The JWK \"kty\" member was not \"RSA\"", "DataError");
      }
      const isPriv = typeof jwk.d === "string";
      checkKeyUsages(usages, isPriv ? face.priv : face.pub, !isPriv);
      jwkRequire(jwk, "n");
      jwkRequire(jwk, "e");
      const n = rsaJwkUint(jwk, "n");
      const e = rsaJwkUint(jwk, "e");
      const expected = face.alg[hash];
      if (isPriv) {
        for (const mem of ["d", "p", "q", "dp", "dq", "qi"]) jwkRequire(jwk, mem);
        const d = rsaJwkUint(jwk, "d");
        const p = rsaJwkUint(jwk, "p");
        const q = rsaJwkUint(jwk, "q");
        const crt = { dp: rsaJwkUint(jwk, "dp"), dq: rsaJwkUint(jwk, "dq"), qi: rsaJwkUint(jwk, "qi") };
        jwkValidateCommon(jwk, extractable, usages, expected);
        m = rsaUnflat(runOp(() => rsaImportOp(() => _OPS.op_subtle_rsa_import_private(n, e, d, p, q), "DataError")));
        // The key is rebuilt from (n, e, d, p, q); the CRT members the jwk
        // carries must be the ones those imply — Chrome 157 (BoringSSL
        // checks the key) refuses a mismatch with an empty DataError, where
        // we used to recompute and silently accept.
        for (const mem of ["dp", "dq", "qi"]) {
          if (!sameUint(crt[mem], m[mem])) throw new DOMException("", "DataError");
        }
        type = "private";
      } else {
        jwkValidateCommon(jwk, extractable, usages, expected);
        m = rsaUnflat(runOp(() => rsaImportOp(() => _OPS.op_subtle_rsa_import_public(n, e), "OperationError")));
        type = "public";
      }
    } else {
      throw new DOMException("Unsupported key format for RSA keys: " + format, "NotSupportedError");
    }
    const algorithm = {
      name: alg.name, modulusLength: m.n.length * 8,
      publicExponent: new Uint8Array(m.e), hash: { name: hash },
    };
    return makeKey(type, extractable, algorithm, usages, m);
  }
  // Chrome's RSA generateKey policy (pinned from 155, both faces): exponent
  // must be 3 or 65537, modulus a multiple of 8 in [256, 8192], and the
  // usages split across the halves — encrypt/wrapKey+decrypt/unwrapKey on
  // OAEP, verify/sign on RSASSA (the public key is always extractable).
  function rsaGenerateKey(alg, extractable, usages) {
    checkUsageStrings("generateKey", usages);
    const hash = requireHashParam(alg, "generateKey", "RsaHashedKeyGenParams");
    const face = rsaFace(alg.name);
    const combined = face.pub.concat(face.priv);
    checkKeyUsages(usages, combined, false);
    const bits = enforceIntMember(alg, "modulusLength", "generateKey", "RsaHashedKeyGenParams", ULONG);
    if (!Number.isInteger(bits) || bits < 256 || bits > 8192 || bits % 8 !== 0) {
      throw new DOMException("The modulus length must be a multiple of 8 bits and >= 256 and <= 8192", "OperationError");
    }
    const eBytes = toBytes(alg.publicExponent);
    let ev = 0n;
    for (const b of eBytes) ev = (ev << 8n) | BigInt(b);
    if (ev !== 3n && ev !== 65537n) {
      throw new DOMException("The \"publicExponent\" must be either 3 or 65537", "OperationError");
    }
    // A plain function returning the async tail, not an async function: the
    // generated exports rely on Annex B block hoisting.
    // Chrome's BoringSSL keygen rounds the size down to a multiple of 128 and
    // reports what it made (Chrome 157: 520 -> 512, 1016 -> 896, 1032 ->
    // 1024); under 512 after rounding it refuses.
    const made = Math.floor(bits / 128) * 128;
    return (async () => {
      // A refused keygen is an OperationError with no text (Chrome 157).
      let flat;
      try { flat = await offThread("rsa_generate", "", made, Number(ev)); }
      catch (e) { throw new DOMException("", "OperationError"); }
      const m = rsaUnflat(flat);
      const algorithm = {
        name: alg.name, modulusLength: made,
        publicExponent: new Uint8Array(m.e), hash: { name: hash },
      };
      const pub = makeKey("public", true, algorithm, usages.filter((u) => face.pub.includes(u)), m);
      const priv = makeKey("private", extractable, algorithm, usages.filter((u) => face.priv.includes(u)), m);
      return { publicKey: pub, privateKey: priv };
    })();
  }

  const subtle = {
    async digest(algorithm, data) {
      const name = (typeof algorithm === "string" ? algorithm : algorithm && algorithm.name || "").toUpperCase().replace("_", "-");
      if (name !== "SHA-1" && name !== "SHA-256" && name !== "SHA-384" && name !== "SHA-512" &&
          name !== "SHA-512/224" && name !== "SHA-512/256") {
        throw new DOMException("Unrecognized algorithm name", "NotSupportedError");
      }
      return bufferOf(_OPS.op_subtle_digest(name, toBytes(data)));
    },

    async importKey(format, keyData, algorithm, extractable, keyUsages) {
      const alg = normalizeAlgo(algorithm);
      if (alg.name === "ECDSA" || alg.name === "ECDH") {
        const crv = String(alg.namedCurve || "");
        if (!(crv in ECDSA_CURVES)) {
          throw new DOMException("Unrecognized namedCurve '" + crv + "'", "NotSupportedError");
        }
        const usages = keyUsages || [];
        const coordLen = ECDSA_CURVES[crv];
        let type, m;
        const importPublic = (bytes) => {
          const parts = ecdsaUnflat(ecImportOp(() => _OPS.op_subtle_ecdsa_import_public(crv, bytes)));
          return { curve: crv, pkcs8: null, sec1: parts[1], scalar: null, spki: parts[3] };
        };
        const importPrivate = (isScalar, bytes, expectSec1) => {
          const parts = ecdsaUnflat(ecImportOp(() => _OPS.op_subtle_ecdsa_import_private(crv, isScalar, bytes, expectSec1)));
          return { curve: crv, pkcs8: parts[0], sec1: parts[1], scalar: parts[2], spki: parts[3] };
        };
        if (format === "raw") {
          // Compressed points included (Chrome 157 takes them).
          m = importPublic(toBytes(keyData));
          type = "public";
          requireEcdsaUsages(usages, ecUsagesFor(alg.name, "public"));
        } else if (format === "spki") {
          m = importPublic(toBytes(keyData));
          type = "public";
          requireEcdsaUsages(usages, ecUsagesFor(alg.name, "public"));
        } else if (format === "pkcs8") {
          m = importPrivate(false, toBytes(keyData), new Uint8Array(0));
          type = "private";
          requireEcdsaUsages(usages, ecUsagesFor(alg.name, "private"));
        } else if (format === "jwk") {
          // Chrome 157's order and texts (probed): kty, the common members
          // (ECDSA checks alg ES256/ES384 and use; ECDH neither), crv, x, y,
          // then d; coordinates are fixed-length octet strings.
          const jwk = keyData;
          if (!jwk || jwk.kty !== "EC") {
            throw new DOMException("The JWK \"kty\" member was not \"EC\"", "DataError");
          }
          const isPriv = jwk.d !== undefined;
          jwkValidateCommon(jwk, extractable, usages, alg.name === "ECDSA" ? ECDSA_JWK_ALG[crv] : null);
          if (jwk.crv === undefined) throw new DOMException("The required JWK member \"crv\" was missing", "DataError");
          if (jwk.crv !== crv) {
            throw new DOMException("The JWK's \"crv\" member specifies a different curve than requested", "DataError");
          }
          const coord = (mem) => {
            jwkRequire(jwk, mem);
            // "" is a zero-length octet string here, failing on length.
            const b = jwk[mem] === "" ? new Uint8Array(0) : jwkMemberBytes(jwk, mem);
            if (b.length !== coordLen) {
              throw new DOMException("The JWK's \"" + mem + "\" member defines an octet string of length " + b.length + " bytes but should be " + coordLen, "DataError");
            }
            return b;
          };
          const x = coord("x"), y = coord("y");
          const point = new Uint8Array(coordLen * 2 + 1);
          point[0] = 4;
          point.set(x, 1);
          point.set(y, coordLen + 1);
          if (isPriv) {
            const d = coord("d");
            // The point must lie on the curve before d is tried against it.
            importPublic(point);
            m = importPrivate(true, d, point);
            type = "private";
            requireEcdsaUsages(usages, ecUsagesFor(alg.name, "private"));
          } else {
            m = importPublic(point);
            type = "public";
            requireEcdsaUsages(usages, ecUsagesFor(alg.name, "public"));
          }
        } else {
          throw new DOMException("Unsupported key format for EC keys: " + format, "NotSupportedError");
        }
        return makeKey(type, extractable, { name: alg.name, namedCurve: crv }, usages, m);
      }
      if (alg.name === "RSA-OAEP" || alg.name === "RSASSA-PKCS1-v1_5" || alg.name === "RSA-PSS") {
        return rsaImportKey(format, keyData, alg, extractable, keyUsages || []);
      }
      checkUsageStrings("importKey", keyUsages || []);
      if (isKdf(alg.name)) {
        // Chrome 157's order: format, usage family, extractable, then empty.
        if (format !== "raw") throw new DOMException("Unsupported import key format for algorithm", "NotSupportedError");
        const usages = keyUsages || [];
        if (usages.some((u) => u !== "deriveKey" && u !== "deriveBits")) {
          throw new DOMException("Cannot create a key using the specified key usages.", "SyntaxError");
        }
        if (extractable) throw new DOMException("KDF keys must set extractable=false", "SyntaxError");
        if (usages.length === 0) throw new DOMException("Usages cannot be empty when creating a key.", "SyntaxError");
        return makeKey("secret", false, { name: alg.name }, usages, toBytes(keyData));
      }
      let bytes;
      if (format === "raw") {
        bytes = toBytes(keyData);
      } else if (format === "jwk") {
        if (!keyData || keyData.kty !== "oct") {
          throw new DOMException("The JWK \"kty\" member was not \"oct\"", "DataError");
        }
        jwkRequire(keyData, "k");
        // "" is zero bytes of key — each algorithm's own length rule names it
        // (Chrome 157), not a base64 failure. A 24-byte k is 192-bit AES:
        // the shared OperationError below, as for raw (Chrome 157; 155's jwk
        // path said DataError).
        bytes = keyData.k === "" ? new Uint8Array(0) : jwkMemberBytes(keyData, "k");
      } else {
        throw new DOMException("Only 'raw' and symmetric 'jwk' key formats are supported", "NotSupportedError");
      }
      let expectedAlg = null;
      if (alg.name === "HMAC") {
        const hash = requireHashParam(alg, "importKey", "HmacImportParams");
        // Before the length rule — even an explicit length: 0 (Chrome 157).
        if (bytes.length === 0) throw new DOMException("HMAC key data must not be empty", "DataError");
        const L = enforceIntMember(alg, "length", "importKey", "HmacImportParams", ULONG, true);
        if (L !== undefined) {
          // Chrome: within the key's last byte — (bytes*8 - 8, bytes*8].
          if (L > bytes.length * 8 || L <= bytes.length * 8 - 8) {
            throw new DOMException("The optional HMAC key length must be shorter than the key data, and by no more than 7 bits.", "DataError");
          }
          alg.length = L;
        } else {
          delete alg.length;
        }
        checkKeyUsages(keyUsages || [], ["sign", "verify"], false);
        // "SHA-1".slice(4) === "1" too — no special case needed.
        expectedAlg = "HS" + hash.slice(4);
      } else if (alg.name === "AES-CTR" || alg.name === "AES-CBC" || alg.name === "AES-GCM" || alg.name === "AES-KW") {
        checkKeyUsages(keyUsages || [],
          alg.name === "AES-KW" ? ["wrapKey", "unwrapKey"] : ["encrypt", "decrypt", "wrapKey", "unwrapKey"], false);
        // A jwk alg of another AES mode (or no AES alg at all) is the generic
        // inconsistency; the right mode at another size says the key length
        // is wrong for it (Chrome 157).
        if (format === "jwk" && keyData.alg !== undefined) {
          const am = /^A(128|192|256)(GCM|CBC|CTR|KW)$/.exec(keyData.alg);
          if (!am || am[2] !== alg.name.slice(4)) {
            throw new DOMException("The JWK \"alg\" member was inconsistent with that specified by the Web Crypto call", "DataError");
          }
          if (Number(am[1]) !== bytes.length * 8) {
            throw new DOMException("The JWK \"k\" member did not include the right length of key data for the given algorithm.", "DataError");
          }
        }
      }
      if (format === "jwk") {
        jwkValidateCommon(keyData, extractable, keyUsages || [], expectedAlg);
      }
      return makeKey("secret", extractable, keyAlgorithmFor(alg, bytes), keyUsages, bytes);
    },

    async exportKey(format, key) {
      const bytes = keyBytes(key);
      if (isKdf(key.algorithm.name)) {
        throw new DOMException("Failed to execute 'exportKey' on 'SubtleCrypto': " + key.algorithm.name + ": Unsupported operation: exportKey", "NotSupportedError");
      }
      if (!key.extractable) throw new DOMException("Failed to execute 'exportKey' on 'SubtleCrypto': key is not extractable", "InvalidAccessError");
      if (key.algorithm && (key.algorithm.name === "ECDSA" || key.algorithm.name === "ECDH")) {
        const m = ecMaterial(key);
        if (format === "raw" || format === "spki") {
          if (key.type !== "public") throw new DOMException("Key is not of type 'public'", "InvalidAccessError");
          return bufferOf(format === "raw" ? m.sec1 : m.spki);
        }
        if (format === "pkcs8") {
          if (key.type !== "private") throw new DOMException("Key is not of type 'private'", "InvalidAccessError");
          return bufferOf(m.pkcs8);
        }
        if (format === "jwk") {
          const coordLen = ECDSA_CURVES[m.curve];
          // Chrome emits EC JWK fields in alphabetical insertion order.
          const jwk = { crv: m.curve };
          if (key.type === "private") jwk.d = bytesToB64url(m.scalar);
          jwk.ext = key.extractable;
          jwk.key_ops = key.usages.slice();
          jwk.kty = "EC";
          jwk.x = bytesToB64url(m.sec1.subarray(1, 1 + coordLen));
          jwk.y = bytesToB64url(m.sec1.subarray(1 + coordLen));
          return jwk;
        }
        throw new DOMException("Unsupported key format for EC keys export: " + format, "NotSupportedError");
      }
      if (key.algorithm && (key.algorithm.name === "RSA-OAEP" || key.algorithm.name === "RSASSA-PKCS1-v1_5" || key.algorithm.name === "RSA-PSS")) {
        const m = rsaMaterial(key);
        if (format === "spki") {
          if (key.type !== "public") throw new DOMException("The key is not of the expected type", "InvalidAccessError");
          return bufferOf(m.spki);
        }
        if (format === "pkcs8") {
          if (key.type !== "private") throw new DOMException("The key is not of the expected type", "InvalidAccessError");
          return bufferOf(m.pkcs8);
        }
        if (format === "jwk") {
          // Chrome emits RSA JWK fields in alphabetical insertion order; the
          // alg name is face-specific (RSA-OAEP-256 vs RS256).
          const jwk = {};
          const algName = key.algorithm.hash && rsaFace(key.algorithm.name).alg[key.algorithm.hash.name];
          if (algName) jwk.alg = algName;
          if (key.type === "private") {
            jwk.d = bytesToB64url(m.d);
            jwk.dp = bytesToB64url(m.dp);
            jwk.dq = bytesToB64url(m.dq);
          }
          jwk.e = bytesToB64url(m.e);
          jwk.ext = key.extractable;
          jwk.key_ops = key.usages.slice();
          jwk.kty = "RSA";
          jwk.n = bytesToB64url(m.n);
          if (key.type === "private") {
            jwk.p = bytesToB64url(m.p);
            jwk.q = bytesToB64url(m.q);
            jwk.qi = bytesToB64url(m.qi);
          }
          return jwk;
        }
        if (format === "raw") {
          throw new DOMException("Unsupported export key format for algorithm", "NotSupportedError");
        }
        throw new DOMException("Unsupported key format for RSA keys export: " + format, "NotSupportedError");
      }
      if (format === "raw") return bufferOf(bytes);
      if (format === "jwk") {
        // Chrome emits oct JWK fields in alphabetical insertion order.
        const jwk = {};
        if (key.algorithm.name && key.algorithm.name.indexOf("AES-") === 0) {
          jwk.alg = "A" + (bytes.length * 8) + key.algorithm.name.slice(4);
        } else if (key.algorithm.name === "HMAC") {
          jwk.alg = "HS" + key.algorithm.hash.name.slice(4);
        }
        jwk.ext = key.extractable;
        jwk.k = bytesToB64url(bytes);
        jwk.key_ops = key.usages.slice();
        jwk.kty = "oct";
        return jwk;
      }
      throw new DOMException("Only 'raw' and 'jwk' export is supported", "NotSupportedError");
    },

    async generateKey(algorithm, extractable, keyUsages) {
      const alg = normalizeAlgo(algorithm);
      if (alg.name === "RSA-OAEP" || alg.name === "RSASSA-PKCS1-v1_5" || alg.name === "RSA-PSS") {
        return rsaGenerateKey(alg, extractable, keyUsages || []);
      }
      if (alg.name === "HMAC") {
        checkUsageStrings("generateKey", keyUsages || []);
        const hash = requireHashParam(alg, "generateKey", "HmacKeyGenParams");
        checkKeyUsages(keyUsages || [], ["sign", "verify"], false);
        const bitsLen = enforceIntMember(alg, "length", "generateKey", "HmacKeyGenParams", ULONG, true);
        if (bitsLen === 0) throw new DOMException("HMAC key length must not be zero", "OperationError");
        const len = bitsLen !== undefined ? Math.ceil(bitsLen / 8) : hashBlockSize(hash);
        const bytes = _OPS.op_random_bytes(len);
        return makeKey("secret", extractable, { name: "HMAC", hash: { name: hash }, length: len * 8 }, keyUsages, bytes);
      }
      if (alg.name === "AES-CTR" || alg.name === "AES-CBC" || alg.name === "AES-GCM" || alg.name === "AES-KW") {
        checkUsageStrings("generateKey", keyUsages || []);
        alg.length = enforceIntMember(alg, "length", "generateKey", "AesKeyGenParams", USHORT);
        if (alg.length === 192) {
          throw new DOMException("192-bit AES keys are not supported", "OperationError");
        }
        if (alg.length !== 128 && alg.length !== 256) {
          throw new DOMException("AES key length must be 128 or 256 bits", "OperationError");
        }
        checkKeyUsages(keyUsages || [],
          alg.name === "AES-KW" ? ["wrapKey", "unwrapKey"] : ["encrypt", "decrypt", "wrapKey", "unwrapKey"], false);
        const bytes = _OPS.op_random_bytes(alg.length / 8);
        return makeKey("secret", extractable, { name: alg.name, length: alg.length }, keyUsages, bytes);
      }
      if (alg.name === "ECDSA" || alg.name === "ECDH") {
        const crv = String(alg.namedCurve || "");
        if (!(crv in ECDSA_CURVES)) {
          throw new DOMException("Unrecognized namedCurve '" + crv + "'", "NotSupportedError");
        }
        const usages = keyUsages || [];
        if (alg.name === "ECDSA") {
          if (usages.some((u) => u !== "sign" && u !== "verify") || (!usages.includes("sign") && !usages.includes("verify"))) {
            throw new DOMException("The key usages must contain 'sign' or 'verify'", "SyntaxError");
          }
        } else if (usages.some((u) => u !== "deriveKey" && u !== "deriveBits") || usages.length === 0) {
          throw new DOMException("The key usages must contain 'deriveKey' or 'deriveBits'", "SyntaxError");
        }
        const parts = ecdsaUnflat(_OPS.op_subtle_ecdsa_generate(crv));
        const m = { curve: crv, pkcs8: parts[0], sec1: parts[1], scalar: parts[2], spki: parts[3] };
        const privUsages = usages.filter((u) => ecUsagesFor(alg.name, "private").includes(u));
        const pub = makeKey("public", true, { name: alg.name, namedCurve: crv }, ecUsagesFor(alg.name, "public"), m);
        const priv = makeKey("private", extractable, { name: alg.name, namedCurve: crv }, privUsages, m);
        return { publicKey: pub, privateKey: priv };
      }
      if (isKdf(alg.name)) {
        throw new DOMException("Failed to execute 'generateKey' on 'SubtleCrypto': " + alg.name + ": Unsupported operation: generateKey", "NotSupportedError");
      }
      throw new DOMException("generateKey does not support " + alg.name, "NotSupportedError");
    },

    async sign(algorithm, key, data) {
      const alg = normalizeAlgo(algorithm);
      if (alg.name === "ECDSA") {
        const m = ecMaterial(key);
        if (key.type !== "private") throw new DOMException("Key is not of type 'private'", "InvalidAccessError");
        if (!key.usages.includes("sign")) throw new DOMException("This CryptoKey cannot be used for signing", "InvalidAccessError");
        const hash = normalizeHash(alg.hash || (key.algorithm.hash && key.algorithm.hash.name));
        return bufferOf(runOp(() => _OPS.op_subtle_ecdsa_sign(m.curve, hash, m.pkcs8, toBytes(data))));
      }
      if (alg.name === "RSASSA-PKCS1-v1_5" || alg.name === "RSA-PSS") {
        // Chrome's order: params convert first, then the key must be of the
        // operation's algorithm, then its usages.
        const salt = alg.name === "RSA-PSS" ? enforceIntMember(alg, "saltLength", "sign", "RsaPssParams", ULONG) : 0;
        const m = rsaMaterial(key);
        if (key.algorithm.name !== alg.name) {
          throw new DOMException("key.algorithm does not match that of operation", "InvalidAccessError");
        }
        // Chrome pins one message for every failure shape here — public key,
        // missing usage — unlike the ECDSA wording above.
        if (key.type !== "private" || !key.usages.includes("sign")) {
          throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
        }
        // The hash rides the key; a hash param on sign() is ignored
        // (Chrome-pinned: same signature with and without).
        const hash = key.algorithm.hash && key.algorithm.hash.name;
        // PSS: a salt the modulus cannot hold is an empty OperationError.
        if (alg.name === "RSA-PSS") return bufferOf(await offThread("rsa_pss_sign", hash, salt, 0, m.n, m.e, m.d, m.p, m.q, toBytes(data)));
        return bufferOf(await offThread("rsa_sign", hash, 0, 0, m.n, m.e, m.d, m.p, m.q, toBytes(data)));
      }
      const bytes = keyBytes(key);
      if (alg.name === "HMAC") {
        const hash = key.algorithm && key.algorithm.hash ? key.algorithm.hash.name : normalizeHash(alg.hash);
        return bufferOf(runOp(() => _OPS.op_subtle_hmac(hash, bytes, toBytes(data))));
      }
      throw new DOMException("sign does not support " + alg.name, "NotSupportedError");
    },

    async verify(algorithm, key, signature, data) {
      const alg = normalizeAlgo(algorithm);
      if (alg.name === "ECDSA") {
        const m = ecMaterial(key);
        if (key.type !== "public") throw new DOMException("Key is not of type 'public'", "InvalidAccessError");
        if (!key.usages.includes("verify")) throw new DOMException("This CryptoKey cannot be used for verifying", "InvalidAccessError");
        const hash = normalizeHash(alg.hash || (key.algorithm.hash && key.algorithm.hash.name));
        return runOp(() => _OPS.op_subtle_ecdsa_verify(m.curve, hash, m.sec1, toBytes(signature), toBytes(data)));
      }
      if (alg.name === "RSASSA-PKCS1-v1_5" || alg.name === "RSA-PSS") {
        const salt = alg.name === "RSA-PSS" ? enforceIntMember(alg, "saltLength", "verify", "RsaPssParams", ULONG) : 0;
        const m = rsaMaterial(key);
        if (key.algorithm.name !== alg.name) {
          throw new DOMException("key.algorithm does not match that of operation", "InvalidAccessError");
        }
        if (key.type !== "public" || !key.usages.includes("verify")) {
          throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
        }
        const hash = key.algorithm.hash && key.algorithm.hash.name;
        // PSS: any mismatch (salt length, oversize salt, bad signature) is false.
        if (alg.name === "RSA-PSS") return runOp(() => _OPS.op_subtle_rsa_pss_verify(hash, m.n, m.e, toBytes(signature), toBytes(data), salt));
        return runOp(() => _OPS.op_subtle_rsa_pkcs1v15_verify(hash, m.n, m.e, toBytes(signature), toBytes(data)));
      }
      const bytes = keyBytes(key);
      if (alg.name === "HMAC") {
        const hash = key.algorithm && key.algorithm.hash ? key.algorithm.hash.name : normalizeHash(alg.hash);
        const mac = runOp(() => _OPS.op_subtle_hmac(hash, bytes, toBytes(data)));
        const sig = toBytes(signature);
        if (sig.length !== mac.length) return false;
        let diff = 0;
        for (let i = 0; i < mac.length; i++) diff |= mac[i] ^ sig[i];
        return diff === 0;
      }
      throw new DOMException("verify does not support " + alg.name, "NotSupportedError");
    },

    async encrypt(algorithm, key, data) { return subtleCipher(true, algorithm, key, data, false); },
    async decrypt(algorithm, key, data) { return subtleCipher(false, algorithm, key, data, false); },

    async deriveBits(algorithm, baseKey, length) {
      const alg = normalizeAlgo(algorithm);
      if (isKdf(alg.name)) return kdfDeriveBits(alg, kdfParams(alg, "deriveBits"), baseKey, length, "deriveBits");
      if (alg.name === "ECDH") {
        requireBaseKey(baseKey, alg);
        const m = ecMaterial(baseKey);
        if (baseKey.type !== "private") throw new DOMException("Key is not of type 'private'", "InvalidAccessError");
        if (!baseKey.usages.includes("deriveBits") && !baseKey.usages.includes("deriveKey")) {
          throw new DOMException("This CryptoKey cannot be used for derivation", "InvalidAccessError");
        }
        const pub = alg.public;
        const pm = ecMaterial(pub);
        if (pub.type !== "public" || pm.curve !== m.curve) {
          throw new DOMException("The ECDH 'public' key is of the wrong type or curve", "InvalidAccessError");
        }
        const secret = runOp(() => _OPS.op_subtle_ecdh_derive_bits(m.curve, m.scalar, pm.sec1));
        if (length == null) return bufferOf(secret);
        if (length <= 0 || length % 8 !== 0 || length / 8 > secret.length) {
          throw new DOMException("Invalid ECDH deriveBits length", "OperationError");
        }
        return bufferOf(secret.subarray(0, length / 8));
      }
      keyBytes(baseKey);
      throw new DOMException("deriveBits does not support " + alg.name, "NotSupportedError");
    },

    async deriveKey(algorithm, baseKey, derivedKeyAlgorithm, extractable, keyUsages) {
      // Chrome normalizes the derivation params first, and names deriveKey in
      // the conversion error (deriveBits below would say deriveBits).
      const pAlg = normalizeAlgo(algorithm);
      const kdf = isKdf(pAlg.name) ? kdfParams(pAlg, "deriveKey") : null;
      const dAlg = normalizeAlgo(derivedKeyAlgorithm);
      let bits;
      if (dAlg.name === "HMAC") {
        const L = enforceIntMember(dAlg, "length", "deriveKey", "HmacImportParams", ULONG, true);
        // Chrome 157 raises this one as a TypeError here (an OperationError
        // in generateKey) — probed, pinned as is.
        if (L === 0) throw new TypeError("HMAC key length must not be zero");
        bits = L !== undefined ? L : hashBlockSize(normalizeHash(dAlg.hash)) * 8;
      } else if (dAlg.name === "AES-CTR" || dAlg.name === "AES-CBC" || dAlg.name === "AES-GCM" || dAlg.name === "AES-KW") {
        bits = enforceIntMember(dAlg, "length", "deriveKey", "AesDerivedKeyParams", USHORT);
        // Same faces as generateKey (Chrome 157 probed): 192-bit AES is gone.
        if (bits === 192) throw new DOMException("192-bit AES keys are not supported", "OperationError");
        if (bits !== 128 && bits !== 256) {
          throw new DOMException("AES key length must be 128 or 256 bits", "OperationError");
        }
      } else if (isKdf(dAlg.name)) {
        // No fixed length: the derivation is asked for null bits, which a KDF
        // base refuses and ECDH answers with the whole secret (Chrome 157).
        bits = null;
      } else if (Object.values(ALGO_CANON).includes(dAlg.name)) {
        throw new DOMException("Failed to execute 'deriveKey' on 'SubtleCrypto': " + dAlg.name + ": Unsupported operation: get key length", "NotSupportedError");
      } else {
        throw new DOMException("deriveKey does not support deriving " + dAlg.name, "NotSupportedError");
      }
      const derivedBits = kdf
        ? await kdfDeriveBits(pAlg, kdf, baseKey, bits, "deriveKey")
        : await this.deriveBits(algorithm, baseKey, bits);
      return this.importKey("raw", derivedBits, derivedKeyAlgorithm, extractable, keyUsages);
    },

    async wrapKey(format, key, wrappingKey, wrapAlgorithm) {
      if (keyMaterial.has(key) && isKdf(key.algorithm.name)) {
        throw new DOMException(key.algorithm.name + ": Unsupported operation: exportKey", "NotSupportedError");
      }
      const exported = await this.exportKey(format, key);
      const bytes = format === "jwk"
        ? new TextEncoder().encode(JSON.stringify(exported))
        : new Uint8Array(exported);
      return subtleCipher(true, wrapAlgorithm, wrappingKey, bytes, true);
    },

    async unwrapKey(format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) {
      checkUsageStrings("unwrapKey", keyUsages || []);
      const decrypted = await subtleCipher(false, unwrapAlgorithm, unwrappingKey, toBytes(wrappedKey), true);
      const keyData = format === "jwk"
        ? JSON.parse(new TextDecoder().decode(new Uint8Array(decrypted)))
        : decrypted;
      return this.importKey(format, keyData, unwrappedKeyAlgorithm, extractable, keyUsages);
    },
  };

  // encrypt/decrypt core, shared with wrapKey/unwrapKey (`viaWrap` opens the
  // AES-KW path — Chrome exposes AES-KW only through wrapping, and rejects a
  // direct encrypt/decrypt call with NotSupportedError).
  function subtleCipher(encrypt, algorithm, key, data, viaWrap) {
    const alg = normalizeAlgo(algorithm);
    const cipherOp = viaWrap ? (encrypt ? "wrapKey" : "unwrapKey") : (encrypt ? "encrypt" : "decrypt");
    const input = toBytes(data);
    if (alg.name === "RSA-OAEP") {
      const m = rsaMaterial(key);
      const needed = encrypt ? ["encrypt", "wrapKey"] : ["decrypt", "unwrapKey"];
      if (!key.usages.some((u) => needed.includes(u))) {
        throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
      }
      if ((encrypt && key.type !== "public") || (!encrypt && key.type !== "private")) {
        throw new DOMException("The key is not of the expected type", "InvalidAccessError");
      }
      const label = alg.label != null ? toBytes(alg.label) : new Uint8Array(0);
      const hash = (key.algorithm && key.algorithm.hash && key.algorithm.hash.name) || "SHA-256";
      const empty = new Uint8Array(0);
      // Off the JS thread (decrypt is a private-key op): the only arm of
      // this function that answers with a promise — every caller awaits.
      // (An async IIFE, not .then: a page may patch Promise.prototype.then.)
      return (async () => bufferOf(await offThread("rsa_oaep", hash, encrypt ? 1 : 0, 0, label, m.n, m.e,
        encrypt ? empty : m.d, encrypt ? empty : m.p, encrypt ? empty : m.q, input)))();
    }
    if (alg.name === "AES-KW") {
      if (!viaWrap) {
        throw new DOMException(
          "Failed to execute '" + (encrypt ? "encrypt" : "decrypt") + "' on 'SubtleCrypto': AES-KW: Unsupported operation: " + (encrypt ? "encrypt" : "decrypt"),
          "NotSupportedError");
      }
      const bytes = keyBytes(key);
      const needed = encrypt ? ["encrypt", "wrapKey"] : ["decrypt", "unwrapKey"];
      if (!key.usages.some((u) => needed.includes(u))) {
        throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
      }
      // Chrome 157's order: too short first (a wrap needs 16 bytes, an unwrap
      // the 24 of a wrapped 16), then the 8-byte multiple, both directions.
      if (input.length < (encrypt ? 16 : 24)) throw new DOMException("The provided data is too small", "OperationError");
      if (input.length % 8 !== 0) {
        throw new DOMException("The AES-KW input data length is invalid: not a multiple of 8 bytes", "DataError");
      }
      return bufferOf(runOp(() => _OPS.op_subtle_aes_kw(encrypt, bytes, input)));
    }
    const bytes = keyBytes(key);
    const needed = encrypt ? ["encrypt", "wrapKey"] : ["decrypt", "unwrapKey"];
    if (!key.usages.some((u) => needed.includes(u))) {
      throw new DOMException("key.usages does not permit this operation", "InvalidAccessError");
    }
    // Chrome 157 faces (probed): a missing BufferSource member is the
    // dictionary TypeError; wrong iv/counter sizes and failed decrypts are
    // OperationErrors (empty for a failed authentication or padding).
    const requireBuffer = (member, dictName) => {
      if (!(member in alg) || alg[member] === undefined) {
        throw new TypeError("Failed to execute '" + cipherOp + "' on 'SubtleCrypto': " + dictName + ": " + member + ": Missing required property");
      }
      return toBytes(alg[member]);
    };
    if (alg.name === "AES-GCM") {
      // Any non-empty IV, and every tag size Chrome takes (the full tag's
      // prefix) — the op is GCM by hand (crypto_secret.rs).
      const iv = requireBuffer("iv", "AeadParams");
      const aad = alg.additionalData != null ? toBytes(alg.additionalData) : new Uint8Array(0);
      const tl = enforceIntMember(alg, "tagLength", cipherOp, "AeadParams", OCTET, true);
      const tagLength = tl === undefined ? 128 : tl;
      if (![32, 64, 96, 104, 112, 120, 128].includes(tagLength)) {
        throw new DOMException("The tag length is invalid: Must be 32, 64, 96, 104, 112, 120, or 128 bits", "OperationError");
      }
      if (!encrypt && input.length < tagLength / 8) throw new DOMException("The provided data is too small", "OperationError");
      return bufferOf(runOp(() => _OPS.op_subtle_aes_gcm(encrypt, bytes, iv, aad, input, tagLength / 8)));
    }
    if (alg.name === "AES-CBC") {
      const iv = requireBuffer("iv", "AesCbcParams");
      if (iv.length !== 16) throw new DOMException("The \"iv\" has an unexpected length -- must be 16 bytes", "OperationError");
      return bufferOf(runOp(() => _OPS.op_subtle_aes_cbc(encrypt, bytes, iv, input)));
    }
    if (alg.name === "AES-CTR") {
      const counter = requireBuffer("counter", "AesCtrParams");
      if (counter.length !== 16) throw new DOMException("The \"counter\" has an unexpected length -- must be 16 bytes", "OperationError");
      const length = enforceIntMember(alg, "length", cipherOp, "AesCtrParams", OCTET);
      if (length < 1 || length > 128) {
        throw new DOMException("The \"length\" member must be >= 1 and <= 128", "OperationError");
      }
      try {
        return bufferOf(_OPS.op_subtle_aes_ctr(bytes, counter, length, input));
      } catch (e) {
        if (e && e.message === "AES-CTR counter overflow") throw new DOMException("The input is too large for the counter length.", "DataError");
        throw new DOMException(e && e.message !== undefined ? String(e.message) : String(e), "OperationError");
      }
    }
    throw new DOMException((encrypt ? "encrypt" : "decrypt") + " does not support " + alg.name, "NotSupportedError");
  }

  globalThis.CryptoKey = CryptoKey;
  globalThis.SubtleCrypto = function SubtleCrypto() { throw new TypeError("Failed to construct '" + ((new.target && new.target.name) || "?") + "': Illegal constructor"); };
  Object.setPrototypeOf(subtle, globalThis.SubtleCrypto.prototype);
  globalThis.crypto.subtle = subtle;
  globalThis.__diting_subtle_hooks = { makeKey, keyBytes, toBytes, bufferOf, b64urlToBytes, bytesToB64url, runOp };

  // A CryptoKey cloned via structuredClone or postMessage is a different
  // object, so the keyMaterial WeakMap lookup misses and crypto.subtle throws
  // "Argument is not a valid CryptoKey". Rebuild the (cloned) key through
  // makeKey so it re-enters the WeakMap and stays usable. `seen` is the clone
  // memo _structuredClone hands every hook; populate it so one key reached
  // twice in a graph clones to one shared object (upstream 8698afc).
  globalThis.__diting_clone_hooks = globalThis.__diting_clone_hooks || {};
  globalThis.__diting_clone_hooks["CryptoKey"] = function (src, seen) {
    if (seen && seen.has(src)) return seen.get(src);
    const copy = makeKey(src.type, src.extractable, src.algorithm, src.usages, keyBytes(src));
    if (seen) seen.set(src, copy);
    return copy;
  };
}
// @@EXPORTS-BEGIN generated by tools/webplatform/analyze/analyze.mjs --gen-exports - do not edit
{
  // Only names the window does not already carry are defined (hidden, non-enumerable) - a Chrome-visible global keeps its value.
  const own = (n) => Object.prototype.hasOwnProperty.call(globalThis, n);
  const hide = (globalThis.__diting_hide_list || (globalThis.__diting_hide_list = []));
  const d = (n, v) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { value: v, writable: true, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  const a = (n, get, set) => { if (own(n)) return; try { Object.defineProperty(globalThis, n, { get, set, enumerable: false, configurable: true }); hide.push(n); } catch (e) {} };
  d('b64urlToBytes', b64urlToBytes); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('bytesToB64url', bytesToB64url); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('checkKeyUsages', checkKeyUsages); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('checkUsageStrings', checkUsageStrings); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('ecMaterial', ecMaterial); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('ecUsagesFor', ecUsagesFor); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('ecdsaUnflat', ecdsaUnflat); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('jwkMemberBytes', jwkMemberBytes); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('jwkRequire', jwkRequire); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('jwkValidateCommon', jwkValidateCommon); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('keyAlgorithmFor', keyAlgorithmFor); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('keyBytes', keyBytes); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('makeKey', makeKey); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('normalizeAlgo', normalizeAlgo); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('normalizeHash', normalizeHash); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('requireEcdsaUsages', requireEcdsaUsages); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('requireHashParam', requireHashParam); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('rsaFace', rsaFace); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('rsaGenerateKey', rsaGenerateKey); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('rsaImportKey', rsaImportKey); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('rsaMaterial', rsaMaterial); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('rsaUnflat', rsaUnflat); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('runOp', runOp); // block-level function (Annex B hoisting): a var binding of the wrapper function
  d('subtleCipher', subtleCipher); // block-level function (Annex B hoisting): a var binding of the wrapper function
}
// @@EXPORTS-END
}).call(globalThis); // @@SCOPE-END
