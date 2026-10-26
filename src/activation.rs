//! DupHub 客户端 —— diting 作为 dup 市场消费方的真实协议层（D18 批次3）。
//!
//! 对接 hub（aginx-hub）的真端点，不再有自造协议（批次2前的 7 端点草案
//! 已废弃——DUPHUB.md §八.7 挂账销账）：
//!   GET  /api/price?name=                    → {price_cents} | 404
//!   GET  /api/templates/{name}/dup/manifest  → {files:{path:sha256}, hash}
//!   GET  /api/templates/{name}/dup/file/{path} → 字节
//!   GET  /api/quota（Bearer）                 → {balance_cents}
//!   POST /api/orders/topup（Bearer）          → {order_id, qr_svg, expires_at}
//!   GET  /api/orders/{order_no}（Bearer）     → {status}
//!   POST /api/templates/{name}/dup/push（Bearer, files b64）→ 上架（review 队列）
//!
//! 认证 = hub 的 oc_ api_key（DUPHUB.md §五c：码即钱包）。存本地
//! ~/.diting/activation.json（0600）。agent 的完整闭环（D19 成果计费）：
//!   查价（目录探测）→ 拉取免费 → 任务 arm（余额不足：topup 出二维码经
//!   /open qr 模板上屏 → 主人扫码 → 轮询 paid）→ 干活 → settle 按成功数
//!   内存执行（不落盘）→ 结果回报。
//! 运行时自检（dump 出的付费流副本在任何地方执行被拒）保留。

use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_HUB: &str = "https://duphub.com";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
const STORE_FILE: &str = "activation.json";

/// 本地激活：hub api key（码=钱包=身份）+ hub 地址。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Activation {
    pub code: String,
    #[serde(default = "default_hub")]
    pub hub_url: String,
}

fn default_hub() -> String {
    std::env::var("DITING_DUPHUB_URL").unwrap_or_else(|_| DEFAULT_HUB.to_string())
}

fn store_path() -> PathBuf {
    crate::config::app_data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(STORE_FILE)
}

/// 本地已激活的 hub 凭证。
pub fn load() -> Option<Activation> {
    let text = std::fs::read_to_string(store_path()).ok()?;
    serde_json::from_str(&text).ok()
}

fn store(a: &Activation) -> std::io::Result<()> {
    let path = store_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(a)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .expect("reqwest client")
}

async fn get_json(a: &Activation, path: &str) -> Result<serde_json::Value, HubError> {
    let url = format!("{}{}", a.hub_url.trim_end_matches('/'), path);
    let resp = client()
        .get(&url)
        .bearer_auth(&a.code)
        .send()
        .await
        .map_err(|e| HubError::Network(e.to_string()))?;
    decode(resp).await
}

async fn post_json(
    a: &Activation,
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, HubError> {
    let url = format!("{}{}", a.hub_url.trim_end_matches('/'), path);
    let resp = client()
        .post(url)
        .bearer_auth(&a.code)
        .json(body)
        .send()
        .await
        .map_err(|e| HubError::Network(e.to_string()))?;
    decode(resp).await
}

async fn decode(resp: reqwest::Response) -> Result<serde_json::Value, HubError> {
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
    if !status.is_success() {
        let msg = parsed
            .as_ref()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| format!("hub returned {status}"));
        return Err(HubError::Status(status.as_u16(), msg));
    }
    parsed.ok_or_else(|| HubError::Network("hub response not JSON".into()))
}

/// hub 交互错误：402/404 带服务器的结构化消息（agent 可读下一步）。
#[derive(Debug)]
pub enum HubError {
    Network(String),
    /// (http_status, server_error_message)
    Status(u16, String),
    NotActivated,
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HubError::Network(e) => write!(f, "duphub unreachable: {e}"),
            HubError::Status(code, msg) => write!(f, "duphub {code}: {msg}"),
            HubError::NotActivated => write!(
                f,
                "no activation code configured — POST /activation/activate {{\"code\":\"oc_...\"}} first"
            ),
        }
    }
}

/// 激活：key 即码（D18 立法：注册/登录在 agent 端，key 由 hub 的
/// /api/auth/keys 铸出，主人从 duphub.com Keys 页复制给 agent）。
/// 这里做连通验证（quota 探针）+ 本地持久化。
pub async fn activate(code: &str, hub_url: Option<String>) -> Result<serde_json::Value, String> {
    let key = code.trim().to_string();
    if !key.starts_with("oc_") {
        return Err("activation code is a hub api key (starts with oc_...)".into());
    }
    let a = Activation {
        code: key,
        hub_url: hub_url.unwrap_or_else(default_hub),
    };
    let v = get_json(&a, "/api/quota").await.map_err(|e| e.to_string())?;
    store(&a).map_err(|e| format!("cannot persist activation: {e}"))?;
    Ok(v)
}

