use async_trait::async_trait;

use super::{
    get_direct_first, proxied_plain_client, RawSearchResult, SearchEngine, SearchEngineError,
    SearchParams,
};

// ---------------------------------------------------------------------------
// 红果短剧 (com.phoenix.read, aid=8662) — 漫剧三榜免签引擎.
//
// 来源: zhangbaio/hongguo 逆向图谱 + 2026-11-07 裸调探针. 免签面只剩
// bookmall/cell/change/v 一条 (landpage/search 空脸 200 = 已要签名;
// 无 web 版). 游客自造设备参数即可, cdid/channel 都不校验.
//
// 签名 oracle 钩子 (未实现, 契约立此): env DITING_HONGGUO_SIGN_SERVER 指向
// 一个 POST {url, headers} -> JSON 签名头 map 的本地服务 (对齐上游 unidbg
// SIGN_SERVER 的 /sign 契约). 接上后解锁: 关键词搜索
// GET /reading/bookapi/search/tab/v (next_offset+passback 翻页)、
// landpage 上新/筛选、episodes、video_model 直链. 实现那天把
// fetch 层加一个 sign 步骤, 榜单模式的 query 过滤模式退役成兜底.
// ---------------------------------------------------------------------------

const API_HOST: &str = "api5-normal-sinfonlinea.fqnovel.com";
const APP_UA: &str = "com.phoenix.read/72232 (Linux; U; Android 12; zh_CN; XIAOMI; \
                      M2012K11AC; Build/SKQ1; tt-ok/3.12.13.20)";
/// 漫剧榜单 cell (hongguo 抓包; 真人短剧榜在 landpage 里 = 要签).
const COMIC_RANK_CELL: &str = "7470092475068071998";

/// 游客设备参数 (探针实测: rank 脸不校验 cdid/channel/klink_egdi,
/// 自造 device_id/iid 即过). 集中成常量, 要换靶时一处改.
const GUEST_DEVICE_PARAMS: &[(&str, &str)] = &[
    ("aid", "8662"),
    ("app_name", "novelread"),
    ("version_code", "72232"),
    ("version_name", "7.2.2.32"),
    ("device_platform", "android"),
    ("device_type", "M2012K11AC"),
    ("os_version", "12"),
    ("update_version_code", "72232"),
    ("device_id", "7512345678901234567"),
    ("iid", "7509876543210987654"),
];

/// A rank board. `sub` is the bookmall sub_selected_items value.
struct RankBoard {
    key: &'static str,
    sub: &'static str,
    label: &'static str,
}

const BOARDS: [RankBoard; 3] = [
    RankBoard {
        key: "recommend",
        sub: "comic_series_hot_rank",
        label: "推荐",
    },
    RankBoard {
        key: "hot",
        sub: "comic_series_hot_play",
        label: "热播",
    },
    RankBoard {
        key: "new",
        sub: "comic_series_new_rank",
        label: "新剧",
    },
];

/// Exact board selection: "hot"/"热播"/"新剧"/"recommend" (ASCII
/// case-insensitive). Anything else falls to keyword-filter mode.
fn board_for_query(query: &str) -> Option<&'static RankBoard> {
    let q = query.trim().to_ascii_lowercase();
    BOARDS
        .iter()
        .find(|b| b.key == q || b.label == query.trim())
}

fn build_rank_url(sub: &str, offset: Option<&str>, session: &str) -> String {
    let mut qs = String::new();
    for (k, v) in GUEST_DEVICE_PARAMS {
        qs.push_str(k);
        qs.push('=');
        qs.push_str(&urlencoding::encode(v));
        qs.push('&');
    }
    for (k, v) in [
        ("cell_id", COMIC_RANK_CELL),
        ("tab_type", "26"),
        ("client_req_type", "2"),
        ("client_template", "2"),
        ("screen_width_px", "1350"),
        ("selected_items", "comic_series_rank"),
        ("sub_selected_items", sub),
        ("session_uuid", session),
    ] {
        qs.push_str(k);
        qs.push('=');
        qs.push_str(&urlencoding::encode(v));
        qs.push('&');
    }
    if let Some(off) = offset {
        qs.push_str("offset=");
        qs.push_str(&urlencoding::encode(off));
        qs.push('&');
    }
    qs.push_str("_rticket=");
    qs.push_str(
        &std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
            .to_string(),
    );
    format!("https://{API_HOST}/reading/bookapi/bookmall/cell/change/v?{qs}")
}

