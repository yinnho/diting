//! Golden-byte test for the TLS 1.3 certificate-compression codecs (#33).
//!
//! `fixtures/cert.pem` is a real root-ca certificate block; `fixtures/cert.brotli8.bin`
//! is that exact text compressed with brotli 8 (the encoder we shipped before the
//! brotli 9 unification). Decode semantics must stay byte-stable across brotli
//! generations — a server that compressed its certificate with any brotli version
//! has to decompress to the identical bytes here. Encoder output itself may drift
//! between versions, so only the decode side is pinned, plus a 9→9 roundtrip.
//!
//! On the next brotli major bump, regenerate the golden blob with
//! `cargo test -p diting_profiles --test compress_golden -- --ignored regen`.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn brotli_decompress(input: &[u8]) -> Vec<u8> {
    let mut reader = brotli::Decompressor::new(input, 4096);
    let mut out = Vec::new();
    reader.read_to_end(&mut out).expect("brotli decode");
    out
}

#[test]
fn brotli8_golden_blob_decodes_byte_identical() {
    let plain = fs::read(fixture("cert.pem")).expect("cert.pem fixture");
    let blob = fs::read(fixture("cert.brotli8.bin")).expect("cert.brotli8.bin fixture");
    let decoded = brotli_decompress(&blob);
    assert_eq!(decoded, plain, "brotli decode of the frozen brotli-8 blob drifted");
}

#[test]
fn brotli_roundtrip_on_cert_shaped_input() {
    let plain = fs::read(fixture("cert.pem")).expect("cert.pem fixture");
    let mut writer = brotli::CompressorWriter::new(Vec::new(), plain.len(), 11, 32);
    writer.write_all(&plain).expect("compress");
    let blob = writer.into_inner();
    assert!(blob.len() < plain.len(), "brotli should shrink certificate text");
    assert_eq!(brotli_decompress(&blob), plain);
}

#[test]
#[ignore = "regenerates the golden fixture with the CURRENT brotli encoder"]
fn regen() {
    let plain = fs::read(fixture("cert.pem")).expect("cert.pem fixture");
    let mut writer = brotli::CompressorWriter::new(Vec::new(), plain.len(), 11, 32);
    writer.write_all(&plain).expect("compress");
    fs::write(fixture("cert.brotli8.bin"), writer.into_inner()).expect("write golden");
}
