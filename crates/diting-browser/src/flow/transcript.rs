//! The #44 transcript pair: local-tool plumbing, deliberately NOT a general
//! exec. The argv is fixed — ffmpeg and the OCR/ASR helper with numeric
//! args only, clamped — so a flow can aim the capability at a file, never
//! at a shell. The evolving half of the pipeline (frame merge, majority
//! vote, SRT assembly, OCR/ASR fusion) lives in flow eval steps, editable
//! without an engine rebuild.

use super::{str_arg, substitute};
use serde_json::{json, Map, Value};

/// `transcript_frames` — the OCR leg: ffmpeg samples the video at a fixed
/// fps, then `DITING_OCR_BIN` reads every frame. Output: one JSON line per
/// frame (`{file, texts: [{t, y}]}`, y = top-origin normalized). Absent
/// tools are a clean error receipt — the no-capability posture, not a
/// crash; the frames dir is a tempdir, removed before returning.
pub(super) async fn exec_transcript_frames_step(
    a: &Value,
    vars: &Map<String, Value>,
) -> Result<Value, String> {
    let a = substitute(a, vars).map_err(|e| format!("transcript_frames: {e}"))?;
    let input = str_arg(&a, "input", "transcript_frames")?;
    let fps = a
        .get("fps")
        .and_then(|v| v.as_f64())
        .unwrap_or(2.0)
        .clamp(0.5, 10.0);
    let scale = a
        .get("scale")
        .and_then(|v| v.as_u64())
        .unwrap_or(640)
        .clamp(320, 1920);
    let ocr_bin = std::env::var("DITING_OCR_BIN").unwrap_or_default();
    if ocr_bin.is_empty() {
        return Err(
            "transcript_frames: DITING_OCR_BIN not set — point it at the OCR helper \
             (see the drama-transcript flow's docs)"
                .into(),
        );
    }
    let dir = std::env::temp_dir().join(format!(
        "diting-transcript-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("transcript_frames: tempdir: {e}"))?;

    // PTS is the timeline truth (#44's hard constraint), so sampling is
    // select-on-container-time (`t`, not frame-count — VFR-safe), and the
    // chosen frames' own timestamps ride along via showinfo's stderr —
    // parsed in output order, which -fps_mode vfr keeps 1:1 with the
    // %04d filenames. Without vfr the image2 muxer duplicates selected
    // frames up to the source frame rate (measured: 179 selected, 2684
    // written) and the numbering lies.
    let step = 1.0 / fps;
    let dir_pattern = dir.join("frame_%04d.jpg");
    let ff = tokio::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-loglevel")
        .arg("info")
        .arg("-y")
        .arg("-i")
        .arg(&input)
        .arg("-vf")
        .arg(format!(
            "select='isnan(prev_selected_t)+gte(t,prev_selected_t+{step})',scale={scale}:-1,showinfo"
        ))
        .arg("-fps_mode")
        .arg("vfr")
        .arg(&dir_pattern)
        .output()
        .await
        .map_err(|e| format!("transcript_frames: ffmpeg spawn: {e} (installed?)"))?;
    if !ff.status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "transcript_frames: ffmpeg failed: {}",
            String::from_utf8_lossy(&ff.stderr).trim()
        ));
    }
    // showinfo's per-frame line: "n: 1 pts: 7680 pts_time:0.5 ..." —
    // one pts_time per selected frame, same order as the jpgs.
    let pts_s: Vec<f64> = String::from_utf8_lossy(&ff.stderr)
        .lines()
        .filter_map(|l| l.split_once("pts_time:"))
        .filter_map(|(_, rest)| {
            rest.split(|c: char| c.is_whitespace())
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        })
        .collect();
    let mut frames: Vec<std::path::PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jpg"))
            .collect(),
        Err(_) => Vec::new(),
    };
    // Numeric frame order, not lexicographic: %04d padding runs out at
    // 10k frames (~83min at fps=2), past which frame_10000 sorts before
    // frame_9999 bytewise and the OCR lines arrive out of order — the
    // merge walks cues assuming frame order.
    frames.sort_by_key(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('_').next())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
    });
    if frames.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "transcript_frames: no frames extracted from {input:?}"
        ));
    }
    if pts_s.len() != frames.len() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!(
            "transcript_frames: frame/timestamp mismatch — {} frames, {} showinfo timestamps \
             (ffmpeg build prints showinfo differently?)",
            frames.len(),
            pts_s.len()
        ));
    }

    // One helper process per ~500 frames: argv length stays far under the
    // OS limit even for a 2-hour episode at high fps, without a streaming
    // protocol between two fixed tools.
    let mut lines: Vec<Value> = Vec::with_capacity(frames.len());
    for chunk in frames.chunks(500) {
        let out = tokio::process::Command::new(&ocr_bin)
            .args(chunk)
            .output()
            .await
            .map_err(|e| {
                let _ = std::fs::remove_dir_all(&dir);
                format!("transcript_frames: OCR helper spawn: {e} (DITING_OCR_BIN executable?)")
            })?;
        if !out.status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!(
                "transcript_frames: OCR helper failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        for l in String::from_utf8_lossy(&out.stdout).lines() {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                lines.push(v);
            }
        }
    }
    let n = frames.len();
    let _ = std::fs::remove_dir_all(&dir);
    Ok(json!({
        "ok": true,
        "input": input,
        "fps": fps,
        "frames": n,
        // Container PTS per frame, seconds, 1-based frame i → pts[i-1].
        // The cue timeline is built on these, never on frame-count math.
        "pts": pts_s,
        "lines": lines,
    }))
}

