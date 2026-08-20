#!/usr/bin/env python3
"""TANI v2: AnimeciX AT-SPI agacini TAM döker (rol adı + isim + childCount).
roleToString ile gerçek rol adını verir; agacin cocuklari aciliyor mu görürüz.
Kullanım: python3 a11y_dump.py
"""
import os
import sys
import time
import subprocess

try:
    import pyatspi
except ImportError:
    sys.exit("pyatspi yok: sudo apt install python3-pyatspi")


def role_of(n):
    try:
        return pyatspi.roleToString(n.getRole())
    except Exception:
        try:
            return f"?{n.getRole()}"
        except Exception:
            return "?"


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
        r = role_of(n)
        nm = n.name or ""
        cc = n.getChildCount()
    except Exception:
        return
    out.append(f"{'  '*depth}[{r}] name={nm!r} childCount={cc}")
    try:
        for c in children(n):
            rec(c, depth + 1, out, cap)
    except Exception:
        pass


def dump(root, label):
    out = []
    rec(root, 0, out, 200)
    print(f"=== {label} ===")
    print("\n".join(out))
    print(f"--- TOPLAM: {len(out)} ---\n")


def main():
    root = find_app()
    fresh = False
    if not root:
        app = "./AnimeciX-x86_64.AppImage"
        if not os.path.exists(app):
            app = "AnimeciX-x86_64.AppImage"
        subprocess.Popen([app], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(6)
        root = find_app()
    if not root:
        sys.exit("app bulunamadi")
    dump(root, "normal baslatma")

    # AT-SPI koprusu acilmamis olabilir; GTK_A11Y=atspi ile dene
    print(">>> GTK_A11Y=atspi ile yeniden baslatiliyor (5sn)...")
    app = "./AnimeciX-x86_64.AppImage"
    if not os.path.exists(app):
        app = "AnimeciX-x86_64.AppImage"
    env = dict(os.environ)
    env["GTK_A11Y"] = "atspi"
    # oncekileri kapat
    try:
        subprocess.run(["pkill", "-f", "AnimeciX-x86_64"], check=False)
    except Exception:
        pass
    time.sleep(1)
    subprocess.Popen([app], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(6)
    root2 = find_app()
    if root2:
        dump(root2, "GTK_A11Y=atspi")
    else:
        print("GTK_A11Y=atspi baslatmasinda app bulunamadi")


if __name__ == "__main__":
    main()
