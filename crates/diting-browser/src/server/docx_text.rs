//! DOCX text salvage — the Office cousin of `pdf_text`. A docx is a zip
//! whose `word/document.xml` carries the body as `<w:t>` runs inside
//! `<w:p>` paragraphs; this walks local file headers without trusting the
//! central directory, inflates the one entry worth reading, and strips the
//! XML back to text. Kept beside the PDF extractor (both are binary
//! document salvage behind the same fetch route) but in its own family
//! file: no shared state, only the byte-scan helpers.

use std::io::Read;

use super::pdf_text::{find, normalize};

/// Salvage DOCX text. Local-file-header scan (no central-directory trust),
/// raw-inflate the one entry we want, strip the XML back to text. `None`
/// when the entry or the text is missing.
pub fn docx_to_text(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return None;
    }
    let mut pos = 0;
    while let Some(at) = find(bytes, pos, b"PK\x03\x04") {
        if at + 30 > bytes.len() {
            break;
        }
        let h = &bytes[at..];
        let method = u16::from_le_bytes([h[8], h[9]]);
        let csize = u32::from_le_bytes([h[18], h[19], h[20], h[21]]) as usize;
        let namelen = u16::from_le_bytes([h[26], h[27]]) as usize;
        let extralen = u16::from_le_bytes([h[28], h[29]]) as usize;
        let name_end = at + 30 + namelen;
        if name_end > bytes.len() {
            break;
        }
        let name = &bytes[at + 30..name_end];
        let data_start = name_end + extralen;
        pos = data_start.max(at + 4);
        if name.ends_with(b"word/document.xml") || name.ends_with(b"word/document2.xml") {
            let raw = &bytes[data_start.min(bytes.len())..];
            let xml: Vec<u8> = if csize > 0 && csize <= raw.len() {
                match method {
                    0 => raw[..csize].to_vec(),
                    _ => match raw_inflate(&raw[..csize]) {
                        Some(v) => v,
                        None => continue,
                    },
                }
            } else {
                // Sizes zeroed (streamed zip): try to the next entry marker.
                let end = find(raw, 0, b"PK\x03\x04")
                    .or_else(|| find(raw, 0, b"PK\x01\x02"))
                    .unwrap_or(raw.len());
                match method {
                    0 => raw[..end].to_vec(),
                    _ => match raw_inflate(&raw[..end]) {
                        Some(v) => v,
                        None => continue,
                    },
                }
            };
            let text = docx_xml_text(&xml);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Raw DEFLATE (zip's method 8 — no zlib wrapper).
fn raw_inflate(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(raw)
        .read_to_end(&mut out)
        .ok()?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// `<w:t>` runs joined, `</w:p>` → newline, `&…;` unescaped. Compact recall
/// forgives layout; only characters matter.
fn docx_xml_text(xml: &[u8]) -> String {
    let s = String::from_utf8_lossy(xml);
    let mut out = String::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<w:t") {
            let open_end = match find(bytes, i, b">") {
                Some(p) => p + 1,
                None => break,
            };
            let close = match find(bytes, open_end, b"</w:t>") {
                Some(p) => p,
                None => break,
            };
            let run = &s[open_end..close];
            out.push_str(&xml_unescape(run));
            i = close + 6;
        } else if bytes[i..].starts_with(b"</w:p>") {
            out.push('\n');
            i += 5;
        } else if bytes[i..].starts_with(b"<w:tab") {
            out.push('\t');
            i += 6;
        } else {
            i += 1;
        }
    }
    normalize(&out)
}

fn xml_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
