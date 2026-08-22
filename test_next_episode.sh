#!/usr/bin/env bash
# Animecix sonraki/önceki bölüm (n/p) geçişi testi.
# - Tüm birim testlerini çalıştırır
# - Deadlock'a karşı özel integration testini (tests/next_episode.rs) çalıştırır
# Kullanım: bash test_next_episode.sh
set -euo pipefail

cd "$(dirname "$0")"

echo "==> Derleniyor..."
cargo build 2>&1 | tail -2

echo "==> Birim + entegrasyon testleri çalıştırılıyor..."
cargo test --test next_episode 2>&1 | tee /tmp/animecix_next_ep_test.log

echo "==> Tüm test paketi (ui/api) çalıştırılıyor..."
cargo test 2>&1 | tail -20

if grep -q "DEADLOCK" /tmp/animecix_next_ep_test.log; then
    echo ""
    echo "[HATA] Deadlock tespit edildi! bkz: /tmp/animecix_next_ep_test.log"
    exit 1
fi

echo ""
echo "[OK] Sonraki/önceki bölüm geçişi testleri başarılı (deadlock yok)."
