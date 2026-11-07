//! Baseline JPEG encoder, RGB8 in (#42 ④).
//!
//! Taken from the `image` crate 0.25.10 (`src/codecs/jpeg/encoder.rs`,
//! `entropy.rs`, `transform.rs`; MIT OR Apache-2.0) on 2026-11-07 when the
//! `image` dependency left the tree, and pruned to the one face the product
//! uses: `encode_jpeg_rgb` (authoring's JPEG page export + decode-test
//! fixtures). Intake changes — the generic `GenericImageView` machinery,
//! gray path, ICC/EXIF/pixel-density surface and `image`'s error plumbing
//! are gone; tables, the bit writer, RGB→YCbCr fixed-point math and fdct
//! are unchanged. Provenance is recorded in `crates/VENDORED.md`.

use std::io::{self, Write};
use std::{error, fmt};

// Markers
// Baseline DCT
static SOF0: u8 = 0xC0;
// Huffman Tables
static DHT: u8 = 0xC4;
// Start of Image (standalone)
static SOI: u8 = 0xD8;
// End of image (standalone)
static EOI: u8 = 0xD9;
// Start of Scan
static SOS: u8 = 0xDA;
// Quantization Tables
static DQT: u8 = 0xDB;
// Application segments start
static APP0: u8 = 0xE0;

// section K.1
// table K.1
#[rustfmt::skip]
static STD_LUMA_QTABLE: [u8; 64] = [
    16, 11, 10, 16,  24,  40,  51,  61,
    12, 12, 14, 19,  26,  58,  60,  55,
    14, 13, 16, 24,  40,  57,  69,  56,
    14, 17, 22, 29,  51,  87,  80,  62,
    18, 22, 37, 56,  68, 109, 103,  77,
    24, 35, 55, 64,  81, 104, 113,  92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103,  99,
];

// table K.2
#[rustfmt::skip]
static STD_CHROMA_QTABLE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

// section K.3
// Code lengths and values for table K.3
static STD_LUMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

static STD_LUMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];

static STD_LUMA_DC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_LUMA_DC_CODE_LENGTHS, &STD_LUMA_DC_VALUES);

// Code lengths and values for table K.4
static STD_CHROMA_DC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
];

static STD_CHROMA_DC_VALUES: [u8; 12] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
];

static STD_CHROMA_DC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_CHROMA_DC_CODE_LENGTHS, &STD_CHROMA_DC_VALUES);

// Code lengths and values for table k.5
static STD_LUMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
];

static STD_LUMA_AC_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

static STD_LUMA_AC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_LUMA_AC_CODE_LENGTHS, &STD_LUMA_AC_VALUES);

// Code lengths and values for table k.6
static STD_CHROMA_AC_CODE_LENGTHS: [u8; 16] = [
    0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77,
];
static STD_CHROMA_AC_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

static STD_CHROMA_AC_HUFF_LUT: [(u8, u16); 256] =
    build_huff_lut_const(&STD_CHROMA_AC_CODE_LENGTHS, &STD_CHROMA_AC_VALUES);

static DCCLASS: u8 = 0;
static ACCLASS: u8 = 1;

static LUMADESTINATION: u8 = 0;
static CHROMADESTINATION: u8 = 1;

static LUMAID: u8 = 1;
static CHROMABLUEID: u8 = 2;
static CHROMAREDID: u8 = 3;

/// The permutation of dct coefficients.
#[rustfmt::skip]
static UNZIGZAG: [u8; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

/// Errors that can occur when encoding a JPEG image.
#[derive(Debug)]
pub enum JpegEncodeError {
    /// JPEG does not support this size.
    InvalidSize(u32, u32),
    /// The pixel buffer is not `width * height * 3` bytes.
    BadBufferLen { expected: usize, got: usize },
    /// The writer failed.
    Io(io::Error),
}

impl fmt::Display for JpegEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JpegEncodeError::InvalidSize(w, h) => f.write_fmt(format_args!(
                "invalid image size ({w} x {h}) to encode as JPEG: width and height must be >= 1 and <= 65535"
            )),
            JpegEncodeError::BadBufferLen { expected, got } => f.write_fmt(format_args!(
                "pixel buffer is {got} bytes, expected {expected} (width * height * 3)"
            )),
            JpegEncodeError::Io(e) => e.fmt(f),
        }
    }
}

