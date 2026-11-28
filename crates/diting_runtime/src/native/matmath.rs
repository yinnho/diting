//! DOMMatrix arithmetic and the CSS transform-list parser — transliterated
//! from crates/diting_js/js/platform/geometry.impl.js, whose behavior is
//! pinned by the Chrome truth probes. Pure Rust, no v8: every function is
//! unit-testable arithmetic. Column-major storage, m_{C}{R} at 4*(C-1)+(R-1),
//! exactly like the JS version.

pub(crate) type M16 = [f64; 16];

pub(crate) fn id16() -> M16 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

pub(crate) fn is_identity16(m: &M16) -> bool {
    m.iter().enumerate().all(|(i, v)| {
        *v == if i == 0 || i == 5 || i == 10 || i == 15 {
            1.0
        } else {
            0.0
        }
    })
}

/// A·B in m_{CR} storage: C[4c+r] = Σ_k A[4k+r]·B[4c+k].
pub(crate) fn mul(a: &M16, b: &M16) -> M16 {
    let mut c = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut t = 0.0;
            for k in 0..4 {
                t += a[4 * k + row] * b[4 * col + k];
            }
            c[4 * col + row] = t;
        }
    }
    c
}

/// Exact values at right angles — Chrome's rotate(90) answers a crisp 0,
/// not cos(π/2) = 6.1e-17; 45°+90°·n rounds to SQRT2/2 (truth ro.rotate45).
pub(crate) fn cos_sin(deg: f64) -> (f64, f64) {
    if deg.is_finite() {
        let k = deg / 90.0;
        if k.fract() == 0.0 {
            let q = ((k as i64) % 4 + 4) % 4;
            return match q {
                0 => (1.0, 0.0),
                1 => (0.0, 1.0),
                2 => (-1.0, 0.0),
                _ => (0.0, -1.0),
            };
        }
        let h = deg / 45.0;
        if h.fract() == 0.0 {
            let s2 = std::f64::consts::SQRT_2 / 2.0;
            let q = ((h as i64) % 8 + 8) % 8;
            match q {
                1 => return (s2, s2),
                3 => return (-s2, s2),
                5 => return (-s2, -s2),
                7 => return (s2, -s2),
                _ => {}
            }
        }
    }
    let rad = deg * std::f64::consts::PI / 180.0;
    (rad.cos(), rad.sin())
}

const D2R: f64 = std::f64::consts::PI / 180.0;
pub(crate) const D2R_: f64 = D2R;

pub(crate) fn rot_z(deg: f64) -> M16 {
    let (c, s) = cos_sin(deg);
    let mut m = id16();
    m[0] = c;
    m[1] = s;
    m[4] = -s;
    m[5] = c;
    m
}

pub(crate) fn rot_x(deg: f64) -> M16 {
    let (c, s) = cos_sin(deg);
    let mut m = id16();
    m[5] = c;
    m[6] = s;
    m[9] = -s;
    m[10] = c;
    m
}

pub(crate) fn rot_y(deg: f64) -> M16 {
    let (c, s) = cos_sin(deg);
    let mut m = id16();
    m[0] = c;
    m[2] = -s;
    m[8] = s;
    m[10] = c;
    m
}

pub(crate) fn axis_angle(x: f64, y: f64, z: f64, deg: f64) -> M16 {
    let h = (x * x + y * y + z * z).sqrt();
    let mut m = id16();
    if h == 0.0 {
        return m;
    }
    let (ax, ay, az) = (x / h, y / h, z / h);
    let (c, s) = cos_sin(deg);
    let t = 1.0 - c;
    m[0] = t * ax * ax + c;
    m[1] = t * ax * ay + s * az;
    m[2] = t * ax * az - s * ay;
    m[4] = t * ax * ay - s * az;
    m[5] = t * ay * ay + c;
    m[6] = t * ay * az + s * ax;
    m[8] = t * ax * az + s * ay;
    m[9] = t * ay * az - s * ax;
    m[10] = t * az * az + c;
    m
}

/// Exact M·T as a structural op: only the last column changes, so a NaN
/// factor never bleeds across the matrix (truth ro.scale.z).
pub(crate) fn post_translate(st: &mut M16, tx: f64, ty: f64, tz: f64) {
    for r in 0..4 {
        st[12 + r] += st[r] * tx + st[4 + r] * ty + st[8 + r] * tz;
    }
}

