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