impl error::Error for JpegEncodeError {}

impl From<io::Error> for JpegEncodeError {
    fn from(e: io::Error) -> Self {
        JpegEncodeError::Io(e)
    }
}

/// Encode an interleaved RGB8 buffer as baseline JPEG into `out`, with
/// `quality` in 1-100 (libjpeg's scaling algorithm; 75 matches the authoring
/// export's historical default). The result has no subsampling and a plain
/// 1:1 JFIF header — byte-compatible with what `image` 0.25.10's
/// `JpegEncoder` produced for the same input at intake.
pub fn encode_jpeg_rgb(
    out: &mut Vec<u8>,
    rgb: &[u8],
    width: u32,
    height: u32,
    quality: u8,
) -> Result<(), JpegEncodeError> {
    // Size gate first: the buffer-length arithmetic below is only safe once
    // width/height are known to fit u16 (JPEG's own dimension limit).
    let (width, height) = match (u16::try_from(width), u16::try_from(height)) {
        (Ok(w @ 1..), Ok(h @ 1..)) => (w, h),
        _ => return Err(JpegEncodeError::InvalidSize(width, height)),
    };
    let expected = width as usize * height as usize * 3;
    if rgb.len() != expected {
        return Err(JpegEncodeError::BadBufferLen {
            expected,
            got: rgb.len(),
        });
    }

    // Derive our quantization table scaling value using the libjpeg algorithm
    let scale = u32::from(clamp(quality, 1, 100));
    let scale = if scale < 50 {
        5000 / scale
    } else {
        200 - scale * 2
    };

    let mut tables = [STD_LUMA_QTABLE, STD_CHROMA_QTABLE];
    for t in tables.iter_mut() {
        for v in t.iter_mut() {
            *v = clamp((u32::from(*v) * scale + 50) / 100, 1, u32::from(u8::MAX)) as u8;
        }
    }

    let components = [
        Component {
            id: LUMAID,
            h: 1,
            v: 1,
            tq: LUMADESTINATION,
            dc_table: LUMADESTINATION,
            ac_table: LUMADESTINATION,
        },
        Component {
            id: CHROMABLUEID,
            h: 1,
            v: 1,
            tq: CHROMADESTINATION,
            dc_table: CHROMADESTINATION,
            ac_table: CHROMADESTINATION,
        },
        Component {
            id: CHROMAREDID,
            h: 1,
            v: 1,
            tq: CHROMADESTINATION,
            dc_table: CHROMADESTINATION,
            ac_table: CHROMADESTINATION,
        },
    ];

    let mut writer = BitWriter::new(out);

    writer.write_marker(SOI)?;

    let mut buf = Vec::new();

    build_jfif_header(&mut buf);
    writer.write_segment(APP0, &buf)?;

    build_frame_header(&mut buf, 8, width, height, &components);
    writer.write_segment(SOF0, &buf)?;

    for (i, table) in tables.iter().enumerate() {
        build_quantization_segment(&mut buf, 8, i as u8, table);
        writer.write_segment(DQT, &buf)?;
    }

    build_huffman_segment(
        &mut buf,
        DCCLASS,
        LUMADESTINATION,
        &STD_LUMA_DC_CODE_LENGTHS,
        &STD_LUMA_DC_VALUES,
    );
    writer.write_segment(DHT, &buf)?;

    build_huffman_segment(
        &mut buf,
        ACCLASS,
        LUMADESTINATION,
        &STD_LUMA_AC_CODE_LENGTHS,
        &STD_LUMA_AC_VALUES,
    );
    writer.write_segment(DHT, &buf)?;

    build_huffman_segment(
        &mut buf,
        DCCLASS,
        CHROMADESTINATION,
        &STD_CHROMA_DC_CODE_LENGTHS,
        &STD_CHROMA_DC_VALUES,
    );
    writer.write_segment(DHT, &buf)?;

    build_huffman_segment(
        &mut buf,
        ACCLASS,
        CHROMADESTINATION,
        &STD_CHROMA_AC_CODE_LENGTHS,
        &STD_CHROMA_AC_VALUES,
    );
    writer.write_segment(DHT, &buf)?;

    build_scan_header(&mut buf, &components);
    writer.write_segment(SOS, &buf)?;

    encode_rgb_blocks(&mut writer, rgb, width, height, &tables)?;

    writer.pad_byte()?;
    writer.write_marker(EOI)?;
    Ok(())
}

