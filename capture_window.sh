#!/usr/bin/env bash
# Hedef pencereye bir kez tıkla, sonra o pencereyi her 5 sn'de bir kaydet.
# Kullanım:
#   ./capture_window.sh            -> screenshots/cap_001.png, cap_002.png, ...
#   ./capture_window.sh home       -> screenshots/home_001.png, home_002.png, ...
# Durdurmak için: Ctrl+C
set -uo pipefail

OUT_DIR="screenshots"
PREFIX="${1:-cap}"
INTERVAL=5
mkdir -p "$OUT_DIR"

WINDOW_ID=""
if command -v xdotool >/dev/null 2>&1; then
    echo "Hedef pencereye tıkla (5 sn içinde)..."
    WINDOW_ID=$(timeout 5 xdotool selectwindow 2>/dev/null || true)
fi

if [ -n "$WINDOW_ID" ]; then
    echo "Pencere seçildi (id=$WINDOW_ID). 5 sn aralıkla çekiliyor... (Ctrl+C ile dur)"
else
    echo "xdotool yok/çalışmadı (Wayland olabilir)."
    echo "Uygulama penceresini şimdi tıkla/odakla; aktif pencere çekilecek."
    sleep 3
fi

i=1
trap 'echo; echo "Bitti. Dosyalar: $OUT_DIR/${PREFIX}_*.png"' EXIT
while true; do
    f="$OUT_DIR/${PREFIX}_$(printf '%03d' "$i").png"
    if [ -n "$WINDOW_ID" ]; then
        # X11: pencereyi tekrar odakla ki her zaman o çekilsin
        xdotool windowactivate "$WINDOW_ID" 2>/dev/null || true
        sleep 0.4
        if import -window "$WINDOW_ID" "$f" 2>/dev/null; then
            echo "[$i] $f"
        else
            spectacle -a -b -o "$f" 2>/dev/null && echo "[$i] $f (spectacle)"
        fi
    else
        spectacle -a -b -o "$f" 2>/dev/null && echo "[$i] $f (aktif pencere)"
    fi
    i=$((i+1))
    sleep "$INTERVAL"
done
