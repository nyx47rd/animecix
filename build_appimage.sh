#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "==> AnimeciX AppImage Oluşturuluyor..."

# 0. Sürüm numarasını otomatik 0.0.1 artır (Cargo.toml)
#    Patch 9'dan taşarsa minor artar, minor 9'dan taşarsa major artar.
OLD_VER=$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)
MAJOR=$(echo "$OLD_VER" | cut -d. -f1)
MINOR=$(echo "$OLD_VER" | cut -d. -f2)
PATCH=$(echo "$OLD_VER" | cut -d. -f3)
NEW_PATCH=$((PATCH + 1))
if [ "$NEW_PATCH" -gt 9 ]; then
    NEW_PATCH=0
    NEW_MINOR=$((MINOR + 1))
    if [ "$NEW_MINOR" -gt 9 ]; then
        NEW_MINOR=0
        NEW_MAJOR=$((MAJOR + 1))
    else
        NEW_MAJOR=$MAJOR
    fi
else
    NEW_MAJOR=$MAJOR
    NEW_MINOR=$MINOR
fi
NEW_VER="${NEW_MAJOR}.${NEW_MINOR}.${NEW_PATCH}"

sed -i "0,/^version = .*/s//version = \"$NEW_VER\"/" Cargo.toml
echo "==> Sürüm yükseltildi: v$OLD_VER -> v$NEW_VER"

VERSION=$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)

# Otomatik GitHub Release: GITHUB_TOKEN tanımlıysa AppImage'ı bir release olarak yayınlar.
publish_release() {
  local token="${GITHUB_TOKEN:-}"
  local repo="${GITHUB_REPO:-nyx47rd/animecix-app}"
  local asset="AnimeciX-x86_64.AppImage"
  if [ -z "$token" ]; then
    echo "==> GITHUB_TOKEN tanımlı değil, GitHub Release oluşturulmayacak (AppImage yerelde kaldı)."
    return 0
  fi
  if [ ! -f "$asset" ]; then
    echo "==> $asset bulunamadı, release atlanıyor."
    return 0
  fi
  echo "==> GitHub Release oluşturuluyor (v$VERSION)..."
  local resp
  resp=$(curl -sS -X POST \
    -H "Authorization: Bearer $token" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/json" \
    -d "{\"tag_name\":\"v$VERSION\",\"name\":\"AnimeciX v$VERSION\",\"body\":\"Otomatik derleme.\\n\\nAppImage tek dosyadır; uygulama başlatmada yeni sürümü kontrol eder ve kendini günceller.\"}" \
    "https://api.github.com/repos/$repo/releases")
  local upload_url
  upload_url=$(printf '%s' "$resp" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('upload_url',''))" 2>/dev/null)
  if [ -z "$upload_url" ]; then
    echo "==> Release oluşturulamadı (zaten var olabilir veya token yetersiz). Asset yüklenmedi."
    return 0
  fi
  upload_url="${upload_url%\{*}"
  echo "==> Asset yükleniyor: $asset"
  curl -sS -X POST \
    -H "Authorization: Bearer $token" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/octet-stream" \
    --data-binary "@$asset" \
    "$upload_url?name=$asset&label=$asset" >/dev/null
  echo "==> Release yayınlandı: https://github.com/$repo/releases/tag/v$VERSION"
}

# 1. Release derlemesi
echo "==> cargo build --release çalıştırılıyor (v$VERSION)..."
cargo build --release

# 2. appimagetool kontrolü ve indirme
APPIMAGETOOL=""
if command -v appimagetool >/dev/null 2>&1; then
    APPIMAGETOOL="appimagetool"
else
    # Bozuk/önceki 302 html indirmesini temizle
    if [ -f "./appimagetool-x86_64.AppImage" ] && [ "$(wc -c < ./appimagetool-x86_64.AppImage)" -lt 100000 ]; then
        rm -f ./appimagetool-x86_64.AppImage
    fi

    if [ ! -f "./appimagetool-x86_64.AppImage" ]; then
        echo "==> appimagetool indiriliyor..."
        curl -sSLO https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
        chmod +x appimagetool-x86_64.AppImage
    fi
    APPIMAGETOOL="./appimagetool-x86_64.AppImage"
fi

export APPIMAGE_EXTRACT_AND_RUN=1

# 3. AppDir dizin yapısını temizle ve oluştur
APPDIR="AnimeciX.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
mkdir -p "$APPDIR/usr/share/animecix/assets"

# 4. Binary ve ikon/asset'leri kopyala
cp target/release/animecix "$APPDIR/usr/bin/animecix"
if [ -d "assets" ]; then
    cp -r assets/* "$APPDIR/usr/share/animecix/assets/"
fi

ICON_SRC="assets/hicolor/256x256/apps/tr.com.animecix.png"
if [ -f "$ICON_SRC" ]; then
    cp "$ICON_SRC" "$APPDIR/usr/share/icons/hicolor/256x256/apps/tr.com.animecix.png"
    cp "$ICON_SRC" "$APPDIR/usr/share/icons/hicolor/256x256/apps/animecix.png"
    cp "$ICON_SRC" "$APPDIR/tr.com.animecix.png"
    cp "$ICON_SRC" "$APPDIR/animecix.png"
    cp "$ICON_SRC" "$APPDIR/.DirIcon"
fi

# 5. .desktop dosyası oluştur
cat > "$APPDIR/tr.com.animecix.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=AnimeciX
Comment=Anime, dizi ve film izleme istemcisi
Exec=animecix
Icon=tr.com.animecix
Terminal=false
Categories=AudioVideo;Video;Network;
X-AppImage-Version=$VERSION
EOF
cp "$APPDIR/tr.com.animecix.desktop" "$APPDIR/animecix.desktop"

# 6. AppRun betiği oluştur
cat > "$APPDIR/AppRun" <<'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export APPDIR="${HERE}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "${HERE}/usr/bin/animecix" "$@"
EOF

chmod +x "$APPDIR/AppRun"

# 7. AppImage paketini oluştur
echo "==> AppImage paketleniyor..."
rm -f "AnimeciX-x86_64.AppImage"
ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" "AnimeciX-x86_64.AppImage"
chmod +x "AnimeciX-x86_64.AppImage"

publish_release

echo "==> Başarılı! AppImage oluşturuldu: AnimeciX-x86_64.AppImage"