/// `transcript_audio` — the ASR leg: ffmpeg splits the track to 16k mono
/// wav, then whisper.cpp (`DITING_ASR_BIN`, default whisper-cli, with
/// `DITING_ASR_MODEL`) transcribes it. Output: whisper's stdout segments
/// parsed to `[{start_s, end_s, text}]` — coarser than OCR but it hears
/// what the censor asterisked out of the burned-in subtitles.
pub(super) async fn exec_transcript_audio_step(
    a: &Value,
    vars: &Map<String, Value>,
) -> Result<Value, String> {
    let a = substitute(a, vars).map_err(|e| format!("transcript_audio: {e}"))?;
    let input = str_arg(&a, "input", "transcript_audio")?;
    let lang = a
        .get("lang")
        .and_then(|v| v.as_str())
        .unwrap_or("zh")
        .to_string();
    let threads = a
        .get("threads")
        .and_then(|v| v.as_u64())
        .unwrap_or(8)
        .clamp(1, 16);
    let model = std::env::var("DITING_ASR_MODEL").unwrap_or_default();
    if model.is_empty() {
        return Err(
            "transcript_audio: DITING_ASR_MODEL not set — point it at a ggml whisper model \
             (e.g. ~/.cache/whisper/ggml-base.bin)"
                .into(),
        );
    }
    let asr_bin = std::env::var("DITING_ASR_BIN").unwrap_or_else(|_| "whisper-cli".to_string());
    let wav = std::env::temp_dir().join(format!(
        "diting-asr-{:x}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let cleanup = || {
        let _ = std::fs::remove_file(&wav);
    };
    let ff = tokio::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-y")
        .arg("-i")
        .arg(&input)
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg(&wav)
        .output()
        .await
        .map_err(|e| format!("transcript_audio: ffmpeg spawn: {e} (installed?)"))?;
    if !ff.status.success() {
        cleanup();
        return Err(format!(
            "transcript_audio: ffmpeg failed: {}",
            String::from_utf8_lossy(&ff.stderr).trim()
        ));
    }
    let out = tokio::process::Command::new(&asr_bin)
        .arg("-m")
        .arg(&model)
        .arg("-l")
        .arg(&lang)
        .arg("-t")
        .arg(threads.to_string())
        .arg("--no-prints")
        .arg("-np")
        .arg(&wav)
        .output()
        .await
        .map_err(|e| {
            cleanup();
            format!("transcript_audio: ASR spawn: {e} (DITING_ASR_BIN executable?)")
        })?;
    cleanup();
    if !out.status.success() {
        return Err(format!(
            "transcript_audio: ASR failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let segs = parse_whisper_segments(&String::from_utf8_lossy(&out.stdout));
    Ok(json!({
        "ok": true,
        "input": input,
        "lang": lang,
        "segments": segs,
    }))
}

/// whisper.cpp's stdout segment shape is
/// `[00:00:02.440 --> 00:00:04.560]  text` — stable across the brew
/// family and the `--no-prints` / `-np` flags used above. Hand-parsed so
/// the op carries no new dependency; banner and bracketless lines fall
/// through, and a timestamp pair with no text after it is dropped (whisper
/// prints those around music beds).
pub(super) fn parse_whisper_segments(stdout: &str) -> Vec<Value> {
    let parse = |s: &str| -> Option<f64> {
        let p: Vec<&str> = s.trim().split(':').collect();
        match p.len() {
            3 => Some(
                p[0].parse::<f64>().ok()? * 3600.0
                    + p[1].parse::<f64>().ok()? * 60.0
                    + p[2].parse::<f64>().ok()?,
            ),
            2 => Some(p[0].parse::<f64>().ok()? * 60.0 + p[1].parse::<f64>().ok()?),
            _ => None,
        }
    };
    let mut segs = Vec::new();
    for l in stdout.lines() {
        let l = l.trim();
        let Some(open) = l.find('[') else { continue };
        let Some(close) = l[open..].find(']') else {
            continue;
        };
        let inner = &l[open + 1..open + close];
        let Some(arrow) = inner.find("-->") else {
            continue;
        };
        let (Some(st), Some(en)) = (parse(&inner[..arrow]), parse(&inner[arrow + 3..])) else {
            continue;
        };
        let text = l[open + close + 1..].trim();
        if !text.is_empty() {
            segs.push(json!({ "start_s": st, "end_s": en, "text": text }));
        }
    }
    segs
}
