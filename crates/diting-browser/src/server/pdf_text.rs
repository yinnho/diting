//! Salvage-grade PDF text extraction.
//!
//! Parses just enough of the file to walk the page tree and replay the
//! text-showing operators (`Tj`/`TJ`/`'`/`"`), decoding string bytes through
//! each font's ToUnicode CMap (cp1252 fallback when a font ships none, with
//! `/Differences` glyph names filling the codes the CMap leaves blank).
//! TeX-toolchain PDFs position words with moves instead of space glyphs, so
//! the interpreter also tracks the text line origin (`Tm`/`Td`) and turns
//! inter-word moves into spaces, wrapped lines into paragraph joins —
//! without that, "Attention Is All You Need" extracts as one camel hump. Not
//! a renderer: stream order is the reading order, which for pdftex/InDesign
//! output matches the common extractors closely enough for n-gram recall
//! scoring.
//!
//! Why salvage and not xref-driven: benchmark PDFs are PDF 1.5 hybrids —
//! cross-reference streams plus object streams (`/ObjStm`) holding a third of
//! the objects compressed. Linear-scanning `N G obj … endobj` covers the
//! uncompressed half; expanding the object streams recovers the rest, and no
//! xref table has to be trusted. `endstream` inside compressed data is warded
//! off by honouring a numeric `/Length` first and zlib-validating the slice.

use std::collections::HashMap;
use std::io::Read as _;

/// Extract text from a PDF, or `None` when nothing parseable came out.
pub fn pdf_to_text(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"%PDF") {
        return None;
    }
    let mut objects = HashMap::new();
    scan_objects(bytes, &mut objects);
    if objects.is_empty() {
        return None;
    }
    expand_object_streams(&mut objects);

    let mut pages = Vec::new();
    let catalog = objects.values().find_map(|o| {
        o.dict
            .as_ref()
            .filter(|d| d.name("Type") == Some("Catalog"))
    });
    match catalog.and_then(|c| c.ref_of("Pages")) {
        Some((root, _)) => collect_pages(root, &objects, None, &mut pages, 0),
        None => {
            // Truncated headless PDF (benchmark serves one: no /Catalog, no
            // xref, cut mid-file). Every surviving /Type /Page in object
            // order — for TeX-toolchain output that is page order.
            let mut nums: Vec<u32> = objects
                .iter()
                .filter(|(_, o)| {
                    o.dict
                        .as_ref()
                        .is_some_and(|d| d.name("Type") == Some("Page"))
                })
                .map(|(n, _)| *n)
                .collect();
            nums.sort_unstable();
            for n in nums {
                collect_pages(n, &objects, None, &mut pages, 0);
            }
        }
    }
    if pages.is_empty() {
        return None;
    }

    // Font decode tables: resource name (e.g. "F1") is page-scoped, but the
    // underlying font objects are file-unique — decode by object number so
    // every page shares the work.
    let mut fonts: HashMap<u32, FontMap> = HashMap::new();
    let mut out = String::new();
    let debug = std::env::var("DITING_PDF_DEBUG").is_ok();
    if debug {
        eprintln!("pdf_text: {} objects, {} pages", objects.len(), pages.len());
    }
    for page in &pages {
        let mut content = Vec::new();
        for c in &page.contents {
            content.extend_from_slice(c);
        }
        if content.is_empty() {
            continue;
        }
        // Resolve ToUnicode tables on first use per font object.
        for font_obj in page.fonts.values() {
            font_map_for(*font_obj, &objects, &mut fonts);
            if debug {
                let m = fonts.get(font_obj).unwrap();
                let d = objects.get(font_obj).and_then(|o| o.dict.as_ref());
                let w = match d.and_then(|d| d.get("Widths")) {
                    Some(Value::Array(_)) => "inline".to_string(),
                    Some(Value::Ref(n, _)) => match objects.get(n).and_then(|o| o.value.as_ref()) {
                        Some(Value::Array(_)) => format!("ref{n}:array"),
                        other => format!("ref{n}:{other:?}"),
                    },
                    other => format!("{other:?}"),
                };
                eprintln!(
                    "  font {font_obj}: {} entries, code_len {}, widths {}, base={:?}, Widths={}",
                    m.map.len(),
                    m.code_len,
                    m.widths.len(),
                    d.and_then(|d| d.name("BaseFont")),
                    w
                );
            }
        }
        interpret_content(&content, &page.fonts, &mut fonts, &mut out);
        out.push_str("\n\n");
    }
    let text = normalize(&out);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// ---------------------------------------------------------------- objects

#[derive(Default, Debug)]
struct PdfObj {
    dict: Option<PdfDict>,
    /// Non-dict object body — `/Widths 17 0 R` points at a bare `[ … ]`.
    value: Option<Value>,
    /// Stream bytes, already decompressed when the filter was FlateDecode.
    stream: Option<Vec<u8>>,
}

#[derive(Default, Debug, Clone)]
struct PdfDict {
    entries: HashMap<String, Value>,
}

