"""Bas'siz mpv atlama testi: sahte video + gercek mpv'ye karsi entegrasyon.

Kapsar: input.conf sozdizimi, keybind IPC kurulumu, `keypress s/e` ile
intro/outro sonuna seek, emoji'li show-text OSD, ASS sag-ust katman
(sub-add + track) ve `run` mekanizmasi (`M` tusunun kullandigi yol;
testte zararsiz `touch` ile kanitlanir).
(Uretici string'lerin dogrulugu Rust birim testlerinde; burada gercek
mpv davranisi kanitlanir.)
Cikis: 0=PASS, 1=FAIL, 3=arac yok.
"""
import json
import shutil
import socket
import subprocess
import sys
import time
import os

VIDEO = "/tmp/skip-harness.mp4"
CONF = "/tmp/skip-harness.conf"
SOCK = "/tmp/skip-harness.sock"
ASS = "/tmp/skip-harness.ass"
MARKER = "/tmp/skip-harness-m-marker"

# Uygulamanin urettigi formatin birebir aynisi (op 5->10, ed 20->25).
CONF_TEXT = (
    's seek 10.0 absolute; show-text "⏩ İntro Atlandı (harness: 00:05 → 00:10)" 3000\n'
    'e seek 25.0 absolute; show-text "⏩ Outro Atlandı (harness: 00:20 → 00:25)" 3000\n'
    'S seek -30; show-text "⏪ 30s Geri" 2000\n'
    "End ignore\n"
)
KEYBINDS = [
    '{"command":["keybind","s","seek 10.0 absolute"]}',
    '{"command":["keybind","e","seek 25.0 absolute"]}',
]
OSD = '{"command":["show-text", "⏩ test • 🎵 X — Y", 1500]}'
NF = [
    '{"command":["keybind","s","show-text \\"⚠️ İntro zamanı bulunamadı\\" 2500"]}',
    '{"command":["keybind","e","show-text \\"⚠️ Outro zamanı bulunamadı\\" 2500"]}',
]


class Fail(Exception):
    pass


def check(cond, msg):
    print(("  ok: " if cond else "  FAIL: ") + msg)
    if not cond:
        raise Fail(msg)


def ipc(cmd):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5)
    s.connect(SOCK)
    s.sendall((cmd + "\n").encode())
    buf = b""
    while not buf.endswith(b"\n"):
        chunk = s.recv(4096)
        if not chunk:
            break
        buf += chunk
    s.close()
    return json.loads(buf.decode().strip())


def time_pos():
    return ipc('{"command":["get_property","time-pos"]}').get("data", -1.0)


def main():
    for tool in ("ffmpeg", "mpv"):
        if shutil.which(tool) is None:
            print(f"SKIP-HARNESS: {tool} yok, atlaniyor (kod 3)")
            return 3
    mpv = None
    try:
        print("1) sahte video uretiliyor...")
        r = subprocess.run(["ffmpeg", "-y", "-v", "error", "-f", "lavfi",
                            "-i", "testsrc=duration=30:size=640x360:rate=10",
                            "-pix_fmt", "yuv420p", "-preset", "ultrafast", VIDEO],
                           timeout=120)
        check(r.returncode == 0 and os.path.getsize(VIDEO) > 10000, "ffmpeg videosu")

        print("2) mpv baslatiliyor (conf+soket)...")
        for f in (CONF, SOCK):
            if os.path.exists(f):
                os.remove(f)
        with open(CONF, "w") as f:
            f.write(CONF_TEXT)
        mpv = subprocess.Popen(["mpv", "--vo=null", "--ao=null", "--pause=yes",
                                f"--input-ipc-server={SOCK}", f"--input-conf={CONF}", VIDEO],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(75):
            if os.path.exists(SOCK):
                break
            time.sleep(0.2)
        check(os.path.exists(SOCK), "ipc soketi")
        time.sleep(2)  # conf yukleme penceresi

        print("3) canli keybind kurulumu...")
        for k in KEYBINDS:
            check(ipc(k).get("error") == "success", f"keybind: {k[:40]}...")

        print("4) `s` -> intro sonu (10sn)...")
        check(ipc('{"command":["keypress","s"]}').get("error") == "success", "keypress s")
        time.sleep(0.8)
        pos = time_pos()
        check(abs(pos - 10.0) < 2.0, f"s seek pos={pos}")

        print("5) `e` -> outro sonu (25sn)...")
        check(ipc('{"command":["keypress","e"]}').get("error") == "success", "keypress e")
        time.sleep(0.8)
        pos = time_pos()
        check(abs(pos - 25.0) < 2.0, f"e seek pos={pos}")

        print("6) emoji OSD + bulunamadi tuslari...")
        check(ipc(OSD).get("error") == "success", "show-text emoji")
        for k in NF:
            check(ipc(k).get("error") == "success", f"not-found: {k[27:45]}...")

        print("7) ASS sag-ust katman (sub-add + track)...")
        with open(ASS, "w") as f:
            f.write("[Script Info]\nScriptType: v4.00+\nPlayResX: 1280\nPlayResY: 720\n"
                    "\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, OutlineColour, "
                    "BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n"
                    "Style: Song,sans-serif,34,&H00FFFFFF,&H90000000,&H90000000,0,0,1,2,0,9,24,24,24,1\n"
                    "\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
                    "Dialogue: 0,0:00:05.00,0:00:10.00,Song,,0,0,0,,{\\an9}\U0001f3b5 test — X\n")
        r = ipc(json.dumps({"command": ["sub-add", ASS]}))
        check(r.get("error") == "success", f"sub-add: {r}")
        tracks = ipc('{"command":["get_property","track-list"]}')
        subs = [t for t in tracks.get("data", []) if t.get("type") == "sub"]
        check(len(subs) >= 1, f"sub track listede: {len(subs)}")

        print("8) `run` mekanizmasi (M tusunun yolu; zararsiz dokunma)...")
        if os.path.exists(MARKER):
            os.remove(MARKER)
        run_cmd = 'run "/bin/sh" "-c" "touch ' + MARKER + '"'
        check(ipc(json.dumps({"command": ["keybind", "M", run_cmd]})).get("error") == "success", "M keybind")
        check(ipc('{"command":["keypress","M"]}').get("error") == "success", "keypress M")
        for _ in range(25):
            if os.path.exists(MARKER):
                break
            time.sleep(0.2)
        check(os.path.exists(MARKER), "`M` komutu calisti")

        print("SKIP-HARNESS: PASS (s→10sn, e→25sn, OSD+ASS+M ok)")
        return 0
    except Fail as e:
        print(f"SKIP-HARNESS: FAIL ({e})")
        return 1
    finally:
        if mpv is not None:
            mpv.kill()
            mpv.wait()
        for f in (VIDEO, CONF, SOCK, ASS, MARKER):
            if os.path.exists(f):
                os.remove(f)


if __name__ == "__main__":
    sys.exit(main())
