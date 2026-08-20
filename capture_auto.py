#!/usr/bin/env python3
"""AnimeciX ekran görüntüleri - TAM OTOMATIK (--goto CLI ile, a11y gerektirmez).

Her sayfa için AppImage'i `--goto <sayfa>` ile başlatır, belirli süre bekler,
`spectacle` ile aktif pencereyi yakalar, sonra kapatır. Kullanıcı müdahalesi yok.

Gereksinimler:
  - spectacle  (KDE ekran görüntüsü aracı)
  - ./AnimeciX-x86_64.AppImage  (v3.2.0+)  -> Releases'dan indir, repo köküne koy

Kullanım:
  python3 capture_auto.py
"""
import os
import sys
import time
import subprocess

APP = "./AnimeciX-x86_64.AppImage"
if not os.path.exists(APP):
    APP = "AnimeciX-x86_64.AppImage"
if not os.path.exists(APP):
    sys.exit(f"HATA: {APP} bulunamadı (v3.2.0+ indir, repo köküne koy).")

os.makedirs("screenshots", exist_ok=True)

# (--goto argümanı, dosya adı, bekleme sn)
PAGES = [
    ("welcome",   "welcome",   3),
    ("home",      "home",      4),
    ("favorites", "favorites", 4),
    ("marathon",  "marathon",  4),
    ("history",   "history",   4),
    ("settings",  "settings",  4),
    ("search",    "search",    6),
    ("episodes",  "episodes",  8),
]


def shot(name):
    subprocess.run(["spectacle", "-a", "-b", "-o", f"screenshots/{name}.png"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(f"[capture_auto] kaydedildi: screenshots/{name}.png", flush=True)


def kill():
    subprocess.run(["pkill", "-f", "AnimeciX-x86_64"], check=False)
    time.sleep(1.2)


def main():
    for arg, name, wait in PAGES:
        print(f"[capture_auto] '{arg}' açılıyor (bekleniyor {wait}s)...", flush=True)
        kill()  # önceki süreç kalıntı bırakmasın
        subprocess.Popen([APP, "--goto", arg],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(wait)
        shot(name)
    kill()
    print("[capture_auto] Bitti. Görseller screenshots/ içinde.")
    print("Sonra: git add screenshots && git commit -m screenshots && git push")


if __name__ == "__main__":
    main()