impl PdfDict {
    fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key)
    }
    fn name(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Name(n)) => Some(n.as_str()),
            _ => None,
        }
    }
    fn num(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::Num(n)) => Some(*n),
            _ => None,
        }
    }
    fn ref_of(&self, key: &str) -> Option<(u32, u32)> {
        match self.get(key) {
            Some(Value::Ref(n, g)) => Some((*n, *g)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum Value {
    Name(String),
    Str(Vec<u8>),
    Num(f64),
    Ref(u32, u32),
    Bool,
    Null,
    Array(Vec<Value>),
    Dict(PdfDict),
}

/// Linear scan for `N G obj … endobj`, storing each object's dictionary and
/// (decompressed) stream.
fn scan_objects(bytes: &[u8], objects: &mut HashMap<u32, PdfObj>) {
    let mut pos = 0;
    while let Some(at) = find(bytes, pos, b"obj") {
        // `obj` must follow `N G ` — walk back over two integers. The one
        // hugging `obj` is the generation; the one before it is the number.
        let (gen, _after_gen) = match trailing_number_before(bytes, at) {
            Some(x) => x,
            None => {
                pos = at + 3;
                continue;
            }
        };
        let (num, after_num) = match trailing_number_before(bytes, _after_gen) {
            Some(x) => x,
            None => {
                pos = at + 3;
                continue;
            }
        };
        let _ = after_num;
        let mut lex = Lexer {
            b: bytes,
            pos: at + 3,
        };
        lex.skip_ws();
        let mut value = None;
        let dict = match lex.parse_value() {
            Some(Value::Dict(d)) => Some(d),
            Some(v) => {
                value = Some(v);
                None
            }
            None => None,
        };
        let mut obj = PdfObj {
            dict,
            value,
            ..Default::default()
        };
        lex.skip_ws();
        if lex.starts_with(b"stream") {
            if let Some(stream) = slice_stream(&lex, bytes, obj.dict.as_ref()) {
                obj.stream = Some(stream);
            }
        }
        let num_key = num as u32;
        // Linearized files repeat first-page objects (content stream in the
        // head section, page dict in the main body). First occurrence wins,
        // but a dict that arrives late merges into a stream-bearing entry —
        // that dict is the page object that references the stream.
        match objects.entry(num_key) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(obj);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                if o.get().stream.is_some()
                    && o.get().dict.is_none()
                    && obj.stream.is_none()
                    && obj.dict.is_some()
                {
                    o.get_mut().dict = obj.dict;
                }
            }
        }
        // Skip past this object; endobj hunt is unreliable inside binary, so
        // jump over the stream when present.
        pos = find(bytes, at + 3, b"endobj")
            .map(|e| e + 6)
            .unwrap_or(bytes.len());
        let _ = gen;
    }
}

/// The integer that ends right before `at` (ignoring one space). Returns
/// (value, start-of-that-integer).
fn trailing_number_before(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let mut i = at;
    while i > 0 && (bytes[i - 1] == b' ') {
        i -= 1;
    }
    let end = i;
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i == end {
        return None;
    }
    let n: usize = std::str::from_utf8(&bytes[i..end]).ok()?.parse().ok()?;
    Some((n, i))
}

fn slice_stream(lex: &Lexer, bytes: &[u8], dict: Option<&PdfDict>) -> Option<Vec<u8>> {
    let mut start = lex.pos + 6; // past `stream`
    while start < bytes.len() && (bytes[start] == b'\r' || bytes[start] == b'\n') {
        start += 1;
    }
    let raw: &[u8] = match dict.and_then(|d| d.num("Length")) {
        Some(len) if len >= 0.0 => {
            let end = (start + len as usize).min(bytes.len());
            &bytes[start..end]
        }
        _ => {
            let end = find(bytes, start, b"endstream")?;
            let mut e = end;
            // strip the EOL that precedes endstream
            while e > start && (bytes[e - 1] == b'\n' || bytes[e - 1] == b'\r') {
                e -= 1;
            }
            &bytes[start..e]
        }
    };
    decode_stream(raw, dict)
}

fn decode_stream(raw: &[u8], dict: Option<&PdfDict>) -> Option<Vec<u8>> {
    let filter = dict.and_then(|d| d.name("Filter")).map(|s| s.to_string());
    match filter.as_deref() {
        None | Some("Identity") => Some(raw.to_vec()),
        // Best-effort: a stream whose declared /Length overshot into the
        // next object still decodes up to the corruption — keep that prefix
        // (linear cost) instead of retrying shorter slices (quadratic).
        Some("FlateDecode") => inflate_best_effort(raw),
        // DCTDecode/CCITTFaxDecode/JBIG2 are images — no text to lose.
        Some(_) => None,
    }
}

/// Inflate everything the decoder manages before any corruption; `None`
/// only when not a byte came out.
fn inflate_best_effort(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut z = flate2::read::ZlibDecoder::new(raw);
    let mut buf = [0u8; 16384];
    loop {
        match z.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Expand `/Type /ObjStm` streams into the object table. Objects inside an
/// object stream may not themselves be streams (spec), so dicts land whole.
fn expand_object_streams(objects: &mut HashMap<u32, PdfObj>) {
    let streams: Vec<(u32, Vec<u8>, usize, usize)> = objects
        .iter()
        .filter(|(_, o)| {
            o.dict
                .as_ref()
                .is_some_and(|d| d.name("Type") == Some("ObjStm"))
        })
        .filter_map(|(num, o)| {
            let d = o.dict.as_ref()?;
            let n = d.num("N")? as usize;
            let first = d.num("First")? as usize;
            let s = o.stream.as_ref()?;
            Some((*num, s.clone(), n, first))
        })
        .collect();
    for (_num, body, count, first) in streams {
        let mut lex = Lexer { b: &body, pos: 0 };
        let mut placements = Vec::with_capacity(count);
        for _ in 0..count {
            let Some(Value::Num(onum)) = lex.parse_value() else {
                break;
            };
            let Some(Value::Num(off)) = lex.parse_value() else {
                break;
            };
            placements.push((onum as u32, off as usize));
        }
        for (onum, off) in placements {
            let mut sub = Lexer {
                b: &body,
                pos: first + off,
            };
            if let Some(v) = sub.parse_value() {
                let dict = match v {
                    Value::Dict(d) => Some(d),
                    _ => None,
                };
                // An ObjStm placement is structural truth; a linear-scan
                // false positive (compressed bytes happening to spell
                // "N 0 obj") must not shadow it. Fill a dict-less impostor.
                match objects.entry(onum) {
                    std::collections::hash_map::Entry::Vacant(vac) => {
                        vac.insert(PdfObj {
                            dict,
                            ..Default::default()
                        });
                    }
                    std::collections::hash_map::Entry::Occupied(mut occ) => {
                        if occ.get().dict.is_none() {
                            occ.get_mut().dict = dict;
                        }
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------ lexer

struct Lexer<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if c == b'%' {
                while self.pos < self.b.len() && self.b[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else if c.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, kw: &[u8]) -> bool {
        self.b[self.pos..].starts_with(kw)
    }

    fn parse_value(&mut self) -> Option<Value> {
        self.skip_ws();
        let c = *self.b.get(self.pos)?;
        match c {
            b'<' if self.b.get(self.pos + 1) == Some(&b'<') => self.parse_dict(),
            b'<' => self.parse_hex_string(),
            b'(' => self.parse_literal_string(),
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.b.get(self.pos) {
                        Some(b']') => {
                            self.pos += 1;
                            return Some(Value::Array(items));
                        }
                        Some(_) => items.push(self.parse_value()?),
                        None => return Some(Value::Array(items)),
                    }
                }
            }
            b'/' => self.parse_name(),
            b't' | b'f' | b'n' => self.parse_keyword(),
            b'+' | b'-' | b'.' | b'0'..=b'9' => self.parse_number_or_ref(),
            _ => {
                self.pos += 1;
                None
            }
        }
    }

    fn parse_dict(&mut self) -> Option<Value> {
        self.pos += 2; // <<
        let mut d = PdfDict::default();
        loop {
            self.skip_ws();
            match self.b.get(self.pos) {
                Some(b'>') if self.b.get(self.pos + 1) == Some(&b'>') => {
                    self.pos += 2;
                    return Some(Value::Dict(d));
                }
                Some(b'/') => {
                    let key = match self.parse_name()? {
                        Value::Name(n) => n,
                        _ => continue,
                    };
                    // A value may be absent (malformed); treat as Null.
                    let before = self.pos;
                    let v = self.parse_value().unwrap_or(Value::Null);
                    if matches!(v, Value::Null) && before == self.pos {
                        self.pos += 1;
                    }
                    d.entries.insert(key, v);
                }
                Some(_) => {
                    self.pos += 1;
                }
                None => return Some(Value::Dict(d)),
            }
        }
    }

    fn parse_name(&mut self) -> Option<Value> {
        self.pos += 1; // /
        let start = self.pos;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            if c.is_ascii_whitespace()
                || matches!(c, b'/' | b'<' | b'>' | b'[' | b']' | b'(' | b')')
            {
                break;
            }
            self.pos += 1;
        }
        if start == self.pos {
            return None;
        }
        Some(Value::Name(
            String::from_utf8_lossy(&self.b[start..self.pos]).into_owned(),
        ))
    }

    fn parse_hex_string(&mut self) -> Option<Value> {
        self.pos += 1; // <
        let start = self.pos;
        while self.pos < self.b.len() && self.b[self.pos] != b'>' {
            self.pos += 1;
        }
        let hex = &self.b[start..self.pos];
        self.pos += 1; // >
        let mut out = Vec::with_capacity(hex.len() / 2 + 1);
        let mut nib: Option<u8> = None;
        for &h in hex {
            let d = (h as char).to_digit(16);
            match (nib, d) {
                (Some(hi), Some(lo)) => {
                    out.push(hi * 16 + lo as u8);
                    nib = None;
                }
                (None, Some(lo)) => nib = Some(lo as u8),
                _ => {}
            }
        }
        if let Some(hi) = nib {
            out.push(hi * 16);
        }
        Some(Value::Str(out))
    }

    fn parse_literal_string(&mut self) -> Option<Value> {
        self.pos += 1; // (
        let mut out = Vec::new();
        let mut depth = 1usize;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            match c {
                b'\\' => {
                    self.pos += 1;
                    let e = *self.b.get(self.pos)?;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' | b'f' => {}
                        b'0'..=b'7' => {
                            // up to three octal digits
                            let mut v = (e - b'0') as u32;
                            let mut k = 1;
                            while k < 3 && self.pos + 1 < self.b.len() {
                                let d = self.b[self.pos + 1];
                                if (b'0'..=b'7').contains(&d) {
                                    self.pos += 1;
                                    v = v * 8 + (d - b'0') as u32;
                                    k += 1;
                                } else {
                                    break;
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                    self.pos += 1;
                }
                b'(' => {
                    depth += 1;
                    out.push(c);
                    self.pos += 1;
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos += 1;
                        return Some(Value::Str(out));
                    }
                    out.push(c);
                    self.pos += 1;
                }
                _ => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
        Some(Value::Str(out))
    }

    fn parse_keyword(&mut self) -> Option<Value> {
        let start = self.pos;
        while self.pos < self.b.len() && self.b[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        match &self.b[start..self.pos] {
            b"true" => Some(Value::Bool),
            b"false" => Some(Value::Bool),
            b"null" => Some(Value::Null),
            _ => {
                self.pos = start;
                None
            }
        }
    }

    fn parse_number_or_ref(&mut self) -> Option<Value> {
        let start = self.pos;
        if matches!(self.b.get(self.pos), Some(b'+') | Some(b'-')) {
            self.pos += 1;
        }
        while self
            .b
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.pos]).ok()?;
        let n: f64 = s.parse().ok()?;
        // `N G R` reference?
        let save = self.pos;
        self.skip_ws();
        if self.b[self.pos..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
            > 0
        {
            let gstart = self.pos;
            while self.pos < self.b.len() && self.b[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
            let g: f64 = std::str::from_utf8(&self.b[gstart..self.pos])
                .ok()?
                .parse()
                .ok()?;
            self.skip_ws();
            if self.b.get(self.pos) == Some(&b'R')
                && !self
                    .b
                    .get(self.pos + 1)
                    .is_some_and(|c| c.is_ascii_alphabetic())
            {
                self.pos += 1;
                return Some(Value::Ref(n as u32, g as u32));
            }
        }
        self.pos = save;
        Some(Value::Num(n))
    }
}

// Byte-scan helper shared with the DOCX salvage family (docx_text.rs).
pub(super) fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

// ------------------------------------------------------------------ pages

struct PageInfo {
    contents: Vec<Vec<u8>>,
    /// resource name → font object number
    fonts: HashMap<String, u32>,
}

fn collect_pages(
    node: u32,
    objects: &HashMap<u32, PdfObj>,
    inherited: Option<&PdfDict>,
    out: &mut Vec<PageInfo>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    let Some(obj) = objects.get(&node) else {
        return;
    };
    let Some(dict) = obj.dict.as_ref() else {
        return;
    };
    let resources = dict
        .get("Resources")
        .and_then(|v| resolve_dict_obj(v, objects))
        .or_else(|| inherited.cloned());
    match dict.name("Type") {
        Some("Page") => {
            let mut contents = Vec::new();
            match dict.get("Contents") {
                Some(Value::Ref(n, _)) => {
                    if let Some(s) = objects.get(n).and_then(|o| o.stream.clone()) {
                        contents.push(s);
                    }
                }
                Some(Value::Array(items)) => {
                    for it in items {
                        if let Value::Ref(n, _) = it {
                            if let Some(s) = objects.get(n).and_then(|o| o.stream.clone()) {
                                contents.push(s);
                            }
                        }
                    }
                }
                _ => {}
            }
            let mut fonts = HashMap::new();
            if let Some(res) = &resources {
                let font_dict = match res.get("Font") {
                    Some(v) => resolve_dict_obj(v, objects),
                    None => None,
                };
                if let Some(fd) = font_dict {
                    for (name, v) in &fd.entries {
                        if let Value::Ref(n, _) = v {
                            fonts.insert(name.clone(), *n);
                        }
                    }
                }
            }
            out.push(PageInfo { contents, fonts });
        }
        Some("Pages") => {
            if let Some(Value::Array(kids)) = dict.get("Kids") {
                for kid in kids {
                    if let Value::Ref(n, _) = kid {
                        collect_pages(*n, objects, resources.as_ref(), out, depth + 1);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Resolve a dict value, following one indirect-reference hop — `/Resources
/// 5835 0 R` and `/Font 5836 0 R` are usually indirect, and the referenced
/// object may itself have come out of an object stream.
fn resolve_dict_obj(v: &Value, objects: &HashMap<u32, PdfObj>) -> Option<PdfDict> {
    match v {
        Value::Dict(d) => Some(d.clone()),
        Value::Ref(n, _) => objects.get(n).and_then(|o| o.dict.clone()),
        _ => None,
    }
}

// ------------------------------------------------------------------- fonts

struct FontMap {
    code_len: usize,
    map: HashMap<u32, String>,
    /// `/Widths` per-code advances in em units (glyph-space thousandths /
    /// 1000) — when present they replace the class estimate, whose error
    /// over a word can swallow a real word gap ("languages.The").
    widths: HashMap<u32, f64>,
    /// `/MissingWidth`-style fallback for codes the Widths array skips.
    default_width: f64,
    /// `/Encoding /MacRomanEncoding` — macOS Quartz output. The high half
    /// differs from WinAnsi; most famously 0xDE/0xDF are the fi/fl
    /// ligatures, which a cp1252 read turns into Þ/þ ("PDF Þle").
    mac_roman: bool,
}

impl FontMap {
    fn decode(&self, bytes: &[u8]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let take = self.code_len.min(bytes.len() - i).max(1);
            let mut code: u32 = 0;
            for k in 0..take {
                code = (code << 8) | bytes[i + k] as u32;
            }
            if take == self.code_len {
                if let Some(s) = self.map.get(&code) {
                    out.push_str(s);
                    i += take;
                    continue;
                }
            }
            if self.mac_roman {
                let s = mac_roman(bytes[i]);
                if !s.is_empty() {
                    out.push_str(s);
                    i += 1;
                    continue;
                }
            }
            // Unknown code (or short tail): cp1252 per byte.
            out.push(cp1252(bytes[i]));
            i += 1;
        }
        out
    }
    /// Exact advance of a show-string's bytes in em units: `/Widths` where
    /// the font declares them, the class estimate where it does not (the
    /// `plain` cp1252 fallback has no width table at all).
    fn em_width(&self, bytes: &[u8]) -> f64 {
        if self.widths.is_empty() {
            let s = self.decode(bytes);
            return est_em_width(&s);
        }
        let mut out = 0.0;
        let mut i = 0;
        while i < bytes.len() {
            let take = self.code_len.min(bytes.len() - i).max(1);
            let mut code: u32 = 0;
            for k in 0..take {
                code = (code << 8) | bytes[i + k] as u32;
            }
            out += match self.widths.get(&code) {
                Some(w) => *w,
                None if self.code_len > 1 => self.default_width,
                None => est_em_width(&cp1252(bytes[i]).to_string()),
            };
            i += take;
        }
        out
    }
}

/// MacRoman's high half — only the codes that actually surface in
/// benchmark text are mapped; everything else falls back to cp1252.
fn mac_roman(b: u8) -> &'static str {
    match b {
        0xDE => "fi",
        0xDF => "fl",
        0x80 => "Ä",
        0x81 => "Å",
        0x82 => "Ç",
        0x83 => "É",
        0x84 => "Ñ",
        0x85 => "Ö",
        0x86 => "Ü",
        0x87 => "á",
        0x88 => "à",
        0x89 => "â",
        0x8A => "ä",
        0x8B => "ã",
        0x8C => "å",
        0x8D => "ç",
        0x8E => "é",
        0x8F => "è",
        0x90 => "ê",
        0x91 => "ë",
        0x92 => "í",
        0x93 => "ì",
        0x94 => "î",
        0x95 => "ï",
        0x96 => "ó",
        0x97 => "ò",
        0x98 => "ô",
        0x99 => "ö",
        0x9A => "õ",
        0x9B => "û",
        0x9C => "ù",
        0x9D => "ÿ",
        0x9E => "Ô",
        0x9F => "Ò",
        0xA7 => "Á",
        0xAB => "Ø",
        0xAC => "œ",
        0xAD => "Œ",
        _ => "",
    }
}

/// Parse a ToUnicode CMap stream into a byte/CID → text table.
fn parse_cmap(stream: &[u8]) -> FontMap {
    let mut code_len = 1usize;
    let mut map: HashMap<u32, String> = HashMap::new();
    let text = String::from_utf8_lossy(stream);
    // codespacerange fixes the code width (Identity-H → 2 bytes).
    if let (Some(a), Some(b)) = (
        find_str(&text, "begincodespacerange"),
        text[b_end(&text, "begincodespacerange")..]
            .find("endcodespacerange")
            .map(|p| p + b_end(&text, "begincodespacerange")),
    ) {
        let seg = &text[a..b];
        if let (Some(h1), Some(h2)) = (seg.find('<'), seg.find('>')) {
            let _ = h2;
            let digits = seg[h1 + 1..].chars().take_while(|c| *c != '>').count();
            if digits % 4 == 0 {
                code_len = digits / 2;
            }
        }
    }
    let _ = &text;
    for section in ["bfchar", "bfrange"] {
        let open = format!("begin{section}");
        let close = format!("end{section}");
        let mut from = 0usize;
        while let Some(rel) = text[from..].find(&open) {
            let start = from + rel + open.len();
            let end = match text[start..].find(&close) {
                Some(r) => start + r,
                None => break,
            };
            let seg = &text[start..end];
            if section == "bfchar" {
                let mut it = hex_strings(seg);
                while let (Some(src), Some(dst)) = (it.next(), it.next()) {
                    if let Some(code) = hex_to_u32(&src) {
                        map.insert(code, utf16_hex_to_string(&dst));
                    }
                }
            } else {
                let mut it = hex_strings(seg);
                while let (Some(lo), Some(hi), Some(third)) = (it.next(), it.next(), it.next()) {
                    let Some(lo) = hex_to_u32(&lo) else { continue };
                    let Some(hi) = hex_to_u32(&hi) else {
                        continue;
                    };
                    if third.starts_with('[') {
                        // array form: one destination per code
                        let items: Vec<&str> = third
                            .trim_start_matches('[')
                            .trim_end_matches(']')
                            .split_whitespace()
                            .collect();
                        for (k, item) in items.iter().enumerate() {
                            if let Some(c) = lo.checked_add(k as u32) {
                                if c > hi {
                                    break;
                                }
                                map.insert(c, utf16_hex_to_string(item));
                            }
                        }
                    } else if let Some(base) = hex_to_u32(&third) {
                        for c in lo..=hi.min(lo.saturating_add(65535)) {
                            map.insert(c, char_from_u32(base + (c - lo)));
                        }
                    }
                }
            }
            from = end + close.len();
        }
    }
    FontMap {
        code_len,
        map,
        widths: HashMap::new(),
        default_width: 0.5,
        mac_roman: false,
    }
}

fn b_end(text: &str, kw: &str) -> usize {
    text.find(kw).map(|p| p + kw.len()).unwrap_or(0)
}

fn find_str(text: &str, kw: &str) -> Option<usize> {
    text.find(kw)
}

/// Yield `<...>` hex strings in order.
fn hex_strings(seg: &str) -> impl Iterator<Item = String> + '_ {
    let mut rest = seg;
    std::iter::from_fn(move || {
        let start = rest.find('<')? + 1;
        let end = rest[start..].find('>')? + start;
        let s = rest[start..end].to_string();
        rest = &rest[end + 1..];
        Some(s)
    })
}

fn hex_to_u32(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim(), 16).ok()
}

/// UTF-16BE hex → String, joining surrogate pairs.
fn utf16_hex_to_string(hex: &str) -> String {
    let clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let units: Vec<u16> = (0..clean.len() / 4)
        .filter_map(|i| u16::from_str_radix(&clean[i * 4..i * 4 + 4], 16).ok())
        .collect();
    char::decode_utf16(units)
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

fn char_from_u32(v: u32) -> String {
    match char::from_u32(v) {
        Some(c) => c.to_string(),
        None => '\u{FFFD}'.to_string(),
    }
}

/// cp1252, with the 0x80–0x9F block the reference extractors also map.
fn cp1252(b: u8) -> char {
    match b {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        _ => b as char,
    }
}

// ------------------------------------------------------- content playback

// ---------------------------------------------------------- text layout

/// Rough per-character advance widths (em units — a Times/CM blend). No font
/// ships a width table we trust at salvage depth; a class-based estimate is
/// enough to tell an inter-word move from a kern adjustment.
fn est_em_width(s: &str) -> f64 {
    s.chars()
        .map(|c| match c {
            'i' | 'l' | 'I' | 'j' | 't' | 'f' | 'r' | '(' | ')' | '.' | ',' | ';' | ':' | '\''
            | '!' => 0.30,
            'm' | 'w' => 0.78,
            'M' | 'W' => 0.90,
            c if c.is_uppercase() => 0.70,
            c if c.is_ascii_digit() => 0.50,
            ' ' => 0.28,
            _ => 0.52,
        })
        .sum()
}

/// Word-gap and line-break reconstruction. Positions are text-space x/y;
/// every comparison happens in em units (divided by the active font size) so
/// one set of thresholds survives 8pt footnotes and 24pt titles.
struct TextCursor {
    size: f64,
    /// x of the current line origin — last `Tm` e, or accumulated `Td` tx.
    line_x: f64,
    line_y: f64,
    /// x where the previous show's estimated advance ended.
    pen_x: f64,
    /// a `Tm`/`Td` moved the line origin since the last show (vs. a plain
    /// continuation, which resumes at `pen_x`).
    at_line_origin: bool,
    /// any deliberate repositioning since the last show — lowers the
    /// word-space floor, because positioning ops are authored gaps while
    /// residuals between bare shows are width-estimate noise.
    repositioned: bool,
    last_y: Option<f64>,
    last_line_x: Option<f64>,
}

impl TextCursor {
    fn new() -> Self {
        TextCursor {
            size: 10.0,
            line_x: 0.0,
            line_y: 0.0,
            pen_x: 0.0,
            at_line_origin: false,
            repositioned: false,
            last_y: None,
            last_line_x: None,
        }
    }

    fn push_space(out: &mut String) {
        if out.chars().last().is_some_and(|c| !c.is_whitespace()) {
            out.push(' ');
        }
    }

    fn push_newline(out: &mut String) {
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }

    /// Emit the separator implied by moving from the previous show to one
    /// starting at `start_x` on the current line: word space, paragraph
    /// join, or line break.
    fn separator(&mut self, start_x: f64, out: &mut String) {
        if let Some(last_y) = self.last_y {
            let dy = (self.line_y - last_y) / self.size;
            if dy < -0.5 || dy > 0.5 {
                // A different line. Snapping back to the same column start
                // is a wrapped paragraph line — join with a space, because
                // verbatim sentences span the wrap. A hyphen at the break
                // already separates; joining compounds ("encoder-decoder")
                // needs it kept, not doubled.
                let col_dx = (self.line_x - self.last_line_x.unwrap_or(self.line_x)) / self.size;
                if col_dx.abs() < 3.0 {
                    if !out.ends_with('-') {
                        Self::push_space(out);
                    }
                } else {
                    Self::push_newline(out);
                }
                self.last_line_x = Some(self.line_x);
            } else {
                // Same line: the distance between where the previous text
                // (estimated) ended and where this one starts decides.
                let gap = (start_x - self.pen_x) / self.size;
                let floor = if self.repositioned { 0.2 } else { 0.45 };
                if gap >= 1.8 {
                    Self::push_newline(out);
                } else if gap >= floor {
                    Self::push_space(out);
                }
            }
        } else {
            self.last_line_x = Some(self.line_x);
        }
        self.last_y = Some(self.line_y);
    }

    /// Show one decoded string. `width_em` is the exact advance when the
    /// font declares `/Widths`; `move_em` folds in a `TJ` number cluster
    /// (thousandths of an em) that repositioned since the previous fragment.
    fn show(&mut self, text: &str, width_em: Option<f64>, move_em: f64, out: &mut String) {
        if text.is_empty() {
            return;
        }
        let mut start_x = if self.at_line_origin {
            self.line_x
        } else {
            self.pen_x
        };
        if move_em != 0.0 {
            start_x += move_em * self.size;
            self.repositioned = true;
        }
        self.separator(start_x, out);
        out.push_str(text);
        let w = width_em.unwrap_or_else(|| est_em_width(text));
        self.pen_x = start_x + w * self.size;
        self.at_line_origin = false;
        self.repositioned = false;
    }

    /// A trailing `TJ` number cluster after the last string advances the pen
    /// for whatever show comes next.
    fn trailing_move(&mut self, move_em: f64) {
        if move_em != 0.0 {
            self.pen_x += move_em * self.size;
            self.repositioned = true;
        }
    }
}

fn interpret_content(
    content: &[u8],
    font_names: &HashMap<String, u32>,
    fonts: &mut HashMap<u32, FontMap>,
    out: &mut String,
) {
    let mut lex = Lexer { b: content, pos: 0 };
    let mut operands: Vec<Value> = Vec::new();
    let mut font: Option<u32> = None;
    // Fonts without ToUnicode still need a code width: decode via cp1252.
    let plain = FontMap {
        code_len: 1,
        map: HashMap::new(),
        widths: HashMap::new(),
        default_width: 0.5,
        mac_roman: false,
    };
    let mut cur = TextCursor::new();
    loop {
        lex.skip_ws();
        if lex.pos >= content.len() {
            break;
        }
        // Operands: name / string ( [ < + - . digit — parsed as values and
        // stacked for the next operator. Anything else starts an operator
        // keyword (Tf Tj TJ q Q …).
        match content[lex.pos] {
            b'/' | b'(' | b'[' | b'<' | b'+' | b'-' | b'.' | b'0'..=b'9' => {
                match lex.parse_value() {
                    Some(v) => operands.push(v),
                    None => lex.pos += 1,
                }
                continue;
            }
            _ => {}
        }
        let kw_start = lex.pos;
        while lex.pos < content.len()
            && (content[lex.pos].is_ascii_alphabetic()
                || content[lex.pos] == b'*'
                || content[lex.pos] == b'\''
                || content[lex.pos] == b'"')
        {
            lex.pos += 1;
        }
        if lex.pos == kw_start {
            lex.pos += 1;
            continue;
        }
        let kw = &content[kw_start..lex.pos];
        match kw {
            b"BI" => {
                // inline image: skip binary through EI
                if let Some(ei) = find(content, lex.pos, b"EI") {
                    lex.pos = ei + 2;
                } else {
                    break;
                }
            }
            b"Tf" => {
                if let Some(Value::Name(n)) = operands.first() {
                    font = font_names.get(n).copied();
                }
                if let Some(Value::Num(s)) = operands.get(1) {
                    if *s != 0.0 {
                        cur.size = s.abs();
                    }
                }
            }
            b"Tm" => {
                if let (Some(Value::Num(e)), Some(Value::Num(f))) =
                    (operands.get(4), operands.get(5))
                {
                    cur.line_x = *e;
                    cur.line_y = *f;
                    cur.at_line_origin = true;
                    cur.repositioned = true;
                }
            }
            b"Td" | b"TD" => {
                if let (Some(Value::Num(tx)), Some(Value::Num(ty))) = (
                    operands.get(operands.len().saturating_sub(2)),
                    operands.last(),
                ) {
                    // A `0 0 Td` no-op is common between shows of one line —
                    // treating it as a move snaps the next fragment back to
                    // the line origin and glues words together.
                    if *tx != 0.0 || *ty != 0.0 {
                        cur.line_x += tx;
                        cur.line_y += ty;
                        cur.at_line_origin = true;
                        cur.repositioned = true;
                    }
                }
            }
            b"T*" => {
                cur.line_y -= 1.2 * cur.size;
                cur.at_line_origin = true;
                cur.repositioned = true;
            }
            b"Tj" | b"'" | b"\"" => {
                // ' and " move to the next line before showing.
                if kw != b"Tj" {
                    cur.line_y -= 1.2 * cur.size;
                    cur.at_line_origin = true;
                    cur.repositioned = true;
                }
                if let Some(Value::Str(bytes)) = operands.last() {
                    match font.and_then(|f| fonts.get(&f)) {
                        Some(fm) => cur.show(&fm.decode(bytes), Some(fm.em_width(bytes)), 0.0, out),
                        None => cur.show(&plain.decode(bytes), None, 0.0, out),
                    }
                }
            }
            b"TJ" => {
                if let Some(Value::Array(items)) = operands.last() {
                    let fm = font.and_then(|f| fonts.get(&f));
                    let mut move_em = 0.0f64;
                    for it in items {
                        match it {
                            Value::Num(n) => move_em += -n / 1000.0,
                            Value::Str(bytes) => {
                                let text = match fm {
                                    Some(f) => f.decode(bytes),
                                    None => plain.decode(bytes),
                                };
                                if !text.is_empty() {
                                    let w = fm.map(|f| f.em_width(bytes));
                                    cur.show(&text, w, move_em, out);
                                    move_em = 0.0;
                                }
                            }
                            _ => {}
                        }
                    }
                    cur.trailing_move(move_em);
                }
            }
            _ => {}
        }
        operands.clear();
    }
}

/// AGL subset: what `/Differences` arrays actually carry in benchmark PDFs —
/// ligatures, digits, punctuation; single-letter glyph names name
/// themselves. Ligatures map to their plain letters ("fi", not U+FB01):
/// verbatim sentence matching normalizes nothing.
fn glyph_unicode(name: &str) -> Option<String> {
    let s = match name {
        "fi" => "fi",
        "fl" => "fl",
        "ff" => "ff",
        "ffi" => "ffi",
        "ffl" => "ffl",
        "space" => " ",
        "period" => ".",
        "comma" => ",",
        "hyphen" => "-",
        "colon" => ":",
        "semicolon" => ";",
        "slash" => "/",
        "backslash" => "\\",
        "parenleft" => "(",
        "parenright" => ")",
        "bracketleft" => "[",
        "bracketright" => "]",
        "quotedbl" => "\"",
        "quotesingle" => "'",
        "endash" => "\u{2013}",
        "emdash" => "\u{2014}",
        "bullet" => "\u{2022}",
        "zero" => "0",
        "one" => "1",
        "two" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        _ => {
            let mut cs = name.chars();
            return match (cs.next(), cs.next()) {
                (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_string()),
                _ => None,
            };
        }
    };
    Some(s.to_string())
}

/// Resolve a page's fonts lazily on first use.
fn font_map_for(font_obj: u32, objects: &HashMap<u32, PdfObj>, cache: &mut HashMap<u32, FontMap>) {
    if cache.contains_key(&font_obj) {
        return;
    }
    let empty = || FontMap {
        code_len: 1,
        map: HashMap::new(),
        widths: HashMap::new(),
        default_width: 0.5,
        mac_roman: false,
    };
    let dict = objects.get(&font_obj).and_then(|o| o.dict.as_ref());
    // Quartz output declares /Encoding /MacRomanEncoding by NAME — the dict
    // form with /Differences is handled below.
    let mac = matches!(
        dict.and_then(|d| d.name("Encoding")),
        Some("MacRomanEncoding")
    );
    let mut map = dict
        .and_then(|d| d.ref_of("ToUnicode"))
        .and_then(|(n, _)| objects.get(&n))
        .and_then(|o| o.stream.clone())
        .map(|s| parse_cmap(&s))
        .unwrap_or_else(empty);
    // ToUnicode stays authoritative where it mapped a code; Differences
    // rebuilds the rest — fonts without a CMap (Word, older TeX) put their
    // ligatures there, and the cp1252 fallback alone turns "file" into "Þle".
    if let Some(d) = dict {
        if let Some(enc) = d.get("Encoding").and_then(|v| resolve_dict_obj(v, objects)) {
            if let Some(Value::Array(items)) = enc.get("Differences") {
                let mut code: Option<u32> = None;
                for it in items {
                    match it {
                        Value::Num(n) => code = Some(*n as u32),
                        Value::Name(g) => {
                            if let Some(c) = code {
                                if let Some(txt) = glyph_unicode(g) {
                                    map.map.entry(c).or_insert(txt);
                                }
                                code = Some(c + 1);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // /Widths: exact per-code advances (glyph-space thousandths), inline or
    // behind an indirect ref. Without them the class estimate's error over a
    // word can swallow a real inter-word gap.
    if let Some(d) = dict {
        let widths_val = d.get("Widths").and_then(|v| match v {
            Value::Array(_) => Some(v.clone()),
            Value::Ref(n, _) => objects.get(n).and_then(|o| o.value.clone()),
            _ => None,
        });
        if let (Some(first), Some(Value::Array(items))) = (d.num("FirstChar"), widths_val) {
            for (i, it) in items.iter().enumerate() {
                if let Value::Num(w) = it {
                    map.widths.insert(first as u32 + i as u32, w / 1000.0);
                }
            }
        }
    }
    map.mac_roman = mac;
    cache.insert(font_obj, map);
}

// Whitespace/control squash shared with the DOCX salvage family.
pub(super) fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for ch in s.chars() {
        if ch == '\r' {
            continue;
        }
        if ch == '\n' {
            blank += 1;
            if blank <= 2 {
                out.push(ch);
            }
        } else {
            blank = 0;
            if ch.is_control() {
                out.push(' ');
            } else {
                out.push(ch);
            }
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdf_with(content: &str, tounicode: &str) -> Vec<u8> {
        let cmap = tounicode.as_bytes().to_vec();
        let stream = content.as_bytes().to_vec();
        format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n\
             4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n\
             5 0 obj\n<< /Type /Font /Subtype /Type1 /ToUnicode 6 0 R >>\nendobj\n\
             6 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n\
             trailer\n<< /Root 1 0 R >>\n%%EOF\n",
            stream.len(),
            content,
            cmap.len(),
            tounicode
        )
        .into_bytes()
    }

    #[test]
    fn extracts_simple_text_with_tounicode() {
        let pdf = pdf_with(
            "BT /F1 12 Tf (Hi) Tj ( there) Tj ET",
            "/CMapType 2 def\n1 begincodespacerange\n<00> <ff>\nendcodespacerange\n\
             1 beginbfchar\n<48> <0048>\n<69> <0069>\n<20> <0020>\n\
             <74> <0074>\n<68> <0068>\n<65> <0065>\n<72> <0072>\nendbfchar",
        );
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("Hi there"), "got: {text:?}");
    }

    #[test]
    fn tj_array_and_fallback_decode() {
        let pdf = pdf_with("BT /F1 12 Tf [(A) -20 (B)] TJ ET", "");
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("AB"), "got: {text:?}");
    }

    /// macOS Quartz PDFs declare `/Encoding /MacRomanEncoding`, where
    /// 0xDE/0xDF are the fi/fl ligatures — cp1252 reads them as Þ/þ.
    #[test]
    fn mac_roman_encoding_decodes_ligatures() {
        let content = "BT /F1 12 Tf (simple PDF ) Tj <DE> Tj (le) Tj ET";
        let stream = content.as_bytes().to_vec();
        let pdf = format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n\
             4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n\
             5 0 obj\n<< /Type /Font /Subtype /TrueType /Encoding /MacRomanEncoding >>\nendobj\n\
             trailer\n<< /Root 1 0 R >>\n%%EOF\n",
            stream.len(),
            content,
        )
        .into_bytes();
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("simple PDF file"), "got: {text:?}");
    }

    #[test]
    fn rejects_non_pdf() {
        assert!(pdf_to_text(b"<html>not a pdf</html>").is_none());
    }

    /// pdftex emits words as TJ fragments with move numbers instead of space
    /// glyphs — the numbers ARE the spaces.
    #[test]
    fn tj_word_gaps_become_spaces() {
        let pdf = pdf_with(
            "BT /F1 10 Tf [(The) -300 (goal) -300 (of) -300 (reducing)] TJ ET",
            "",
        );
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("The goal of reducing"), "got: {text:?}");
    }

    /// Small adjustments are kerning, not words.
    #[test]
    fn tj_kerning_stays_glued() {
        let pdf = pdf_with("BT /F1 10 Tf [(A) -20 (V) -20 (A)] TJ ET", "");
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("AVA"), "got: {text:?}");
    }

    /// Per-word `Td` positioning on the same line (dvips style). `Td` offsets
    /// are from the previous line origin, so the moves are large even for a
    /// one-word gap.
    #[test]
    fn td_word_moves_become_spaces() {
        let pdf = pdf_with(
            "BT /F1 10 Tf (The) Tj 20 0 Td (goal) Tj 24 0 Td (of) Tj ET",
            "",
        );
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("The goal of"), "got: {text:?}");
    }

    /// A sentence wrapped across two lines of the same column must come back
    /// joined — verbatim fact checks span the wrap.
    #[test]
    fn wrapped_line_joins_paragraph() {
        let pdf = pdf_with(
            "BT /F1 10 Tf 1 0 0 1 50 700 Tm (forms the) Tj ET \
             BT /F1 10 Tf 1 0 0 1 50 686 Tm (foundation) Tj ET",
            "",
        );
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("forms the foundation"), "got: {text:?}");
    }

    /// Same baseline but a distant column start: a line break, not a wall of
    /// spaces.
    #[test]
    fn column_switch_breaks_line() {
        let pdf = pdf_with(
            "BT /F1 10 Tf 1 0 0 1 50 700 Tm (alpha) Tj ET \
             BT /F1 10 Tf 1 0 0 1 300 700 Tm (beta) Tj ET",
            "",
        );
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("alpha\nbeta"), "got: {text:?}");
    }

    /// Ligature codes live in `/Differences` when the font ships no
    /// ToUnicode; cp1252 alone decodes 0xDE as Þ ("PDF Þle").
    #[test]
    fn differences_map_ligatures() {
        let content = "BT /F1 12 Tf (PDF ) Tj <DE> Tj (le) Tj ET";
        let stream = content.as_bytes().to_vec();
        let pdf = format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n\
             4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n\
             5 0 obj\n<< /Type /Font /Subtype /Type1 /Encoding << /Type /Encoding /Differences [222 /fi /fl] >> >>\nendobj\n\
             trailer\n<< /Root 1 0 R >>\n%%EOF\n",
            stream.len(),
            content,
        )
        .into_bytes();
        let text = pdf_to_text(&pdf).unwrap();
        assert!(text.contains("PDF file"), "got: {text:?}");
    }

    /// Real-fixture smoke test — opt-in via env because the fixture lives
    /// outside the repo (2MB arXiv paper). Skipped when unset.
    #[test]
    fn salvage_against_local_fixture() {
        let Ok(path) = std::env::var("DITING_PDF_FIXTURE") else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let text = if bytes.starts_with(b"PK\x03\x04") {
            crate::server::docx_text::docx_to_text(&bytes).expect("fixture must extract")
        } else {
            pdf_to_text(&bytes).expect("fixture must extract")
        };
        assert!(!text.trim().is_empty(), "extraction empty");
        if let Ok(dump) = std::env::var("DITING_PDF_DUMP") {
            let _ = std::fs::write(&dump, &text);
            eprintln!("dumped {} chars to {dump}", text.len());
            return;
        }
        assert!(text.len() > 10_000, "len {}", text.len());
        let compact: String = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        assert!(
            compact.contains("attentionisallyouneed"),
            "title missing in compact form"
        );
    }
}
