# Değişiklik günlüğü

Biçim [Keep a Changelog](https://keepachangelog.com/) esinlidir; sürümler [SemVer](https://semver.org/) izler.

## [Yayınlanmadı]

## [0.3.0] — 2026-11-09

### Eklendi
- Tarama ilerlemesi (kategori kategori) ve **Taramayı durdur** düğmesi.
- Öğe sınırına ulaşan kategorilerde "liste kırpıldı" uyarısı.
- Geliştirici önbellekleri: pip, uv, Poetry, conda, Go, Yarn, Gradle.
- Eski program klasörlerinde **İçine bak** önizlemesi (en büyük alt klasörler, dosya sayısı, son değişiklik).
- Ayarlar: dosya yolu paylaşımı, bildirimler ve sohbet geçmişi için açma/kapama; tema (Sistem/Koyu/Açık);
  **Tanılama bilgisini kopyala** (API anahtarları gizlenir).
- Sohbet geçmişi diske yazılır (`~/.local/state/bosun/chats.json`, 0600) ve servis yeniden başlayınca geri gelir.
- Servis kapalıyken son bilinen ölçümler gösterilir.
- Yapılandırmada `stale_days` ve `large_file_mb` eşikleri.
- GitHub Actions CI (fmt, clippy, test, frontend, .deb) ve gerçek servisle socket entegrasyon testleri.

### Değişti
- journald için tek hedef: hem temizlik hem kalıcı sınır 500 MB.
- Denetim günlüğü 2 MB'ı aşınca döndürülür (en fazla ~4 MB).
- Asistana giden araç çıktıları 12 000 karakterle sınırlanır.
- Yönetici işlemi sonucu, sistemin gerçekte bildirdiğini gösterir.

### Düzeltildi
- Flatpak: sabitlenmiş (pinned) çalışma ortamları, uygulama eklentileri ve GTK temaları artık
  "kullanılmıyor" sayılmıyor; Flatpak'in kaldırmayacağı şeyler listelenmiyor.

## [0.2.0] — 2026-11-09

### Eklendi
- Sohbet sağlayıcısı seçimi: Anthropic Claude, xAI Grok, Google Gemini, OpenAI ve OpenAI uyumlu
  özel sunucular (Ollama, LM Studio). Anahtarlar Ayarlar'dan girilir, sağlayıcı başına saklanır.
- Tek tek seçilen listelerde "Tümünü seç / Seçimi temizle".
- Yardımcı kullanılamadığında nedenini (yok / root'a ait değil / başkaları yazabiliyor) ve
  düzeltme komutunu gösteren uyarı.

### Düzeltildi
- `.deb` ile kurulan yardımcı "kurulu değil" görünüyor, yönetici işlemlerinde **Devam et** pasif
  kalıyordu. systemd kullanıcı servisindeki `ProtectSystem`/`PrivateTmp` ayrı bir kullanıcı ad
  alanı açıyor ve root'a ait dosyaları `nobody` sahipli gösteriyordu. Bu seçenekler kaldırıldı;
  yardımcı durumu artık pkexec'i çalıştıran uygulama sürecinden okunuyor.

## [0.1.0] — 2026-11-09

### Eklendi
- İlk sürüm: izleme servisi (`bosund`), Tauri arayüzü, onaylı temizlik planları, denetim günlüğü.
- Faz 2: yönetici işlemleri (`bosun-helper` + polkit), ayar önerilerini uygulama/geri alma,
  Flatpak, kırık bağlantılar ve kısayollar, artık paketler, eski program klasörleri,
  sistem tepsisi, İngilizce/Türkçe arayüz, tek `.deb` paketi.