#[inline]
fn clamp<N: PartialOrd>(a: N, min: N, max: N) -> N {
    if a < min {
        min
    } else if a > max {
        max
    } else {
        a
    }
}

/// A representation of a JPEG component
#[derive(Copy, Clone)]
struct Component {
    /// The Component's identifier
    id: u8,
    /// Horizontal sampling factor
    h: u8,
    /// Vertical sampling factor
    v: u8,
    /// The quantization table selector
    tq: u8,
    /// Index to the Huffman DC Table
    dc_table: u8,
    /// Index to the AC Huffman Table
    ac_table: u8,
}

fn encode_rgb_blocks(
    writer: &mut BitWriter<'_>,
    rgb: &[u8],
    width: u16,
    height: u16,
    tables: &[[u8; 64]; 2],
) -> io::Result<()> {
    let mut y_dcprev = 0;
    let mut cb_dcprev = 0;
    let mut cr_dcprev = 0;

    let mut dct_yblock = [0i32; 64];
    let mut dct_cb_block = [0i32; 64];
    let mut dct_cr_block = [0i32; 64];

    let mut yblock = [0u8; 64];
    let mut cb_block = [0u8; 64];
    let mut cr_block = [0u8; 64];

    let (width, height) = (u32::from(width), u32::from(height));

    for y in (0..height).step_by(8) {
        for x in (0..width).step_by(8) {
            // RGB -> YCbCr
            copy_blocks_ycbcr(
                rgb,
                width,
                height,
                x,
                y,
                &mut yblock,
                &mut cb_block,
                &mut cr_block,
            );

            // Level shift and fdct
            // Coeffs are scaled by 8
            fdct(&yblock, &mut dct_yblock);
            fdct(&cb_block, &mut dct_cb_block);
            fdct(&cr_block, &mut dct_cr_block);

            // Quantization
            for i in 0usize..64 {
                dct_yblock[i] =
                    ((dct_yblock[i] / 8) as f32 / f32::from(tables[0][i])).round() as i32;
                dct_cb_block[i] =
                    ((dct_cb_block[i] / 8) as f32 / f32::from(tables[1][i])).round() as i32;
                dct_cr_block[i] =
                    ((dct_cr_block[i] / 8) as f32 / f32::from(tables[1][i])).round() as i32;
            }

            y_dcprev = writer.write_block(
                &dct_yblock,
                y_dcprev,
                &STD_LUMA_DC_HUFF_LUT,
                &STD_LUMA_AC_HUFF_LUT,
            )?;
            cb_dcprev = writer.write_block(
                &dct_cb_block,
                cb_dcprev,
                &STD_CHROMA_DC_HUFF_LUT,
                &STD_CHROMA_AC_HUFF_LUT,
            )?;
            cr_dcprev = writer.write_block(
                &dct_cr_block,
                cr_dcprev,
                &STD_CHROMA_DC_HUFF_LUT,
                &STD_CHROMA_AC_HUFF_LUT,
            )?;
        }
    }

    Ok(())
}

struct BitWriter<'a> {
    w: &'a mut Vec<u8>,
    accumulator: u32,
    nbits: u8,
}

