//! The per-connection HTTP/2 GREASE setting generator (Chrome 145 trait).

use diting_wire_proto::conn::http2::grease_setting;

#[test]
fn ids_have_the_0x_a_a_shape_and_vary_across_connections() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..200 {
        let (id, _) = grease_setting();
        assert_eq!(id & 0x0F0F, 0x0A0A, "{id:#06x} must look like 0x?A?A");
        ids.insert(id);
    }
    assert!(ids.len() > 20, "200 draws gave only {} distinct ids", ids.len());
}

#[test]
fn values_use_the_full_u32_range() {
    let big = (0..200).filter(|_| grease_setting().1 > u32::MAX / 2).count();
    assert!(big > 40 && big < 160, "values look biased: {big}/200 above half range");
}
