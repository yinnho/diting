//! Compression Streams (DecompressionStream) 的原生解压腿。
//! 无状态一次性 inflate：JS 类按 TransformStream 攒块、flush 时一次解完
//! （模块加载器形状——整块压缩资源消费完才读文本）。真有流式消费者再
//! 开状态化 create/write/flush 三 op。

use diting_ops::diting_op;
use diting_runtime::state::OpState;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use std::io::Read as _;

/// 输出上限：gzip 炸弹膨胀比上千，64MiB 兜底——宁可这条流失败，不让一个
/// 恶意压缩包吃光 isolate 堆（heap_guard 会救，但流语义应该是失败不是拖死）。
const MAX_INFLATE_OUT: u64 = 64 * 1024 * 1024;

/// `op_inflate(format, data_b64) -> b64`：format = gzip / deflate /
/// deflate-raw（Compression Streams 规范的三个枚举值）；任何一步失败
/// （坏 base64、坏格式名、解压错误、超上限）返回空串，由 JS 侧转成
/// TypeError——panic 永不过 FFI 边界（op_dom 同款 catch_unwind 先例）。
#[diting_op]
#[string]
pub(super) fn op_inflate(
    state: &OpState,
    #[string] format: String,
    #[string] data_b64: String,
) -> String {
    let _ = state;
    std::panic::catch_unwind(|| {
        use base64::Engine as _;
        let raw = match base64::engine::general_purpose::STANDARD.decode(data_b64) {
            Ok(v) => v,
            Err(_) => return String::new(),
        };
        let mut out = Vec::new();
        let read = match format.as_str() {
            "gzip" => GzDecoder::new(&raw[..])
                .take(MAX_INFLATE_OUT + 1)
                .read_to_end(&mut out),
            "deflate" => ZlibDecoder::new(&raw[..])
                .take(MAX_INFLATE_OUT + 1)
                .read_to_end(&mut out),
            "deflate-raw" => DeflateDecoder::new(&raw[..])
                .take(MAX_INFLATE_OUT + 1)
                .read_to_end(&mut out),
            _ => return String::new(),
        };
        match read {
            Ok(n) if (n as u64) <= MAX_INFLATE_OUT => {
                base64::engine::general_purpose::STANDARD.encode(&out)
            }
            _ => String::new(),
        }
    })
    .unwrap_or_default()
}