/// One parsed rank entry.
struct RankItem {
    series_id: String,
    title: String,
    episode_cnt: u64,
    score: String,
    play_cnt: u64,
    copyright: String,
    desc: String,
}

/// Parse one rank page. Returns (items, has_more, next_offset).
fn parse_rank_json(body: &str) -> Result<(Vec<RankItem>, bool, Option<String>), SearchEngineError> {
    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| SearchEngineError::Transient(format!("json parse: {e}")))?;
    let code = parsed.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = parsed.get("message").and_then(|v| v.as_str()).unwrap_or("");
        return Err(SearchEngineError::Transient(format!(
            "hongguo api code={code} {msg}"
        )));
    }
    let view = parsed
        .get("data")
        .and_then(|d| d.get("cell_view"))
        .cloned()
        .unwrap_or_default();
    let has_more = view
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let next_offset = view
        .get("next_offset")
        .and_then(|v| v.as_i64())
        .map(|v| v.to_string());
    let mut items = Vec::new();
    for cell in view
        .get("cell_data")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
    {
        // video_data is a list on this face (upstream python tolerates both).
        let v = match cell.get("video_data") {
            Some(serde_json::Value::Array(a)) => a.first().cloned().unwrap_or_default(),
            Some(v @ serde_json::Value::Object(_)) => v.clone(),
            _ => continue,
        };
        let series_id = v
            .get("series_id")
            .or_else(|| v.get("book_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let title = v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if series_id.is_empty() || title.is_empty() {
            continue;
        }
        items.push(RankItem {
            series_id,
            title,
            episode_cnt: v.get("episode_cnt").and_then(|x| x.as_u64()).unwrap_or(0),
            score: v
                .get("score")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            play_cnt: v.get("play_cnt").and_then(|x| x.as_u64()).unwrap_or(0),
            copyright: v
                .get("copyright")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            desc: v
                .get("video_desc")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .chars()
                .take(60)
                .collect(),
        });
    }
    Ok((items, has_more, next_offset))
}

impl RankItem {
    fn to_result(&self, position: usize, board_key: &str) -> RawSearchResult {
        let mut snippet = format!("[{}] {}集", board_key, self.episode_cnt);
        if !self.score.is_empty() {
            snippet.push_str(&format!(" ★{}", self.score));
        }
        snippet.push_str(&format!(" · {}播放", self.play_cnt));
        if !self.copyright.is_empty() {
            snippet.push_str(&format!(" · {}", self.copyright));
        }
        if !self.desc.is_empty() {
            snippet.push_str(&format!(" · {}", self.desc));
        }
        RawSearchResult {
            title: self.title.clone(),
            url: format!("hongguo://series/{}", self.series_id),
            snippet,
            engine: "hongguo".into(),
            // Rank lists are short (10/page); position score within the page.
            score: 100.0 - position as f64,
            cookies: Vec::new(),
            js_extract_result: None,
            image: None,
        }
    }
}

/// Walk `want_pages` pages of one board, following next_offset (the API's
/// cursor is not guaranteed arithmetic). Safety cap: 5 fetches.
async fn fetch_board(
    board: &RankBoard,
    want_pages: usize,
) -> Result<Vec<RawSearchResult>, SearchEngineError> {
    let session = uuid::Uuid::new_v4().to_string();
    let mut offset: Option<String> = None;
    let mut out = Vec::new();
    for page in 0..want_pages.min(5) {
        let url = build_rank_url(board.sub, offset.as_deref(), &session);
        let body = get_direct_first(&url, &[("user-agent", APP_UA)], proxied_plain_client).await?;
        let (items, has_more, next) = parse_rank_json(&body)?;
        for (i, item) in items.iter().enumerate() {
            out.push(item.to_result(page * 10 + i, board.key));
        }
        if !has_more {
            break;
        }
        offset = next;
    }
    Ok(out)
}

