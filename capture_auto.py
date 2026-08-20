#!/usr/bin/env python3
"""AnimeciX ekran görüntüleri - TAM OTOMATIK yakalama (KDE Plasma / Wayland).

GTK a11y (AT-SPI) üzerinden butonları isimleriyle bulup tıklar, her sayfayı
`spectacle` ile kaydeder. Kullanıcı müdahalesi gerekmez.

Gereksinimler:
  - spectacle  (KDE ekran görüntüsü aracı)
  - python3-pyatspi  (Debian/Ubuntu: sudo apt install python3-pyatspi)
                     (Arch:          sudo pacman -S python-at-spi)
                     (Fedora:        sudo dnf install python3-pyatspi)
  - AppImage repo kökünde: ./AnimeciX-x86_64.AppImage

Kullanım:
  python3 capture_auto.py
"""
import os
import sys
import time
import subprocess

try:
    import pyatspi
except ImportError:
    sys.exit("HATA: pyatspi yok. Kurulum: sudo apt install python3-pyatspi "
             "(veya pacman/dnf karşılığı).")

APP = "./AnimeciX-x86_64.AppImage"
if not os.path.exists(APP):
    APP = "AnimeciX-x86_64.AppImage"
if not os.path.exists(APP):
    sys.exit(f"HATA: {APP} bulunamadı (repo köküne koy).")

os.makedirs("screenshots", exist_ok=True)


def log(msg):
    print(f"[capture_auto] {msg}", flush=True)


def find_app():
    desktop = pyatspi.Registry.getDesktop(0)
    for app in desktop:
        name = (app.name or "").lower()
        if "animecix" in name:
            return app
    return None


def walk(node, pred):
    """pred(node) -> bool; ilk eşleşenizi döndürür (BFS)."""
    stack = [node]
    while stack:
        n = stack.pop()
        try:
            if pred(n):
                return n
        except Exception:
            pass
        try:
            for c in n:
                stack.append(c)
        except Exception:
            pass
    return None


def find_button(root, *labels):
    labels = [l.lower() for l in labels]
    def pred(n):
        try:
            return str(n.getRole()) == "push button" and (n.name or "").lower() in labels
        except Exception:
            return False
    return walk(root, pred)


def click(obj):
    try:
        ai = obj.queryAction()
        for i in range(ai.nActions):
            if ai.getName(i).lower() in ("click", "activate", "press"):
                ai.doAction(i)
                return True
    except Exception as e:
        log(f"  tiklama hatasi: {e}")
    return False


def shot(name):
    subprocess.run(["spectacle", "-a", "-b", "-o", f"screenshots/{name}.png"],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    log(f"kaydedildi: screenshots/{name}.png")


def go_home(root):
    # Geri butonuna basarak Ana Sayfa'ya dönmeye çalış
    for _ in range(6):
        b = find_button(root, "geri")
        if not b:
            break
        if not click(b):
            break
        time.sleep(0.8)


log("AppImage başlatılıyor...")
subprocess.Popen([APP], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(6)

root = find_app()
if not root:
    sys.exit("HATA: AnimeciX a11y ağacında bulunamadı (at-spi çalışıyor mu?)")

# 1) Ana Sayfa (başlangıç)
shot("home")
time.sleep(1.2)

# 2) Favoriler / Maraton / Geçmiş / Ayarlar
for label, fname in [("Favoriler", "favorites"), ("Maraton", "marathon"),
                     ("Geçmiş", "history"), ("Ayarlar", "settings")]:
    b = find_button(root, label)
    if b:
        click(b)
        time.sleep(1.5)
        shot(fname)
    else:
        log(f"UYARI: buton bulunamadı -> {label}")

# 3) Arama (arama butonu yoksa Ctrl+S kısayolu)
b = find_button(root, "arama yap", "ara", "search")
if b:
    click(b)
    time.sleep(1.5)
    shot("search")
else:
    log("UYARI: arama butonu bulunamadı; Ctrl+S deneniyor")
    try:
        subprocess.run(["xdotool", "key", "ctrl+s"], check=False)
        time.sleep(1.5)
        shot("search")
    except Exception:
        log("  arama atlandi")

# 4) Bölümler: Ana Sayfa'ya dön, ilk başlık kartına tıkla
go_home(root)
time.sleep(1.0)
card = walk(root, lambda n: str(n.getRole()) in ("list item", "table cell")
            and (n.name or "").strip() != "")
if card:
    click(card)
    time.sleep(2.0)
    shot("episodes")
    go_home(root)
else:
    log("UYARI: bölüm kartı bulunamadı (ağ/Liste boş olabilir) -> episodes atlandi")

log("Bitti. Görseller screenshots/ içinde.")
log("Sonra: git add screenshots && git commit -m screenshots && git push")
