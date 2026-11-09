#!/usr/bin/env bash
# Bosun kurulumu (kaynak koddan).
#
#   ./install.sh            servisi derler, ~/.local/bin altına koyar, systemd kullanıcı servisi olarak başlatır
#   ./install.sh --helper   ayrıca yönetici yardımcısını ve polkit kuralını kurar (sudo ister)
#   ./install.sh --app      her şeyi içeren .deb paketini derler (önerilen yol)
set -euo pipefail
cd "$(dirname "$0")"

if [[ $EUID -eq 0 ]]; then
  echo "Bu betiği sudo ile değil, kendi kullanıcınızla çalıştırın." >&2
  exit 1
fi

need() { command -v "$1" >/dev/null || { echo "Eksik: $1 — $2" >&2; exit 1; }; }
need cargo "https://rustup.rs adresinden Rust kurun"

install_service() {
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
}

install_helper() {
  echo "› Yönetici yardımcısı derleniyor…"
  cargo build --release -p bosun-helper
  echo "› Yardımcı ve polkit kuralı kuruluyor (sudo şifresi istenecek)…"
  sudo install -D -o root -g root -m 755 target/release/bosun-helper /usr/lib/bosun/bosun-helper
  sudo install -D -o root -g root -m 644 packaging/com.kaptanoguz.bosun.policy /usr/share/polkit-1/actions/com.kaptanoguz.bosun.policy
  echo "› Kuruldu: /usr/lib/bosun/bosun-helper"
}

build_deb() {
  if ! pkg-config --exists 'gtk4 >= 4.14' 'libadwaita-1 >= 1.5'; then
    echo "› GTK 4 ve libadwaita geliştirme paketleri kuruluyor (sudo şifresi istenebilir)…"
    sudo apt-get install -y libgtk-4-dev libadwaita-1-dev build-essential
  fi
  echo "› Paket derleniyor (ilk seferde birkaç dakika sürer)…"
  deb=$(packaging/build-deb.sh | tail -1)
  echo
  echo "› Paket hazır: $deb"
  echo "  Kurmak için:   sudo apt install ./$deb"
  echo "  Sonra:         systemctl --user enable --now bosund"
  echo "  (Uygulamadaki 'Start service' düğmesi de aynısını yapar.)"
}

case "${1:-}" in
  --app) build_deb ;;
  --helper) install_service; install_helper ;;
  "") install_service ;;
  *) echo "Kullanım: ./install.sh [--helper|--app]" >&2; exit 2 ;;
esac

echo
echo "Sohbet için API anahtarınızı ~/.config/bosun/config.toml dosyasına ekleyip"
echo "  systemctl --user restart bosund"
echo "komutunu çalıştırın. İzleme ve temizlik anahtarsız da çalışır."
