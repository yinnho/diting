//! Experimental (GREASE) HTTP/2 settings on the wire.
//!
//! Real Chrome 145 appends one setting with an id of the form 0x?A?A (far above 15) after the
//! regular ones. The original `http2` capped experimental ids at 15 and could not send it.

use bytes::BytesMut;
use diting_wire_h2::frame::{ExperimentalSettings, Setting, SettingId, Settings};

fn encode(s: &Settings) -> BytesMut {
    let mut b = BytesMut::new();
    s.encode(&mut b);
    b
}

/// (id, value) pairs of a SETTINGS frame payload, after the 9-byte frame head.
fn pairs(frame: &[u8]) -> Vec<(u16, u32)> {
    frame[9..]
        .chunks(6)
        .map(|c| (u16::from_be_bytes([c[0], c[1]]), u32::from_be_bytes([c[2], c[3], c[4], c[5]])))
        .collect()
}

#[test]
fn grease_style_id_is_sent_last_and_counted_in_the_length() {
    let mut s = Settings::default();
    s.set_initial_window_size(Some(6291456));
    s.set_max_header_list_size(Some(262144));
    let grease = Setting::from_id(SettingId::Unknown(0x7A3A), 0xDEAD_BEEF).expect("ids above 15 are allowed");
    s.set_experimental_settings(ExperimentalSettings::builder().push(grease).build());

    let frame = encode(&s);
    let len = u32::from_be_bytes([0, frame[0], frame[1], frame[2]]) as usize;
    assert_eq!(len, frame.len() - 9, "payload length field must include the experimental setting");
    let p = pairs(&frame);
    assert_eq!(p.last(), Some(&(0x7A3A, 0xDEAD_BEEF)), "the GREASE setting goes LAST: {p:x?}");
    assert!(p.contains(&(4, 6291456)) && p.contains(&(6, 262144)), "regular settings still sent: {p:x?}");
    assert_eq!(p.iter().filter(|(id, _)| *id == 0x7A3A).count(), 1);
}

#[test]
fn settings_without_experimental_ones_are_unchanged() {
    let mut s = Settings::default();
    s.set_initial_window_size(Some(65535));
    let p = pairs(&encode(&s));
    assert_eq!(p, vec![(4, 65535)]);
}

#[test]
fn builder_keeps_the_first_of_a_duplicate_id_and_rejects_id_zero() {
    let one = ExperimentalSettings::builder()
        .push(Setting::from_id(SettingId::Unknown(0xCA0A), 1))
        .push(Setting::from_id(SettingId::Unknown(0xCA0A), 2))
        .build();
    let mut s = Settings::default();
    s.set_experimental_settings(one);
    assert_eq!(pairs(&encode(&s)), vec![(0xCA0A, 1)]);
    assert!(Setting::from_id(SettingId::Unknown(0), 7).is_none());
}
