//! Geometry interfaces, declared (not coded): DOMRect*, DOMPoint*.
//! Shapes are pinned against Chrome by tools/webplatform/chrome/
//! native-pilot-probe.js (same probe runs in Chrome and here).

use super::declare::{Attr, Iface, Member, Slot};

const fn slot(name: &'static str, default: f64) -> Slot { Slot { name, default } }
const fn ro(name: &'static str, attr: Attr) -> Member { Member { name, attr, writable: false } }
const fn rw(name: &'static str, i: usize) -> Member { Member { name, attr: Attr::Slot(i), writable: true } }

fn fmin(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) } }
fn fmax(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) } }

const RECT_SLOTS: &[Slot] = &[slot("x", 0.0), slot("y", 0.0), slot("width", 0.0), slot("height", 0.0)];
const POINT_SLOTS: &[Slot] = &[slot("x", 0.0), slot("y", 0.0), slot("z", 0.0), slot("w", 1.0)];

pub(crate) static IFACES: &[Iface] = &[
    Iface {
        name: "DOMRectReadOnly", parent: None, family: 0x0D01, slots: RECT_SLOTS,
        members: &[
            ro("x", Attr::Slot(0)), ro("y", Attr::Slot(1)), ro("width", Attr::Slot(2)), ro("height", Attr::Slot(3)),
            ro("top", Attr::Derived(|v| fmin(v[1], v[1] + v[3]))),
            ro("right", Attr::Derived(|v| fmax(v[0], v[0] + v[2]))),
            ro("bottom", Attr::Derived(|v| fmax(v[1], v[1] + v[3]))),
            ro("left", Attr::Derived(|v| fmin(v[0], v[0] + v[2]))),
        ],
        to_json: true, factory: "fromRect",
    },
    Iface {
        name: "DOMRect", parent: Some(0), family: 0x0D01, slots: RECT_SLOTS,
        members: &[rw("x", 0), rw("y", 1), rw("width", 2), rw("height", 3)],
        to_json: false, factory: "fromRect",
    },
    Iface {
        name: "DOMPointReadOnly", parent: None, family: 0x0D02, slots: POINT_SLOTS,
        members: &[ro("x", Attr::Slot(0)), ro("y", Attr::Slot(1)), ro("z", Attr::Slot(2)), ro("w", Attr::Slot(3))],
        to_json: true, factory: "fromPoint",
    },
    Iface {
        name: "DOMPoint", parent: Some(2), family: 0x0D02, slots: POINT_SLOTS,
        members: &[rw("x", 0), rw("y", 1), rw("z", 2), rw("w", 3)],
        to_json: false, factory: "fromPoint",
    },
];