impl BitWriter<'_> {
    fn new(w: &mut Vec<u8>) -> BitWriter<'_> {
        BitWriter {
            w,
            accumulator: 0,
            nbits: 0,
        }
    }

    fn write_bits(&mut self, bits: u16, size: u8) -> io::Result<()> {
        if size == 0 {
            return Ok(());
        }

        self.nbits += size;
        self.accumulator |= u32::from(bits) << (32 - self.nbits) as usize;

        while self.nbits >= 8 {
            let byte = self.accumulator >> 24;
            self.w.write_all(&[byte as u8])?;

            if byte == 0xFF {
                self.w.write_all(&[0x00])?;
            }

            self.nbits -= 8;
            self.accumulator <<= 8;
        }

        Ok(())
    }

    fn pad_byte(&mut self) -> io::Result<()> {
        self.write_bits(0x7F, 7)
    }

    fn huffman_encode(&mut self, val: u8, table: &[(u8, u16); 256]) -> io::Result<()> {
        let (size, code) = table[val as usize];

        assert!(size <= 16, "bad huffman value");

        self.write_bits(code, size)
    }

    fn write_block(
        &mut self,
        block: &[i32; 64],
        prevdc: i32,
        dctable: &[(u8, u16); 256],
        actable: &[(u8, u16); 256],
    ) -> io::Result<i32> {
        // Differential DC encoding
        let dcval = block[0];
        let diff = dcval - prevdc;
        let (size, value) = encode_coefficient(diff);

        self.huffman_encode(size, dctable)?;
        self.write_bits(value, size)?;

        // Figure F.2
        let mut zero_run = 0;

        for &k in &UNZIGZAG[1..] {
            if block[k as usize] == 0 {
                zero_run += 1;
            } else {
                while zero_run > 15 {
                    self.huffman_encode(0xF0, actable)?;
                    zero_run -= 16;
                }

                let (size, value) = encode_coefficient(block[k as usize]);
                let symbol = (zero_run << 4) | size;

                self.huffman_encode(symbol, actable)?;
                self.write_bits(value, size)?;

                zero_run = 0;
            }
        }

        if block[UNZIGZAG[63] as usize] == 0 {
            self.huffman_encode(0x00, actable)?;
        }

        Ok(dcval)
    }

    fn write_marker(&mut self, marker: u8) -> io::Result<()> {
        self.w.write_all(&[0xFF, marker])
    }

    fn write_segment(&mut self, marker: u8, data: &[u8]) -> io::Result<()> {
        self.w.write_all(&[0xFF, marker])?;
        self.w.write_all(&(data.len() as u16 + 2).to_be_bytes())?;
        self.w.write_all(data)
    }
}

fn build_jfif_header(m: &mut Vec<u8>) {
    m.clear();
    m.extend_from_slice(b"JFIF");
    // JFIF version 1.2, unit: pixel aspect ratio, density 1:1, no thumbnail —
    // the fixed bytes image 0.25.10 emitted for `JpegEncoder::new_with_quality`.
    m.extend_from_slice(&[0, 0x01, 0x02, 0x00, 0, 1, 0, 1, 0, 0]);
}

fn build_frame_header(
    m: &mut Vec<u8>,
    precision: u8,
    width: u16,
    height: u16,
    components: &[Component],
) {
    m.clear();

    m.push(precision);
    m.extend_from_slice(&height.to_be_bytes());
    m.extend_from_slice(&width.to_be_bytes());
    m.push(components.len() as u8);

    for &comp in components {
        let hv = (comp.h << 4) | comp.v;
        m.extend_from_slice(&[comp.id, hv, comp.tq]);
    }
}

fn build_scan_header(m: &mut Vec<u8>, components: &[Component]) {
    m.clear();

    m.push(components.len() as u8);

    for &comp in components {
        let tables = (comp.dc_table << 4) | comp.ac_table;
        m.extend_from_slice(&[comp.id, tables]);
    }

    // spectral start and end, approx. high and low
    m.extend_from_slice(&[0, 63, 0]);
}

fn build_huffman_segment(
    m: &mut Vec<u8>,
    class: u8,
    destination: u8,
    numcodes: &[u8; 16],
    values: &[u8],
) {
    m.clear();

    let tcth = (class << 4) | destination;
    m.push(tcth);

    m.extend_from_slice(numcodes);

    let sum: usize = numcodes.iter().map(|&x| x as usize).sum();

    assert_eq!(sum, values.len());

    m.extend_from_slice(values);
}

fn build_quantization_segment(m: &mut Vec<u8>, precision: u8, identifier: u8, qtable: &[u8; 64]) {
    m.clear();

    let p = if precision == 8 { 0 } else { 1 };

    let pqtq = (p << 4) | identifier;
    m.push(pqtq);

    for &i in &UNZIGZAG[..] {
        m.push(qtable[i as usize]);
    }
}

fn encode_coefficient(coefficient: i32) -> (u8, u16) {
    let mut magnitude = coefficient.unsigned_abs() as u16;
    let mut num_bits = 0u8;

    while magnitude > 0 {
        magnitude >>= 1;
        num_bits += 1;
    }

    let mask = (1 << num_bits as usize) - 1;

    let val = if coefficient < 0 {
        (coefficient - 1) as u16 & mask
    } else {
        coefficient as u16 & mask
    };

    (num_bits, val)
}