/// Generic 4x4 inverse (adjugate); singular/NaN -> None.
pub(crate) fn inverse16(m: &M16) -> Option<M16> {
    let idx = |r: usize, c: usize| 4 * c + r;
    let sub3 = |rows: [usize; 3], cols: [usize; 3]| {
        let s = |rr: usize, cc: usize| m[idx(rows[rr], cols[cc])];
        s(0, 0) * (s(1, 1) * s(2, 2) - s(1, 2) * s(2, 1))
            - s(0, 1) * (s(1, 0) * s(2, 2) - s(1, 2) * s(2, 0))
            + s(0, 2) * (s(1, 0) * s(2, 1) - s(1, 1) * s(2, 0))
    };
    let mut det = 0.0;
    for c in 0..4 {
        let rest: Vec<usize> = (0..4).filter(|&x| x != c).collect();
        det += if c % 2 == 0 { 1.0 } else { -1.0 }
            * m[idx(0, c)]
            * sub3([1, 2, 3], [rest[0], rest[1], rest[2]]);
    }
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let mut out = [0.0; 16];
    for r in 0..4 {
        for c in 0..4 {
            let rows: Vec<usize> = (0..4).filter(|&x| x != r).collect();
            let cols: Vec<usize> = (0..4).filter(|&x| x != c).collect();
            let cof = sub3([rows[0], rows[1], rows[2]], [cols[0], cols[1], cols[2]]);
            out[idx(c, r)] = if (r + c) % 2 == 0 { cof } else { -cof } / det;
        }
    }
    Some(out)
}

/// x' = m11x+m21y+m31z+m41w ... — no division by w (truth dp.mt).
pub(crate) fn transform4(m: &M16, p: [f64; 4]) -> [f64; 4] {
    let [x, y, z, w] = p;
    [
        m[0] * x + m[4] * y + m[8] * z + m[12] * w,
        m[1] * x + m[5] * y + m[9] * z + m[13] * w,
        m[2] * x + m[6] * y + m[10] * z + m[14] * w,
        m[3] * x + m[7] * y + m[11] * z + m[15] * w,
    ]
}

// ---- CSS transform-list parser ------------------------------------------
// '' / 'none' -> identity; percent or font/viewport lengths -> Resolvable;
// calc() of px terms resolves; exponent notation in numbers; angle units
// deg/rad/grad/turn, unitless only for 0; trailing garbage fails the WHOLE
// original string.

#[derive(Debug)]
pub(crate) enum ParseErr {
    /// DOMException SyntaxError "Values must be resolvable at parse time"
    /// (NO trailing period — truth smv.percent).
    Resolvable,
    /// DOMException SyntaxError "Failed to parse '<s>'."
    Failed,
}

pub(crate) struct Parsed {
    pub m: M16,
    pub is2d: bool,
}

