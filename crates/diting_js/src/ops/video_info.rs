//! Video container header metadata (god-file ratchet split, the media
//! sibling of image_info.rs).

/// Metadata from an mp4/quicktime header alone — the video counterpart of
/// `image_header_dimensions`. `moov/mvhd` carries timescale+duration,
/// `moov/trak/tkhd` the 16.16 video width/height (a 90°/270° rotation
/// matrix swaps them — Chrome reports the rotated face). Media pipelines
/// gate their upload on `loadedmetadata` with nonzero `videoWidth`/
/// `videoHeight` (the X composer never INITs its chunked upload while the
/// preview `<video>` sits at 0×0/NaN — same disease as the xhs image
/// dims fix), but a demuxer/decoder is not this engine's job: header
/// parsing is dependency-free and drives the metadata face exactly.
/// Truncated/garbage input is `None`, never a panic.
pub(super) struct VideoHeaderMeta {
    pub width: u32,
    pub height: u32,
    /// `None` = the container marks the duration unknown/infinite
    /// (all-ones or zero); the JS face then reports `Infinity`.
    pub duration_secs: Option<f64>,
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// One `[size u32 BE][type 4]` box. `size == 0` (runs to EOF) and
/// `size == 1` (64-bit largesize at +8) are normalized; `payload`/`end`
/// bound the box's children, clamped to the buffer.
struct BoxRange {
    ty: [u8; 4],
    payload: usize,
    end: usize,
}

fn next_box(b: &[u8], i: usize) -> Option<BoxRange> {
    if i + 8 > b.len() {
        return None;
    }
    let mut size = be32(&b[i..]) as usize;
    let ty = [b[i + 4], b[i + 5], b[i + 6], b[i + 7]];
    let mut header = 8usize;
    if size == 1 {
        if i + 16 > b.len() {
            return None;
        }
        let big = u64::from_be_bytes([
            b[i + 8], b[i + 9], b[i + 10], b[i + 11],
            b[i + 12], b[i + 13], b[i + 14], b[i + 15],
        ]);
        if big > usize::MAX as u64 {
            return None;
        }
        size = big as usize;
        header = 16;
    } else if size == 0 {
        size = b.len() - i; // box runs to end of data
    }
    if size < header || i.checked_add(size).map_or(true, |e| e > b.len()) {
        return None;
    }
    Some(BoxRange { ty, payload: i + header, end: i + size })
}

/// tkhd video size. The width/height offsets are version-dependent
/// (v0 dims at +76, v1 at +88 — the 8-byte times and 8-byte duration
/// push everything after them); the 9×i32 matrix sits 36 bytes ahead of
/// the dims, and a quarter turn puts the ±0x00010000 units off the
/// diagonal — the display axes swap. Sound/hint tracks carry 0×0 and
/// are skipped.
fn tkhd_video_size(p: &[u8]) -> Option<(u32, u32)> {
    let (dim_off, matrix_off) = if p.first() == Some(&1) { (88usize, 52usize) } else { (76usize, 40usize) };
    if p.len() < dim_off + 8 {
        return None;
    }
    let w = be32(&p[dim_off..]) >> 16;
    let h = be32(&p[dim_off + 4..]) >> 16;
    if w == 0 || h == 0 {
        return None;
    }
    if p.len() >= matrix_off + 36 {
        let unit = |k: usize| {
            i32::from_be_bytes([
                p[matrix_off + k * 4],
                p[matrix_off + k * 4 + 1],
                p[matrix_off + k * 4 + 2],
                p[matrix_off + k * 4 + 3],
            ])
            .unsigned_abs()
        };
        if unit(1) == 0x0001_0000 || unit(3) == 0x0001_0000 {
            return Some((h, w)); // rotated 90°/270°
        }
    }
    Some((w, h))
}

/// mvhd duration in seconds. v0: timescale u32@12, duration u32@16; v1
/// pushes both past the 8-byte times (+8). All-ones (and zero — a
/// live/streaming marker) mean "no duration": the HTML face reports
/// `Infinity`, not a number.
fn mvhd_duration_secs(p: &[u8]) -> Option<f64> {
    let (ts_off, dur_off, wide) = if p.first() == Some(&1) { (20usize, 24usize, true) } else { (12usize, 16usize, false) };
    if p.len() < if wide { dur_off + 8 } else { dur_off + 4 } {
        return None;
    }
    let timescale = be32(&p[ts_off..]);
    if timescale == 0 {
        return None;
    }
    let raw = if wide {
        let d = u64::from_be_bytes([
            p[dur_off], p[dur_off + 1], p[dur_off + 2], p[dur_off + 3],
            p[dur_off + 4], p[dur_off + 5], p[dur_off + 6], p[dur_off + 7],
        ]);
        if d == 0 || d >= u32::MAX as u64 {
            return None;
        }
        d as f64
    } else {
        let d = be32(&p[dur_off..]);
        if d == 0 || d == u32::MAX {
            return None;
        }
        d as f64
    };
    Some(raw / timescale as f64)
}

pub(super) fn video_header_meta(b: &[u8]) -> Option<VideoHeaderMeta> {
    let mut i = 0usize;
    while let Some(bx) = next_box(b, i) {
        if bx.ty == *b"moov" {
            return moov_meta(&b[bx.payload..bx.end]);
        }
        i = bx.end;
    }
    None
}

/// Inside moov: mvhd duration + the first video trak's tkhd size. A moov
/// with neither (or only sound traks and no mvhd) parses as nothing; one
/// with only mvhd is an audio file — Chrome still reports its metadata
/// face at 0×0 on a `<video>`, so the dims stay zero and the caller fires.
fn moov_meta(moov: &[u8]) -> Option<VideoHeaderMeta> {
    let mut duration_secs = None;
    let mut size = None;
    let mut i = 0usize;
    while let Some(bx) = next_box(moov, i) {
        let p = &moov[bx.payload..bx.end];
        if bx.ty == *b"mvhd" {
            if duration_secs.is_none() {
                duration_secs = mvhd_duration_secs(p);
            }
        } else if bx.ty == *b"trak" && size.is_none() {
            let mut j = 0usize;
            while let Some(inner) = next_box(p, j) {
                if inner.ty == *b"tkhd" {
                    if let Some(s) = tkhd_video_size(&p[inner.payload..inner.end]) {
                        size = Some(s);
                        break;
                    }
                }
                j = inner.end;
            }
        }
        i = bx.end;
    }
    if duration_secs.is_none() && size.is_none() {
        return None;
    }
    let (width, height) = size.unwrap_or((0, 0));
    Some(VideoHeaderMeta { width, height, duration_secs })
}
