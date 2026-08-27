#!/usr/bin/env bash
# Ayarlar sayfasının tamamını scroll ederek yakala ve dikey stitch et.
# Kullanım:
#   ./capture_settings.sh          → pencere yoksa X11 backend ile otomatik başlatır
#                                    (sen ayarlara gidemezsin, ana sayfa yakalanır)
#   ./capture_settings.sh --no-launch → SADECE mevcut pencereyi kullanır, başlatmaz
#                                    (sen açıp ayarlara geçtikten sonra çalıştır)

set -e

PROJECT_DIR="/home/veilzon/animecix-app"
APPIMAGE="$PROJECT_DIR/AnimeciX-x86_64.AppImage"
OUT_PNG="$PROJECT_DIR/docs/assets/screenshots/settings.png"
TMP_DIR="/tmp/animecix_shot_$$"
STARTED_BY_SCRIPT=0
APP_PID=""
AUTO_LAUNCH=1
[ "${1:-}" = "--no-launch" ] && AUTO_LAUNCH=0

mkdir -p "$TMP_DIR"

echo "[1/5] AnimeciX penceresi aranıyor"
WID=$(xdotool search --name "AnimeciX" 2>/dev/null | head -1)
if [ -z "$WID" ]; then
  # Title farklı olabilir, tüm pencereleri tara classname'e göre
  for w in $(xdotool search --name "" 2>/dev/null); do
    cls=$(xdotool getwindowclass "$w" 2>/dev/null)
    if echo "$cls" | grep -qi "animecix\|tr.com.animecix"; then
      WID="$w"; break
    fi
  done
fi

if [ -z "$WID" ]; then
  if [ "$AUTO_LAUNCH" -eq 0 ]; then
    echo "HATA: --no-launch modunda mevcut pencere gerekli, bulunamadı."
    exit 1
  fi
  echo "  pencere bulunamadı, AppImage X11 backend ile başlatılıyor..."
  GDK_BACKEND=x11 XDG_SESSION_TYPE=x11 WAYLAND_DISPLAY= \
    nohup "$APPIMAGE" >/tmp/animecix_run.log 2>&1 &
  APP_PID=$!
  STARTED_BY_SCRIPT=1
  for i in $(seq 1 40); do
    sleep 0.5
    WID=$(xdotool search --name "AnimeciX" 2>/dev/null | head -1)
    [ -n "$WID" ] && break
  done
  if [ -z "$WID" ]; then
    echo "HATA: Pencere bulunamadı (20s içinde). /tmp/animecix_run.log:"
    tail -20 /tmp/animecix_run.log
    [ -n "$APP_PID" ] && kill $APP_PID 2>/dev/null
    exit 1
  fi
fi
echo "  WID=$WID"

echo "[2/5] Pencere öne getirilip geometrisi alınıyor"
xdotool windowactivate --sync "$WID" 2>/dev/null || true
xdotool windowfocus --sync "$WID" 2>/dev/null || true
sleep 1
# Pencereye tıklayarak focus ver
GEO=$(xdotool getwindowgeometry --shell "$WID")
X=$(echo "$GEO" | awk -F= '/^X=/ {gsub(" ","",$2); print $2}')
Y=$(echo "$GEO" | awk -F= '/^Y=/ {gsub(" ","",$2); print $2}')
W=$(echo "$GEO" | awk -F= '/^WIDTH=/ {gsub(" ","",$2); print $2}')
H=$(echo "$GEO" | awk -F= '/^HEIGHT=/ {gsub(" ","",$2); print $2}')
CX=$((X + W/2))
CY=$((Y + H/2))
xdotool mousemove --window "$WID" $CX $CY click 1
sleep 0.8
echo "  X=$X Y=$Y W=$W H=$H"
if [ -z "$W" ] || [ "$W" -lt 200 ] || [ "$H" -lt 200 ]; then
  echo "HATA: Pencere geometrisi anormal: $Wx$H"
  exit 1
fi

echo "[3/5] En üste dönülüyor"
# Ayarlar sayfasının en üstüne git (sen zaten oradasın veya scroll etmiş olabilirsin)
# Ctrl+Home genelde çalışır
xdotool key --clearmodifiers "ctrl+Home" 2>/dev/null || true
sleep 1

echo "[4/5] Scroll ederek frame'ler yakalanıyor"
ffmpeg -hide_banner -loglevel error \
  -f x11grab -framerate 1 -video_size "${W}x${H}" -i ":0.0+${X},${Y}" \
  -frames:v 1 -y "$TMP_DIR/frame_00.png" 2>&1 | tail -1
echo "  frame_00.png"

SCROLL_COUNT=10
SLEEP_AFTER=0.8
for i in $(seq 1 $SCROLL_COUNT); do
  # xdotool'da Page_Down bazı widget'larda çalışmaz; Down ok ile birkaç satır,
  # sonra Page_Down. En güvenli: fare tekerleği (button 5) — gerçek scroll event.
  xdotool click --repeat 5 5
  sleep 0.4
  xdotool key --clearmodifiers "Page_Down" 2>/dev/null || true
  sleep $SLEEP_AFTER
  ffmpeg -hide_banner -loglevel error \
    -f x11grab -framerate 1 -video_size "${W}x${H}" -i ":0.0+${X},${Y}" \
    -frames:v 1 -y "$TMP_DIR/frame_$(printf %02d $i).png" 2>&1 | tail -1
  echo "  frame_$(printf %02d $i).png"
done

TOTAL=$(ls "$TMP_DIR"/frame_*.png 2>/dev/null | wc -l)
DISTINCT=$(md5sum "$TMP_DIR"/frame_*.png 2>/dev/null | awk '{print $1}' | sort -u | wc -l)
echo "  toplam $TOTAL frame, farklı $DISTINCT"

if [ "$DISTINCT" -lt 3 ]; then
  echo "UYARI: Frame'ler çok benzer — scroll çalışmamış olabilir."
fi

echo "[5/5] Pillow ile dikey birleştirme"
python3 - "$TMP_DIR" "$OUT_PNG" <<'PY'
import sys, os
from PIL import Image

tmp_dir, out = sys.argv[1], sys.argv[2]
files = sorted(f for f in os.listdir(tmp_dir) if f.startswith("frame_") and f.endswith(".png"))
if not files:
    print("HATA: hiç frame yok"); sys.exit(1)

imgs = [Image.open(os.path.join(tmp_dir, f)).convert("RGB") for f in files]
W = max(i.width for i in imgs)
total_h = sum(i.height for i in imgs)
combined = Image.new("RGB", (W, total_h), (255, 255, 255))
y = 0
for img in imgs:
    combined.paste(img, (0, y))
    y += img.height

if combined.height > 5000:
    ratio = 5000 / combined.height
    new_w = int(combined.width * ratio)
    combined = combined.resize((new_w, 5000), Image.LANCZOS)
combined.save(out, "PNG", optimize=True)
print(f"  → {out} ({combined.width}x{combined.height}, {os.path.getsize(out)//1024} KB)")
PY

rm -rf "$TMP_DIR"

if [ "$STARTED_BY_SCRIPT" -eq 1 ] && [ -n "$APP_PID" ]; then
  echo "AppImage kapatılıyor (PID=$APP_PID)..."
  kill $APP_PID 2>/dev/null || true
  sleep 1
  kill -9 $APP_PID 2>/dev/null || true
  pkill -9 -f "/tmp/.mount_Animec" 2>/dev/null || true
fi

echo "Bitti: $OUT_PNG"
