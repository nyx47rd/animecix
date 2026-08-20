#!/usr/bin/env python3
"""TANI AMAÇLI: AnimeciX'in gerçek AT-SPI ağacını döker (rol + isim).
capture_auto.py'i tahminle değil, bu çıktıya göre düzeltmek için kullanıyoruz.
Kullanım: python3 a11y_dump.py   (app çalışırken veya otomatik başlatır)
"""
import os
import sys
import time
import subprocess

try:
    import pyatspi
except ImportError:
    sys.exit("pyatspi yok: sudo apt install python3-pyatspi")


def children(n):
    try:
        return [n.getChild(i) for i in range(n.getChildCount())]
    except Exception:
        return []


def find_app():
    desktop = pyatspi.Registry.getDesktop(0)
    for a in desktop:
        if "animecix" in (a.name or "").lower():
            return a
    return None


def rec(n, depth, out, cap):
    if len(out) >= cap:
        return
    try:
        r = str(n.getRole())
        nm = n.name or ""
    except Exception:
        return
    if nm.strip() or "button" in r.lower():
        out.append(f"{'  '*depth}[{r}] {nm!r}")
    try:
        for c in children(n):
            rec(c, depth + 1, out, cap)
    except Exception:
        pass


def main():
    root = find_app()
    if not root:
        app = "./AnimeciX-x86_64.AppImage"
        if not os.path.exists(app):
            app = "AnimeciX-x86_64.AppImage"
        subprocess.Popen([app], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(6)
        root = find_app()
    if not root:
        sys.exit("app bulunamadi")
    out = []
    rec(root, 0, out, 500)
    print("\n".join(out))
    print(f"\n--- TOPLAM YAZDIRILAN: {len(out)} ---")


if __name__ == "__main__":
    main()
