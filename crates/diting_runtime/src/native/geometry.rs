//! Geometry interfaces, declared (not coded): DOMRect*, DOMPoint*.
//! Shapes are pinned against Chrome by tools/webplatform/chrome/
//! native-pilot-probe.js (same probe runs in Chrome and here).

use super::declare::{Attr, Def, Iface, Member, Slot, Val};

const fn slot(name: &'static str, default: f64) -> Slot { Slot { name, default: Def::Num(default) } }
const fn ro(name: &'static str, attr: Attr) -> Member { Member { name, attr, writable: false } }
const fn rw(name: &'static str, i: usize) -> Member { Member { name, attr: Attr::Slot(i), writable: true } }

fn fmin(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) } }
fn fmax(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) } }

const RECT_SLOTS: &[Slot] = &[slot("x", 0.0), slot("y", 0.0), slot("width", 0.0), slot("height", 0.0)];
const POINT_SLOTS: &[Slot] = &[slot("x", 0.0), slot("y", 0.0), slot("z", 0.0), slot("w", 1.0)];

pub(crate) static RECT_RO: Iface = Iface {
        name: "DOMRectReadOnly", parent: None, family: 0x0D01, slots: RECT_SLOTS,
        members: &[
            ro("x", Attr::Slot(0)), ro("y", Attr::Slot(1)), ro("width", Attr::Slot(2)), ro("height", Attr::Slot(3)),
            ro("top", Attr::Derived(|v| Val::Num(fmin(v[1].num(), v[1].num() + v[3].num())))),
            ro("right", Attr::Derived(|v| Val::Num(fmax(v[0].num(), v[0].num() + v[2].num())))),
            ro("bottom", Attr::Derived(|v| Val::Num(fmax(v[1].num(), v[1].num() + v[3].num())))),
            ro("left", Attr::Derived(|v| Val::Num(fmin(v[0].num(), v[0].num() + v[2].num())))),
        ],
        to_json: true, factory: Some("fromRect"), consts: &[], error_like: false,
    };

pub(crate) static RECT: Iface = Iface {
        name: "DOMRect", parent: Some(0), family: 0x0D01, slots: RECT_SLOTS,
        members: &[rw("x", 0), rw("y", 1), rw("width", 2), rw("height", 3)],
        to_json: false, factory: Some("fromRect"), consts: &[], error_like: false,
    };

pub(crate) static POINT_RO: Iface = Iface {
        name: "DOMPointReadOnly", parent: None, family: 0x0D02, slots: POINT_SLOTS,
        members: &[ro("x", Attr::Slot(0)), ro("y", Attr::Slot(1)), ro("z", Attr::Slot(2)), ro("w", Attr::Slot(3))],
        to_json: true, factory: Some("fromPoint"), consts: &[], error_like: false,
    };

pub(crate) static POINT: Iface = Iface {
        name: "DOMPoint", parent: Some(2), family: 0x0D02, slots: POINT_SLOTS,
        members: &[rw("x", 0), rw("y", 1), rw("z", 2), rw("w", 3)],
        to_json: false, factory: Some("fromPoint"), consts: &[], error_like: false,
    };