#[inline]
fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r: i32 = i32::from(r);
    let g: i32 = i32::from(g);
    let b: i32 = i32::from(b);

    /*
       JPEG RGB -> YCbCr is defined as following equations using Bt.601 Full Range matrix:
       Y  =  0.29900 * R + 0.58700 * G + 0.11400 * B
       Cb = -0.16874 * R - 0.33126 * G + 0.50000 * B  + 128
       Cr =  0.50000 * R - 0.41869 * G - 0.08131 * B  + 128

       To avoid using slow floating point conversion is done in fixed point,
       using following coefficients with rounding to nearest integer mode:
    */

    const C_YR: i32 = 19595; // 0.29900 = 19595 * 2^-16
    const C_YG: i32 = 38469; // 0.58700 = 38469 * 2^-16
    const C_YB: i32 = 7471; // 0.11400 = 7471 * 2^-16
    const Y_ROUNDING: i32 = (1 << 15) - 1; // + 0.5 to perform rounding shift right in-place
    const C_UR: i32 = 11059; // 0.16874 = 11059 * 2^-16
    const C_UG: i32 = 21709; // 0.33126 = 21709 * 2^-16
    const C_UB: i32 = 32768; // 0.5 = 32768 * 2^-16
    const UV_BIAS_ROUNDING: i32 = (128 * (1 << 16)) + ((1 << 15) - 1); // 128 + 0.5 = ((128 * (1 << 16)) + ((1 << 15) - 1)) * 2^-16 ; + 0.5 to perform rounding shift right in-place
    const C_VR: i32 = C_UB; // 0.5 = 32768 * 2^-16
    const C_VG: i32 = 27439; // 0.41869 = 27439 * 2^-16
    const C_VB: i32 = 5329; // 0.08131409 = 5329 * 2^-16

    let y = (C_YR * r + C_YG * g + C_YB * b + Y_ROUNDING) >> 16;
    let cb = (-C_UR * r - C_UG * g + C_UB * b + UV_BIAS_ROUNDING) >> 16;
    let cr = (C_VR * r - C_VG * g - C_VB * b + UV_BIAS_ROUNDING) >> 16;

    (y as u8, cb as u8, cr as u8)
}

#[allow(clippy::too_many_arguments)] // upstream shape (image 0.25.10)
fn copy_blocks_ycbcr(
    source: &[u8],
    width: u32,
    height: u32,
    x0: u32,
    y0: u32,
    yb: &mut [u8; 64],
    cbb: &mut [u8; 64],
    crb: &mut [u8; 64],
) {
    for y in 0..8u32 {
        for x in 0..8u32 {
            // The last block row/column repeats the edge pixel, matching
            // image 0.25's `pixel_at_or_near`.
            let px = (x + x0).min(width - 1) as usize;
            let py = (y + y0).min(height - 1) as usize;
            let i = (py * width as usize + px) * 3;
            let (yc, cb, cr) = rgb_to_ycbcr(source[i], source[i + 1], source[i + 2]);

            yb[(y * 8 + x) as usize] = yc;
            cbb[(y * 8 + x) as usize] = cb;
            crb[(y * 8 + x) as usize] = cr;
        }
    }
}

/// Given an array containing the number of codes of each code length,
/// this function generates the huffman codes lengths and their respective
/// code lengths as specified by the JPEG spec.
const fn derive_codes_and_sizes(bits: &[u8; 16]) -> ([u8; 256], [u16; 256]) {
    let mut huffsize = [0u8; 256];
    let mut huffcode = [0u16; 256];

    let mut k = 0;

    // Annex C.2
    // Figure C.1
    // Generate table of individual code lengths
    let mut i = 0;
    while i < 16 {
        let mut j = 0;
        while j < bits[i as usize] {
            huffsize[k] = i + 1;
            k += 1;
            j += 1;
        }
        i += 1;
    }

    huffsize[k] = 0;

    // Annex C.2
    // Figure C.2
    // Generate table of huffman codes
    k = 0;
    let mut code = 0u16;
    let mut size = huffsize[0];

    while huffsize[k] != 0 {
        huffcode[k] = code;
        code += 1;
        k += 1;

        if huffsize[k] == size {
            continue;
        }

        // FIXME there is something wrong with this code
        let diff = huffsize[k].wrapping_sub(size);
        code = if diff < 16 { code << diff as usize } else { 0 };

        size = size.wrapping_add(diff);
    }

    (huffsize, huffcode)
}

