//! Font-metrics truth tables served outside the V8 heap.
//!
//! Artifact v2 keeps the lazy `font_table` contract from the raw v1 format but
//! changes storage: top-level entries are packed into blocks, each block is an
//! independent zstd frame, and the frames are concatenated in
//! `js/font_metrics_data.zst`. `font_metrics_index.json` maps table/key to
//! `{block, offset, len}` and pins both the compressed blob hash and a semantic
//! payload digest. The runtime decodes only the block containing a requested
//! entry and keeps a bounded block cache, so blank realms still do not pay the
//! old per-realm heap cost and the repository/binary no longer embed raw JSON.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

mod data {
    include!(concat!(env!("OUT_DIR"), "/font_data.rs"));
}

const BLOCK_CACHE_CAPACITY: usize = 64;

#[derive(Debug, Deserialize)]
struct IndexFile {
    format: String,
    codec: String,
    data_size: usize,
    data_sha256: String,
    semantic_sha256: String,
    blocks: Vec<BlockMeta>,
    tables: HashMap<String, HashMap<String, EntryMeta>>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct BlockMeta {
    off: usize,
    clen: usize,
    ulen: usize,
}

#[derive(Debug, Deserialize)]
struct EntryMeta {
    b: usize,
    o: usize,
    n: usize,
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}

fn index() -> &'static IndexFile {
    static INDEX: OnceLock<IndexFile> = OnceLock::new();
    INDEX.get_or_init(|| {
        let ix: IndexFile = serde_json::from_str(include_str!("../js/font_metrics_index.json"))
            .expect("font_metrics_index.json is generated v2 JSON");
        assert_eq!(ix.format, "font-metrics-index-v2");
        assert_eq!(ix.codec, "zstd-frame-concat-v1");
        assert_eq!(
            ix.data_size,
            data::FONT_DATA.len(),
            "font blob/index size mismatch"
        );
        let digest = Sha256::digest(data::FONT_DATA);
        assert_eq!(
            hex(&digest),
            ix.data_sha256,
            "font blob/index sha256 mismatch"
        );
        assert_eq!(ix.semantic_sha256.len(), 64);
        ix
    })
}

struct BlockCache {
    blocks: HashMap<usize, Arc<Vec<u8>>>,
    order: VecDeque<usize>,
}

fn cache() -> &'static Mutex<BlockCache> {
    static CACHE: OnceLock<Mutex<BlockCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(BlockCache {
            blocks: HashMap::new(),
            order: VecDeque::new(),
        })
    })
}

fn decoded_block(block_index: usize) -> Option<Arc<Vec<u8>>> {
    if let Some(hit) = cache().lock().ok()?.blocks.get(&block_index).cloned() {
        return Some(hit);
    }

    let meta = *index().blocks.get(block_index)?;
    let frame = data::FONT_DATA.get(meta.off..meta.off + meta.clen)?;
    let decoded = zstd::stream::decode_all(frame).ok()?;
    if decoded.len() != meta.ulen {
        return None;
    }
    let decoded = Arc::new(decoded);

    if let Ok(mut cache) = cache().lock() {
        while cache.blocks.len() >= BLOCK_CACHE_CAPACITY {
            let Some(old) = cache.order.pop_front() else {
                break;
            };
            cache.blocks.remove(&old);
        }
        cache.blocks.insert(block_index, Arc::clone(&decoded));
        cache.order.push_back(block_index);
    }
    Some(decoded)
}

/// The raw JSON text of `table[key]`, or `""` when there is no such entry.
pub(crate) fn lookup(table: &str, key: &str) -> String {
    let Some(entry) = index().tables.get(table).and_then(|t| t.get(key)) else {
        return String::new();
    };
    let Some(block) = decoded_block(entry.b) else {
        return String::new();
    };
    block
        .get(entry.o..entry.o + entry.n)
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{index, lookup};

    #[test]
    fn serves_entries_and_misses() {
        let absent = lookup("NAMES", "__absent__");
        assert!(absent.starts_with('{'), "{absent:.40}");
        assert!(serde_json::from_str::<serde_json::Value>(&absent).is_ok());
        assert!(lookup("ADV", "0").starts_with('['));
        assert_eq!(lookup("NAMES", "no such family"), "");
        assert_eq!(lookup("NOPE", "0"), "");
    }

    #[test]
    fn v2_index_is_self_consistent() {
        let ix = index();
        assert!(!ix.blocks.is_empty());
        assert!(ix.blocks.iter().all(|b| b.ulen > 0 && b.clen > 0));
        assert!(ix
            .tables
            .values()
            .flat_map(|t| t.values())
            .all(|e| e.b < ix.blocks.len()));
    }
}