/// 钱包余额。
pub async fn quota() -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    get_json(&a, "/api/quota").await
}

/// 目录价格：Some(0)=在目录且免费；Some(n)=付费 n 分；None=不在目录
/// （用户自己的 flow，本地免费执行）。网络故障是 Err（不是"免费"）。
pub async fn price(flow_name: &str) -> Result<Option<i64>, HubError> {
    let Some(a) = load() else { return Ok(None) };
    match get_json(&a, &format!("/api/price?name={}", urlencode(flow_name))).await {
        Ok(v) => Ok(v.get("price_cents").and_then(|p| p.as_i64())),
        Err(HubError::Status(404, _)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 内存中的付费流（拉取自 hub，永不落盘）。Drop 即消失。
pub struct LoadedFlow {
    pub doc: serde_json::Value,
    /// 回执/日志展示用（票从 ticket 字段走）。
    #[allow(dead_code)]
    pub duphub_id: String,
    /// 运行时自检票据（内存态；dump 副本无票即拒）。
    pub ticket: String,
}

/// 市场拉取（D19：免费）：按 dup 协议 manifest→逐文件拉齐组装，全程
/// 内存。计费在任务面（/api/tasks），拉取不再过任何购买门。
pub async fn market_fetch(flow_name: &str) -> Result<LoadedFlow, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    // D19（〇之二.1）：拉取免费——买断 purchase 调用退役。计费在任务
    // 结算（task arm/settle），price_cents 参数保留为兼容形状（调用方
    // 已查过价，不再在此消费）。
    // 2. manifest → files
    let manifest = get_json(
        &a,
        &format!("/api/templates/{}/dup/manifest", urlencode(flow_name)),
    )
    .await?;
    let files = manifest
        .get("files")
        .and_then(|f| f.as_object())
        .ok_or_else(|| HubError::Network("manifest missing files".into()))?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    // D19 加密分发（〇之二.4）：know-how 文件 AES-256-GCM 密文下发，
    // per-flow key 随 manifest（激活面）到达，进程内解密——磁盘与网线
    // 上都没有明文。无 cipher 块的 hub（或匿名错路径）拉到密文时给出
    // 引导升级的错误而不是解析乱码。
    let cipher_key = manifest
        .get("cipher")
        .and_then(|c| c.get("key_hex"))
        .and_then(|k| k.as_str())
        .map(str::to_string);
    let mut flow_text: Option<String> = None;
    let mut bundled = serde_json::Map::new();
    for path in files {
        if path != "flow.json" && recipe_stem(&path).is_none() {
            continue; // 伴随件（flow.md/template.json）不进执行面
        }
        let resp = client()
            .get(format!(
                "{}/api/templates/{}/dup/file/{}",
                a.hub_url.trim_end_matches('/'),
                urlencode(flow_name),
                urlencode(&path)
            ))
            .bearer_auth(&a.code)
            .send()
            .await
            .map_err(|e| HubError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(HubError::Status(
                resp.status().as_u16(),
                format!("file {path} fetch failed"),
            ));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| HubError::Network(e.to_string()))?;
        let bytes = if bytes.starts_with(b"DITENC1") {
            let key = cipher_key.as_deref().ok_or_else(|| {
                HubError::Status(
                    409,
                    "encrypted flow package but no key in manifest — activate diting                      (or upgrade: hub packages are D19-encrypted)"
                        .into(),
                )
            })?;
            crate::activation_cipher::open(key, flow_name, &bytes)
                .map_err(|e| HubError::Network(format!("decrypt {path}: {e}")))?
        } else {
            bytes.to_vec()
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        if path == "flow.json" {
            flow_text = Some(text);
        } else if let Some(stem) = recipe_stem(&path) {
            // 配方依赖随 flow 包进内存（D18 flow 打包）：与 flow 同生命周期，
            // 不落盘；preload 步骤在内置注册表查无时从这里解析。
            bundled.insert(stem, serde_json::Value::String(text));
        }
    }
    let text = flow_text.ok_or_else(|| HubError::Network("flow package has no flow.json".into()))?;
    let mut doc: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| HubError::Network(format!("flow.json invalid: {e}")))?;
    // 3. duphub_id 标记（自检锚点）+ 原子票
    let duphub_id = format!("{flow_name}@diting");
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("duphub_id".into(), serde_json::json!(duphub_id));
        if !bundled.is_empty() {
            obj.insert("__bundled_recipes".into(), serde_json::Value::Object(bundled));
        }
    }
    let ticket = format!("{duphub_id}:live");
    Ok(LoadedFlow {
        doc,
        duphub_id,
        ticket,
    })
}

/// `recipes/<stem>.js` → Some(stem)：flow 包内的配方依赖路径。
/// stem 走与 flow 名相同的字符白名单（小写/数字/横线）。
fn recipe_stem(path: &str) -> Option<String> {
    let rest = path.strip_prefix("recipes/")?;
    let stem = rest.strip_suffix(".js")?;
    if stem.is_empty()
        || !stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return None;
    }
    Some(stem.to_string())
}

