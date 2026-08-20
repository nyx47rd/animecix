#!/usr/bin/env bash
# AnimeciX ekran görüntüleri yakalama yardımcı scripti (KDE Plasma).
# Her adımda ilgili sayfayı açıp bu terminale tıklayıp ENTER'a basın;
# aktif pencere screenshots/<sayfa>.png olarak kaydedilir.
set -u
cd "$(dirname "$0")"
mkdir -p screenshots

APP="./AnimeciX-x86_64.AppImage"
[ -x "$APP" ] || APP="AnimeciX-x86_64.AppImage"

CAP=""
if command -v spectacle >/dev/null 2>&1; then
  CAP="spectacle -a -b -o"   # KDE: aktif pencere, GUI'siz
elif command -v import >/dev/null 2>&1; then
  CAP="import -window root"  # X11 yedeği: tüm masaüstü
fi
if [ -z "$CAP" ]; then
  echo "Hata: ne 'spectacle' ne de 'import' bulundu."
  exit 1
fi

echo "AppImage başlatılıyor..."
"$APP" >/dev/null 2>&1 &
APP_PID=$!
sleep 4

pages=(welcome home search episodes favorites marathon history settings)
for p in "${pages[@]}"; do
  echo
  echo ">>> Şimdi '$p' sayfasını açın, ardından bu terminale tıklayıp ENTER'a basın."
  read -r _
  $CAP "screenshots/$p.png"
  echo "    kaydedildi: screenshots/$p.png"
done

kill "$APP_PID" 2>/dev/null
echo "Bitti. Görseller screenshots/ içinde."
echo "Şimdi: git add screenshots && git commit -m 'screenshots' && git push"
