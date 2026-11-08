//! Font-metrics truth tables served outside the V8 heap.
//!
//! `js/font_metrics.js` (generated) used to carry ~90MB of JSON literals
//! that every realm restored from the snapshot into its own heap: a blank
//! session cost ~340MB RSS for data only `canvas.measureText` reads. The
//! generator now writes the tables' top-level entries to
//! `js/font_metrics_data.tables` (raw JSON bytes) with an `[offset, len]`
//! index in `js/font_metrics_index.json`; the blob is embedded read-only
//! (one copy per process) and the JS side asks for one entry at a time
//! through the `font_table` op, keeping what it decoded per realm.

use std::collections::HashMap;
use std::sync::OnceLock;

mod data {
    include!(concat!(env!("OUT_DIR"), "/font_data.rs"));
}

type Index = HashMap<String, HashMap<String, (usize, usize)>>;

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let raw: HashMap<String, HashMap<String, [usize; 2]>> =
            serde_json::from_str(include_str!("../js/font_metrics_index.json"))
                .expect("font_metrics_index.json is generated JSON");
        raw.into_iter()
            .map(|(tbl, keys)| {
                (
                    tbl,
                    keys.into_iter().map(|(k, [o, n])| (k, (o, n))).collect(),
                )
            })
            .collect()
    })
}

/// The raw JSON text of `table[key]`, or `""` when there is no such entry.
pub(crate) fn lookup(table: &str, key: &str) -> String {
    let Some(&(off, len)) = index().get(table).and_then(|t| t.get(key)) else {
        return String::new();
    };
    data::FONT_DATA
        .get(off..off + len)
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::lookup;

    #[test]
    fn serves_entries_and_misses() {
        let absent = lookup("NAMES", "__absent__");
        assert!(absent.starts_with('{'), "{absent:.40}");
        assert!(serde_json::from_str::<serde_json::Value>(&absent).is_ok());
        assert!(lookup("ADV", "0").starts_with('['));
        assert_eq!(lookup("NAMES", "no such family"), "");
        assert_eq!(lookup("NOPE", "0"), "");
    }
}