/// 充值下单：返回二维码（agent 经 /open 的 qr 面板展示给主人）。
pub async fn topup(amount_cents: i64) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    post_json(
        &a,
        "/api/orders/topup",
        &serde_json::json!({"amount_cents": amount_cents}),
    )
    .await
}

/// 订单轮询（agent 等主人扫码）。
pub async fn order_status(order_no: &str) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    get_json(&a, &format!("/api/orders/{}", urlencode(order_no))).await
}

/// 用户流上架（review 队列；分账记本机 key 名下）。
pub async fn publish(
    name: &str,
    flow: &serde_json::Value,
    price_cents: i64,
) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    use base64::Engine;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let body = serde_json::json!({
        "hash": format!("diting-publish-{stamp}"),
        "files": {
            "flow.json": base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_string(flow).unwrap_or_default()),
            "template.json": base64::engine::general_purpose::STANDARD.encode(
                serde_json::json!({
                    "version": "1",
                    "name": name,
                    "kind": "flow",
                    "display_name": name,
                    "author": "diting-agent",
                    "description": format!("published from diting (target price {price_cents} cents)"),
                })
                .to_string()
            ),
        },
        "changelog": "published from diting",
    });
    // 定价由 hub 侧审后定（先审后卖；push 不带价）
    post_json(
        &a,
        &format!("/api/templates/{}/dup/push", urlencode(name)),
        &body,
    )
    .await
}

/// 运行时自检（解释器 step 边界调用）：带 duphub_id 的流必须持票。
pub fn verify_runtime_pair(doc: &serde_json::Value, ticket: Option<&str>) -> Result<(), String> {
    let Some(id) = doc.get("duphub_id").and_then(|d| d.as_str()) else {
        return Ok(()); // 用户自己的流
    };
    match ticket {
        Some(t) if t == format!("{id}:live") => Ok(()),
        _ => Err(format!(
            "runtime self-check failed: this flow carries the DupHub identity {id:?} but the run \
             holds no matching execution ticket — a dumped copy cannot execute; run it through \
             POST /flow/run with an activation code"
        )),
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// D19 任务面客户端（hub /api/tasks；DUPHUB.md 〇之二：成果计费）

/// arm 一个任务。锚任务带 anchor（hub 取其每次成功价）；裸任务
/// （Tier-2）anchor 为空时 price_cents 显式传（缺省 0=免费记账）。
/// 返回 hub 任务行（含 id/state/hold_cents/expires_at）。
pub async fn task_arm(
    anchor: &str,
    session_id: &str,
    expected_n: u32,
    price_cents: i64,
    ttl_s: i64,
) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    let mut body = serde_json::json!({
        "anchor": anchor,
        "session_id": session_id,
        "expected_n": expected_n,
        "ttl_s": ttl_s,
    });
    if anchor.is_empty() {
        body["price_cents"] = serde_json::json!(price_cents);
    }
    post_json(&a, "/api/tasks", &body).await
}

/// 结算（幂等）：successes 次成功 + 证据包。Tier-1 观察器命中与
/// Tier-2 agent 申报同门。
pub async fn task_settle(
    task_id: i64,
    successes: u32,
    evidence: &serde_json::Value,
) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    post_json(
        &a,
        &format!("/api/tasks/{task_id}/settle"),
        &serde_json::json!({ "successes": successes, "evidence": evidence }),
    )
    .await
}

/// 主动放弃（agent 明知失败/主人取消）：冻结全退。
pub async fn task_refund(task_id: i64) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    post_json(&a, &format!("/api/tasks/{task_id}/refund"), &serde_json::json!({})).await
}

/// 任务行（惰性过期在读端发生）。
pub async fn task_get(task_id: i64) -> Result<serde_json::Value, HubError> {
    let a = load().ok_or(HubError::NotActivated)?;
    get_json(&a, &format!("/api/tasks/{task_id}")).await
}
