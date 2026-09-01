use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn query_mpv_prop(sock: &str, prop: &str) -> Option<f64> {
    let mut stream = UnixStream::connect(sock).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(400))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(400))).ok();
    let cmd = format!("{{\"command\":[\"get_property\",\"{prop}\"]}}\n");
    stream.write_all(cmd.as_bytes()).ok()?;
    let mut buf = vec![0u8; 512];
    let n = stream.read(&mut buf).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&buf[..n]).ok()?;
    if v["error"].as_str() != Some("success") {
        return None;
    }
    match v["data"] {
        serde_json::Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        _ => v["data"].as_f64(),
    }
}

pub fn query_mpv_position(sock: &str) -> Option<(f64, f64)> {
    let mut stream = UnixStream::connect(sock).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(400))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(400))).ok();

    let cmd1 = r#"{"command":["get_property","time-pos"]}"#;
    let cmd2 = r#"{"command":["get_property","duration"]}"#;
    stream.write_all(cmd1.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.write_all(cmd2.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;

    let mut buf = vec![0u8; 512];
    let n1 = stream.read(&mut buf).ok()?;
    let v1: serde_json::Value = serde_json::from_slice(&buf[..n1]).ok()?;
    let pos = if v1["error"].as_str() == Some("success") {
        v1["data"].as_f64().unwrap_or(0.0)
    } else {
        return None;
    };

    let n2 = stream.read(&mut buf).ok()?;
    let v2: serde_json::Value = serde_json::from_slice(&buf[..n2]).ok()?;
    let dur = if v2["error"].as_str() == Some("success") {
        v2["data"].as_f64().unwrap_or(0.0)
    } else {
        0.0
    };

    Some((pos, dur))
}

pub fn send_mpv_cmd(sock: &str, json_cmd: &str) -> bool {
    let Ok(mut stream) = UnixStream::connect(sock) else { return false; };
    stream.set_write_timeout(Some(Duration::from_millis(400))).ok();
    stream.write_all(json_cmd.as_bytes()).is_ok()
}

pub fn seek_mpv_to(sock: &str, seconds: f64) -> bool {
    let cmd = format!(
        r#"{{"command":["set_property","time-pos",{seconds:.3}]}}"#
    );
    send_mpv_cmd(sock, &cmd)
}

pub fn show_text(sock: &str, text: &str, duration_ms: u64) -> bool {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let cmd = format!(
        r#"{{"command":["show-text","{escaped}",{duration_ms}]}}"#
    );
    send_mpv_cmd(sock, &cmd)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SkipWindows {
    pub op_start: f64,
    pub op_end: f64,
    pub ed_start: f64,
    pub ed_end: f64,
    pub episode_length: f64,
}

pub fn classify_position(sk: &SkipWindows, pos: f64) -> Option<&'static str> {
    if pos < 0.0 {
        return None;
    }
    if sk.op_end > 0.0 && pos >= sk.op_start && pos < sk.op_end {
        return Some("op");
    }
    if sk.ed_end > 0.0 && pos >= sk.ed_start && pos < sk.ed_end {
        return Some("ed");
    }
    None
}

#[derive(Debug, Default)]
pub struct AniSkipState {
    pub op_skipped: bool,
    pub ed_skipped: bool,
    pub windows: Option<SkipWindows>,
}

#[derive(Debug, Clone, Copy)]
pub enum SkipOutcome {
    Op { from: f64, to: f64 },
    Ed { from: f64, to: f64 },
}

pub fn apply_aniskip(sock: &str, state: &mut AniSkipState, pos: f64) -> Option<SkipOutcome> {
    let win = state.windows?;
    if !state.op_skipped {
        if let Some(kind) = classify_position(&win, pos) {
            if kind == "op" && seek_mpv_to(sock, win.op_end) {
                state.op_skipped = true;
                return Some(SkipOutcome::Op { from: win.op_start, to: win.op_end });
            }
        }
    }
    if !state.ed_skipped {
        if let Some(kind) = classify_position(&win, pos) {
            if kind == "ed" && seek_mpv_to(sock, win.ed_end + 1.0) {
                state.ed_skipped = true;
                return Some(SkipOutcome::Ed { from: win.ed_start, to: win.ed_end + 1.0 });
            }
        }
    }
    None
}
