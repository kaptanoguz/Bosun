#!/usr/bin/env bash
# Bosun kurulumu: servisi derler, ~/.local/bin altına koyar ve systemd kullanıcı servisi olarak başlatır.
# Arayüzü de derlemek için:  ./install.sh --app
set -euo pipefail
cd "$(dirname "$0")"

if [[ $EUID -eq 0 ]]; then
  echo "Bu betiği sudo ile değil, kendi kullanıcınızla çalıştırın." >&2
  exit 1
fi

need() { command -v "$1" >/dev/null || { echo "Eksik: $1 — $2" >&2; exit 1; }; }
need cargo "https://rustup.rs adresinden Rust kurun"

echo "› bosund derleniyor…"
cargo build --release -p bosund

mkdir -p ~/.local/bin ~/.config/systemd/user ~/.config/bosun
install -m 755 target/release/bosund ~/.local/bin/bosund
install -m 644 packaging/bosund.service ~/.config/systemd/user/bosund.service

if [[ ! -f ~/.config/bosun/config.toml ]]; then
  install -m 600 packaging/config.example.toml ~/.config/bosun/config.toml
  echo "› Örnek yapılandırma yazıldı: ~/.config/bosun/config.toml"
fi

systemctl --user daemon-reload
systemctl --user enable --now bosund.service
systemctl --user restart bosund.service
echo "› Servis çalışıyor: $(systemctl --user is-active bosund.service)"

if [[ "${1:-}" == "--app" ]]; then
  need npm "Node.js 20+ kurun"
  if ! pkg-config --exists webkit2gtk-4.1; then
    echo "› Tauri bağımlılıkları kuruluyor (sudo şifresi istenebilir)…"
    sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev build-essential libssl-dev
  fi
  echo "› Arayüz derleniyor…"
  (cd app && npm ci && npm run tauri build -- --bundles deb)
  deb=$(ls -t target/release/bundle/deb/*.deb | head -1)
  echo "› Paket hazır: $deb"
  echo "  Kurmak için: sudo apt install ./$deb"
fi

echo
echo "Sohbet için API anahtarınızı ~/.config/bosun/config.toml dosyasına ekleyip"
echo "  systemctl --user restart bosund"
echo "komutunu çalıştırın. İzleme ve temizlik anahtarsız da çalışır."
