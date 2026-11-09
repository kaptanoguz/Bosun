#!/usr/bin/env bash
# Bosun .deb paketini derler: GTK uygulaması, arka plan servisi, yönetici yardımcısı,
# polkit kuralı, systemd kullanıcı birimi, masaüstü girdisi ve simgeler.
#
#   packaging/build-deb.sh        → target/debian/bosun_<sürüm>_amd64.deb
#
# Gerekenler: cargo, dpkg-deb, libgtk-4-dev (>= 4.14) ve libadwaita-1-dev (>= 1.5).
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
arch=$(dpkg --print-architecture)
cargo build --release --locked -p bosun-gtk -p bosund -p bosun-helper

root=target/debian/root
rm -rf "$root"
install -Dm755 target/release/bosun "$root/usr/bin/bosun"
install -Dm755 target/release/bosund "$root/usr/bin/bosund"
install -Dm755 target/release/bosun-helper "$root/usr/lib/bosun/bosun-helper"
install -Dm644 packaging/bosund-system.service "$root/usr/lib/systemd/user/bosund.service"
install -Dm644 packaging/com.kaptanoguz.bosun.policy "$root/usr/share/polkit-1/actions/com.kaptanoguz.bosun.policy"
install -Dm644 packaging/io.github.kaptanoguz.Bosun.desktop "$root/usr/share/applications/io.github.kaptanoguz.Bosun.desktop"
install -Dm644 packaging/icons/bosun.svg "$root/usr/share/icons/hicolor/scalable/apps/bosun.svg"
for s in 32 64 128 256; do
  install -Dm644 "packaging/icons/bosun-$s.png" "$root/usr/share/icons/hicolor/${s}x${s}/apps/bosun.png"
done
install -Dm644 LICENSE "$root/usr/share/doc/bosun/copyright"
install -Dm644 packaging/config.example.toml "$root/usr/share/doc/bosun/config.example.toml"

size=$(du -sk "$root" | cut -f1)
mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CONTROL
Package: bosun
Version: $version
Architecture: $arch
Maintainer: Oğuz <kaptanoguz@users.noreply.github.com>
Installed-Size: $size
Depends: libc6 (>= 2.39), libgtk-4-1 (>= 4.14), libadwaita-1-0 (>= 1.5), pkexec | policykit-1, libnotify-bin
Section: utils
Priority: optional
Homepage: https://github.com/kaptanoguz/Bosun
Description: Maintenance assistant for the Linux desktop
 Bosun watches the system, explains what it sees, finds what can be cleaned
 and suggests better settings. It changes nothing until you approve.
CONTROL

# Kurulumdan sonra simge ve masaüstü önbelleklerini tazele (araçlar yoksa sessizce geç).
cat > "$root/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
  command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
fi
exit 0
POSTINST
chmod 755 "$root/DEBIAN/postinst"

out="target/debian/bosun_${version}_${arch}.deb"
dpkg-deb --root-owner-group --build "$root" "$out" >/dev/null
echo "$out"