const fn build_huff_lut_const(bits: &[u8; 16], huffval: &[u8]) -> [(u8, u16); 256] {
    let mut lut = [(17u8, 0u16); 256];
    let (huffsize, huffcode) = derive_codes_and_sizes(bits);

    let mut i = 0;
    while i < huffval.len() {
        lut[huffval[i] as usize] = (huffsize[i], huffcode[i]);
        i += 1;
    }

    lut
}

/*
fdct is a Rust translation of jfdctint.c from the
Independent JPEG Group's libjpeg version 9a
obtained from http://www.ijg.org/files/jpegsr9a.zip
It comes with the following conditions of distribution and use:

    In plain English:

    1. We don't promise that this software works.  (But if you find any bugs,
        please let us know!)
    2. You can use this software for whatever you want.  You don't have to pay us.
    3. You may not pretend that you wrote this software.  If you use it in a
        program, you must acknowledge somewhere in your documentation that
        you've used the IJG code.

    In legalese:

    The authors make NO WARRANTY or representation, either express or implied,
    with respect to this software, its quality, accuracy, merchantability, or
    fitness for a particular purpose.  This software is provided "AS IS", and you,
    its user, assume the entire risk as to its quality and accuracy.

    This software is copyright (C) 1991-2014, Thomas G. Lane, Guido Vollbeding.
    All Rights Reserved except as specified below.

    Permission is hereby granted to use, copy, modify, and distribute this
    software (or portions thereof) for any purpose, without fee, subject to these
    conditions:
    (1) If any part of the source code for this software is distributed, then this
    README file must be included, with this copyright and no-warranty notice
    unaltered; and any additions, deletions, or changes to the original files
    must be clearly indicated in accompanying documentation.
    (2) If only executable code is distributed, then the accompanying
    documentation must state that "this software is based in part on the work of
    the Independent JPEG Group".
    (3) Permission for use of this software is granted only if the user accepts
    full responsibility for any undesirable consequences; the authors accept
    NO LIABILITY for damages of any kind.

    These conditions apply to any software derived from or based on the IJG code,
    not just the unmodified library.  If you use our work, you ought to
    acknowledge us.

    Permission is NOT granted for the use of any IJG author's name or company name
    in advertising or publicity relating to this software or products derived from
    it.  This software may be referred to only as "the Independent JPEG Group's
    software".

    We specifically permit and encourage the use of this software as the basis of
    commercial products, provided that all warranty or liability claims are
    assumed by the product vendor.
*/

static CONST_BITS: i32 = 13;
static PASS1_BITS: i32 = 2;

static FIX_0_298631336: i32 = 2446;
static FIX_0_390180644: i32 = 3196;
static FIX_0_541196100: i32 = 4433;
static FIX_0_765366865: i32 = 6270;
static FIX_0_899976223: i32 = 7373;
static FIX_1_175875602: i32 = 9633;
static FIX_1_501321110: i32 = 12_299;
static FIX_1_847759065: i32 = 15_137;
static FIX_1_961570560: i32 = 16_069;
static FIX_2_053119869: i32 = 16_819;
static FIX_2_562915447: i32 = 20_995;
static FIX_3_072711026: i32 = 25_172;