pub struct HongguoEngine;

impl HongguoEngine {
    pub fn new() -> Self {
        HongguoEngine
    }
}

#[async_trait]
impl SearchEngine for HongguoEngine {
    fn name(&self) -> &str {
        "hongguo"
    }

    fn categories(&self) -> &[&str] {
        // Own vertical (like academic/code): opt in via engines=hongguo or
        // category=drama. Not on "general" — a 3-board walk per ordinary
        // query would cost real API calls for near-always-zero hits.
        &["drama"]
    }

    async fn search(
        &self,
        query: &str,
        params: SearchParams,
    ) -> Result<Vec<RawSearchResult>, SearchEngineError> {
        if let Some(board) = board_for_query(query) {
            let page = params.pageno.max(1);
            return fetch_board(board, page).await;
        }
        // Keyword mode: pull page 1 of all three boards, filter by title.
        // A single board failing (flaky CDN leg) degrades to the rest;
        // only total failure surfaces as an error.
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        let mut first_err = None;
        for board in &BOARDS {
            match fetch_board(board, 1).await {
                Ok(mut rows) => {
                    rows.retain(|r| r.title.to_lowercase().contains(&needle));
                    results.extend(rows);
                }
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        if results.is_empty() {
            if let Some(e) = first_err {
                return Err(e);
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_board_by_key_and_label() {
        assert_eq!(board_for_query("hot").unwrap().sub, "comic_series_hot_play");
        assert_eq!(board_for_query("HOT").unwrap().key, "hot");
        assert_eq!(board_for_query("热播").unwrap().key, "hot");
        assert_eq!(board_for_query("新剧").unwrap().key, "new");
        assert!(board_for_query("皇后她自带江山").is_none());
        assert!(board_for_query("").is_none());
    }

    #[test]
    fn parses_rank_fixture() {
        // Real captured cell (2026-11-07 probe), values shortened.
        let body = r#"{"code": 0, "message": "SUCCESS", "data": {"cell_view": {
            "cell_data": [
              {"cell_id": 0, "show_type": 505, "video_data": [{
                "vid": "7621226646028160024",
                "title": "万妖图录传第一季",
                "video_desc": "以妖血为墨、百妖谱为卷，可摹其形、夺其神通。",
                "series_id": "7621200213801176126",
                "play_cnt": 5567285,
                "copyright": "梦绘星界动漫",
                "score": "8.3",
                "episode_cnt": 81
              }]}
            ],
            "has_more": true, "next_offset": 10}}}"#;
        let (items, has_more, next) = parse_rank_json(body).unwrap();
        assert_eq!(items.len(), 1);
        assert!(has_more);
        assert_eq!(next.as_deref(), Some("10"));
        assert_eq!(items[0].series_id, "7621200213801176126");
        assert_eq!(items[0].episode_cnt, 81);
        assert_eq!(items[0].score, "8.3");
        let r = items[0].to_result(0, "hot");
        assert_eq!(r.title, "万妖图录传第一季");
        assert_eq!(r.url, "hongguo://series/7621200213801176126");
        assert!(r.snippet.contains("[hot] 81集 ★8.3 · 5567285播放"));
        assert!(r.snippet.contains("梦绘星界动漫"));
        assert_eq!(r.engine, "hongguo");
    }

    #[test]
    fn api_error_code_is_transient() {
        let body = r#"{"code": 8, "message": "risk control"}"#;
        assert!(parse_rank_json(body).is_err());
    }

    #[test]
    fn malformed_json_is_transient() {
        assert!(parse_rank_json("bad").is_err());
        assert!(parse_rank_json("").is_err());
    }
}