pub(crate) fn parse_transform_list(s: &str) -> Result<Parsed, ParseErr> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Ok(Parsed {
            m: id16(),
            is2d: true,
        });
    }
    let b = t.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut acc: Option<M16> = None;
    let mut any3d = false;

    macro_rules! ws {
        () => {
            while i < n && (b[i] as char).is_whitespace() {
                i += 1;
            }
        };
    }

    // These helpers are closures over `i`/`b` — written as a nested parser
    // struct because Rust closures can't borrow-and-mutate the same state.
    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn number(&mut self) -> Option<f64> {
            let start = self.i;
            if self.i < self.b.len() && (self.b[self.i] == b'+' || self.b[self.i] == b'-') {
                self.i += 1;
            }
            let mut digits = 0;
            while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                self.i += 1;
                digits += 1;
            }
            if self.i < self.b.len() && self.b[self.i] == b'.' {
                self.i += 1;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    self.i += 1;
                    digits += 1;
                }
            }
            if digits == 0 {
                self.i = start;
                return None;
            }
            if self.i < self.b.len() && (self.b[self.i] == b'e' || self.b[self.i] == b'E') {
                let save = self.i;
                self.i += 1;
                if self.i < self.b.len() && (self.b[self.i] == b'+' || self.b[self.i] == b'-') {
                    self.i += 1;
                }
                let mut ed = 0;
                while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
                    self.i += 1;
                    ed += 1;
                }
                if ed == 0 {
                    self.i = save;
                }
            }
            std::str::from_utf8(&self.b[start..self.i])
                .ok()?
                .parse()
                .ok()
        }
        fn unit(&mut self) -> String {
            let start = self.i;
            while self.i < self.b.len()
                && (self.b[self.i].is_ascii_alphabetic() || self.b[self.i] == b'%')
            {
                self.i += 1;
            }
            String::from_utf8_lossy(&self.b[start..self.i]).into_owned()
        }
        fn at_calc(&self) -> bool {
            self.b.len() - self.i >= 5 && self.b[self.i..self.i + 5].eq_ignore_ascii_case(b"calc(")
        }
        fn length_value(&mut self) -> Result<Option<f64>, ParseErr> {
            if self.at_calc() {
                return self.calc_value().map(Some);
            }
            let Some(v) = self.number() else {
                return Ok(None);
            };
            let u = self.unit();
            if u.is_empty() {
                return Ok(if v == 0.0 { Some(0.0) } else { None });
            }
            if u == "px" {
                return Ok(Some(v));
            }
            Err(ParseErr::Resolvable)
        }
        fn angle_deg(&mut self) -> Result<Option<f64>, ParseErr> {
            if self.at_calc() {
                return self.calc_value().map(Some);
            }
            let Some(v) = self.number() else {
                return Ok(None);
            };
            let u = self.unit().to_lowercase();
            if u.is_empty() {
                return Ok(if v == 0.0 { Some(0.0) } else { None });
            }
            Ok(match u.as_str() {
                "deg" => Some(v),
                "rad" => Some(v / D2R),
                "grad" => Some(v * 0.9),
                "turn" => Some(v * 360.0),
                _ => None,
            })
        }
        fn plain_number(&mut self) -> Result<Option<f64>, ParseErr> {
            if self.at_calc() {
                return self.calc_value().map(Some);
            }
            let Some(v) = self.number() else {
                return Ok(None);
            };
            if self.unit().is_empty() {
                Ok(Some(v))
            } else {
                Ok(None)
            }
        }
        fn calc_value(&mut self) -> Result<f64, ParseErr> {
            self.i += 5; // past 'calc('
            let val = self.calc_expr()?;
            self.skip_ws();
            if self.i >= self.b.len() || self.b[self.i] != b')' {
                return Err(ParseErr::Failed);
            }
            self.i += 1;
            match val {
                Some(v) => Ok(v),
                None => Err(ParseErr::Failed),
            }
        }
        fn skip_ws(&mut self) {
            while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
                self.i += 1;
            }
        }
        fn calc_expr(&mut self) -> Result<Option<f64>, ParseErr> {
            let mut v = self.calc_term()?;
            loop {
                self.skip_ws();
                let op = if self.i < self.b.len() {
                    self.b[self.i]
                } else {
                    0
                };
                if op == b'+' || op == b'-' {
                    self.i += 1;
                    let r = self.calc_term()?;
                    v = match (v, r) {
                        (Some(a), Some(b)) => Some(if op == b'+' { a + b } else { a - b }),
                        _ => None,
                    };
                } else {
                    return Ok(v);
                }
            }
        }
        fn calc_term(&mut self) -> Result<Option<f64>, ParseErr> {
            self.skip_ws();
            let mut v = self.calc_prim()?;
            loop {
                self.skip_ws();
                let op = if self.i < self.b.len() {
                    self.b[self.i]
                } else {
                    0
                };
                if op == b'*' || op == b'/' {
                    self.i += 1;
                    let r = self.calc_prim()?;
                    v = match (v, r) {
                        (Some(a), Some(b)) => Some(if op == b'*' { a * b } else { a / b }),
                        _ => None,
                    };
                } else {
                    return Ok(v);
                }
            }
        }
        fn calc_prim(&mut self) -> Result<Option<f64>, ParseErr> {
            if self.i < self.b.len() && self.b[self.i] == b'(' {
                self.i += 1;
                let v = self.calc_expr()?;
                self.skip_ws();
                if self.i >= self.b.len() || self.b[self.i] != b')' {
                    return Ok(None);
                }
                self.i += 1;
                return Ok(v);
            }
            let Some(v) = self.number() else {
                return Ok(None);
            };
            let u = self.unit();
            if u.is_empty() || u == "px" {
                return Ok(Some(v));
            }
            Err(ParseErr::Resolvable)
        }
    }

    fn kind_of(name: &str) -> Option<char> {
        Some(match name {
            "matrix" | "matrix3d" | "scale" | "scalex" | "scaley" | "scalez" | "scale3d" => 'n',
            "rotate" | "rotatex" | "rotatey" | "rotatez" | "skew" | "skewx" | "skewy" => 'a',
            "translate" | "translatex" | "translatey" | "translatez" | "translate3d"
            | "perspective" => 'l',
            "rotate3d" => 'r', // first three args numbers, fourth angle
            _ => return None,
        })
    }
    fn force3d(name: &str) -> bool {
        matches!(
            name,
            "matrix3d"
                | "translate3d"
                | "translatez"
                | "scale3d"
                | "scalez"
                | "rotate3d"
                | "rotatex"
                | "rotatey"
                | "rotatez"
                | "perspective"
        )
    }

    loop {
        ws!();
        if i >= n {
            break;
        }
        let start = i;
        while i < n && b[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let name = std::str::from_utf8(&b[start..i])
            .unwrap_or("")
            .to_lowercase();
        ws!();
        if name.is_empty() || i >= n || b[i] != b'(' {
            return Err(ParseErr::Failed);
        }
        i += 1;
        let Some(kind) = kind_of(&name) else {
            return Err(ParseErr::Failed);
        };

        // Parse this function's arguments.
        let mut p = P { b, i };
        let mut args: Vec<f64> = Vec::new();
        let mut bad = false;
        let fm: Option<M16> = loop {
            p.skip_ws();
            let k = if name == "rotate3d" {
                if args.len() == 3 {
                    'a'
                } else {
                    'n'
                }
            } else {
                kind
            };
            let v = match k {
                'n' => p.plain_number(),
                'a' => p.angle_deg(),
                _ => p.length_value(),
            };
            let v = match v {
                Ok(Some(v)) => v,
                Ok(None) => {
                    bad = true;
                    break None;
                }
                Err(e) => return Err(e),
            };
            args.push(v);
            p.skip_ws();
            if p.i < n && p.b[p.i] == b',' {
                p.i += 1;
                continue;
            }
            if p.i >= n || p.b[p.i] != b')' {
                bad = true;
                break None;
            }
            p.i += 1;
            let mut m = id16();
            let r = args.as_slice();
            let built: Option<M16> = match name.as_str() {
                "matrix" if r.len() == 6 => {
                    m[0] = r[0];
                    m[1] = r[1];
                    m[4] = r[2];
                    m[5] = r[3];
                    m[12] = r[4];
                    m[13] = r[5];
                    Some(m)
                }
                "matrix3d" if r.len() == 16 => {
                    m.copy_from_slice(&args[..16]);
                    Some(m)
                }
                "translate" if (1..=2).contains(&r.len()) => {
                    m[12] = r[0];
                    m[13] = if r.len() > 1 { r[1] } else { 0.0 };
                    Some(m)
                }
                "translatex" if r.len() == 1 => {
                    m[12] = r[0];
                    Some(m)
                }
                "translatey" if r.len() == 1 => {
                    m[13] = r[0];
                    Some(m)
                }
                "translatez" if r.len() == 1 => {
                    m[14] = r[0];
                    Some(m)
                }
                "translate3d" if r.len() == 3 => {
                    m[12] = r[0];
                    m[13] = r[1];
                    m[14] = r[2];
                    Some(m)
                }
                "scale" if (1..=3).contains(&r.len()) => {
                    m[0] = r[0];
                    m[5] = if r.len() > 1 { r[1] } else { r[0] };
                    m[10] = if r.len() > 2 { r[2] } else { 1.0 };
                    Some(m)
                }
                "scalex" if r.len() == 1 => {
                    m[0] = r[0];
                    Some(m)
                }
                "scaley" if r.len() == 1 => {
                    m[5] = r[0];
                    Some(m)
                }
                "scalez" if r.len() == 1 => {
                    m[10] = r[0];
                    Some(m)
                }
                "scale3d" if r.len() == 3 => {
                    m[0] = r[0];
                    m[5] = r[1];
                    m[10] = r[2];
                    Some(m)
                }
                "rotate" | "rotatez" if r.len() == 1 => Some(rot_z(r[0])),
                "rotatex" if r.len() == 1 => Some(rot_x(r[0])),
                "rotatey" if r.len() == 1 => Some(rot_y(r[0])),
                "rotate3d" if r.len() == 4 => Some(axis_angle(r[0], r[1], r[2], r[3])),
                "skewx" if r.len() == 1 => {
                    m[4] = (r[0] * D2R).tan();
                    Some(m)
                }
                "skewy" if r.len() == 1 => {
                    m[1] = (r[0] * D2R).tan();
                    Some(m)
                }
                "skew" if (1..=2).contains(&r.len()) => {
                    m[4] = (r[0] * D2R).tan();
                    if r.len() > 1 {
                        m[1] = (r[1] * D2R).tan();
                    }
                    Some(m)
                }
                "perspective" if r.len() == 1 => {
                    if r[0] != 0.0 {
                        m[11] = -1.0 / r[0];
                    }
                    Some(m)
                }
                _ => None,
            };
            break built;
        };
        i = p.i;
        if bad {
            return Err(ParseErr::Failed);
        }
        let Some(fm) = fm else {
            return Err(ParseErr::Failed);
        };
        if force3d(&name) {
            any3d = true;
        }
        acc = Some(match acc {
            Some(a) => mul(&a, &fm),
            None => fm,
        });
    }
    let Some(m) = acc else {
        return Err(ParseErr::Failed);
    };
    Ok(Parsed { m, is2d: !any3d })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mul() {
        let a = id16();
        assert!(is_identity16(&mul(&a, &a)));
    }

    #[test]
    fn rotate_exact_right_angles() {
        let m = rot_z(90.0);
        assert_eq!(m[0], 0.0);
        assert_eq!(m[1], 1.0);
        assert_eq!(m[4], -1.0);
        assert_eq!(m[5], 0.0);
        let m45 = rot_z(45.0);
        let s2 = std::f64::consts::SQRT_2 / 2.0;
        assert_eq!(m45[0], s2);
        assert_eq!(m45[1], s2);
    }

    #[test]
    fn inverse_roundtrip() {
        let mut m = id16();
        post_translate(&mut m, 3.0, 4.0, 5.0);
        m[0] = 2.0;
        m[5] = 3.0;
        m[10] = 4.0;
        let inv = inverse16(&m).expect("invertible");
        let prod = mul(&m, &inv);
        for (i, v) in prod.iter().enumerate() {
            let want = if i == 0 || i == 5 || i == 10 || i == 15 {
                1.0
            } else {
                0.0
            };
            assert!((v - want).abs() < 1e-9, "idx {i}: {v} vs {want}");
        }
    }

    #[test]
    fn inverse_singular_is_none() {
        let m = [0.0; 16];
        assert!(inverse16(&m).is_none());
    }

    #[test]
    fn parse_none_and_empty() {
        assert!(is_identity16(&parse_transform_list("").unwrap().m));
        assert!(is_identity16(&parse_transform_list("none").unwrap().m));
        assert!(parse_transform_list(" none ").unwrap().is2d);
    }

    #[test]
    fn parse_translate() {
        let p = parse_transform_list("translate(10px, 20px)").unwrap();
        assert_eq!(p.m[12], 10.0);
        assert_eq!(p.m[13], 20.0);
        assert!(p.is2d);
    }

    #[test]
    fn parse_percent_unresolvable() {
        assert!(matches!(
            parse_transform_list("translate(50%)"),
            Err(ParseErr::Resolvable)
        ));
    }

    #[test]
    fn parse_calc_px_resolves() {
        let p = parse_transform_list("translate(calc(10px + 5px))").unwrap();
        assert_eq!(p.m[12], 15.0);
    }

    #[test]
    fn parse_rotate3d_is_3d() {
        let p = parse_transform_list("rotate3d(0, 0, 1, 45deg)").unwrap();
        assert!(!p.is2d);
    }

    #[test]
    fn parse_garbage_fails() {
        assert!(matches!(
            parse_transform_list("translate(10px) junk"),
            Err(ParseErr::Failed)
        ));
        assert!(matches!(
            parse_transform_list("unknown(1)"),
            Err(ParseErr::Failed)
        ));
    }

    #[test]
    fn parse_exponent_numbers() {
        let p = parse_transform_list("scale(1e1)").unwrap();
        assert_eq!(p.m[0], 10.0);
    }

    #[test]
    fn post_translate_exact() {
        let mut m = id16();
        m[0] = f64::NAN;
        post_translate(&mut m, 2.0, 3.0, 0.0);
        // NaN in col0 must not bleed: only the last column changes.
        assert!(m[0].is_nan());
        assert!(m[12].is_nan()); // 0*2? no — m[0] is NaN so NaN*2 bleeds into m12
        assert_eq!(m[5], 1.0);
    }
}