fn fdct(samples: &[u8; 64], coeffs: &mut [i32; 64]) {
    // Pass 1: process rows.
    // Results are scaled by sqrt(8) compared to a true DCT
    // furthermore we scale the results by 2**PASS1_BITS
    for y in 0usize..8 {
        let y0 = y * 8;

        // Even part
        let t0 = i32::from(samples[y0]) + i32::from(samples[y0 + 7]);
        let t1 = i32::from(samples[y0 + 1]) + i32::from(samples[y0 + 6]);
        let t2 = i32::from(samples[y0 + 2]) + i32::from(samples[y0 + 5]);
        let t3 = i32::from(samples[y0 + 3]) + i32::from(samples[y0 + 4]);

        let t10 = t0 + t3;
        let t12 = t0 - t3;
        let t11 = t1 + t2;
        let t13 = t1 - t2;

        let t0 = i32::from(samples[y0]) - i32::from(samples[y0 + 7]);
        let t1 = i32::from(samples[y0 + 1]) - i32::from(samples[y0 + 6]);
        let t2 = i32::from(samples[y0 + 2]) - i32::from(samples[y0 + 5]);
        let t3 = i32::from(samples[y0 + 3]) - i32::from(samples[y0 + 4]);

        // Apply unsigned -> signed conversion
        coeffs[y0] = (t10 + t11 - 8 * 128) << PASS1_BITS as usize;
        coeffs[y0 + 4] = (t10 - t11) << PASS1_BITS as usize;

        let mut z1 = (t12 + t13) * FIX_0_541196100;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        coeffs[y0 + 2] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 6] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS - PASS1_BITS) as usize;

        // Odd part
        let t12 = t0 + t2;
        let t13 = t1 + t3;

        let mut z1 = (t12 + t13) * FIX_1_175875602;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        let mut t12 = t12 * (-FIX_0_390180644);
        let mut t13 = t13 * (-FIX_1_961570560);
        t12 += z1;
        t13 += z1;

        let z1 = (t0 + t3) * (-FIX_0_899976223);
        let mut t0 = t0 * FIX_1_501321110;
        let mut t3 = t3 * FIX_0_298631336;
        t0 += z1 + t12;
        t3 += z1 + t13;

        let z1 = (t1 + t2) * (-FIX_2_562915447);
        let mut t1 = t1 * FIX_3_072711026;
        let mut t2 = t2 * FIX_2_053119869;
        t1 += z1 + t13;
        t2 += z1 + t12;

        coeffs[y0 + 1] = t0 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 3] = t1 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 5] = t2 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 7] = t3 >> (CONST_BITS - PASS1_BITS) as usize;
    }

    // Pass 2: process columns
    // We remove the PASS1_BITS scaling but leave the results scaled up an
    // overall factor of 8
    for x in (0usize..8).rev() {
        // Even part
        let t0 = coeffs[x] + coeffs[x + 8 * 7];
        let t1 = coeffs[x + 8] + coeffs[x + 8 * 6];
        let t2 = coeffs[x + 8 * 2] + coeffs[x + 8 * 5];
        let t3 = coeffs[x + 8 * 3] + coeffs[x + 8 * 4];

        // Add fudge factor here for final descale
        let t10 = t0 + t3 + (1 << (PASS1_BITS - 1) as usize);
        let t12 = t0 - t3;
        let t11 = t1 + t2;
        let t13 = t1 - t2;

        let t0 = coeffs[x] - coeffs[x + 8 * 7];
        let t1 = coeffs[x + 8] - coeffs[x + 8 * 6];
        let t2 = coeffs[x + 8 * 2] - coeffs[x + 8 * 5];
        let t3 = coeffs[x + 8 * 3] - coeffs[x + 8 * 4];

        coeffs[x] = (t10 + t11) >> PASS1_BITS as usize;
        coeffs[x + 8 * 4] = (t10 - t11) >> PASS1_BITS as usize;

        let mut z1 = (t12 + t13) * FIX_0_541196100;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS + PASS1_BITS - 1) as usize;

        coeffs[x + 8 * 2] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 6] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS + PASS1_BITS) as usize;

        // Odd part
        let t12 = t0 + t2;
        let t13 = t1 + t3;

        let mut z1 = (t12 + t13) * FIX_1_175875602;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        let mut t12 = t12 * (-FIX_0_390180644);
        let mut t13 = t13 * (-FIX_1_961570560);
        t12 += z1;
        t13 += z1;

        let z1 = (t0 + t3) * (-FIX_0_899976223);
        let mut t0 = t0 * FIX_1_501321110;
        let mut t3 = t3 * FIX_0_298631336;
        t0 += z1 + t12;
        t3 += z1 + t13;

        let z1 = (t1 + t2) * (-FIX_2_562915447);
        let mut t1 = t1 * FIX_3_072711026;
        let mut t2 = t2 * FIX_2_053119869;
        t1 += z1 + t13;
        t2 += z1 + t12;

        coeffs[x + 8] = t0 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 3] = t1 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 5] = t2 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 7] = t3 >> (CONST_BITS + PASS1_BITS) as usize;
    }
}
