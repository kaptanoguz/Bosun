# Bosun — Detaylı Proje Planı

> Bosun (lostromo): gemide güverte bakımından sorumlu kişi. Bu uygulama da Linux sisteminin
> bakımını yapar: sürekli izler, ne gördüğünü anlatır, öneri verir ve **yalnızca onay alarak** temizler.

## 1. Hedef ve kapsam

| Hedef | Ölçüt |
|---|---|
| Sistemi sürekli izlemek | Arayüz kapalıyken bile çalışan systemd kullanıcı servisi |
| Sorunları erkenden haber vermek | Eşik aşımında masaüstü bildirimi, tekrar eden bildirim yok |
| Konuşarak açıklamak | Claude ile Türkçe sohbet; model yalnızca tanımlı araçları kullanır |
| Güvenli temizlik | Hiçbir dosya onaysız silinmez; silinenler çöp kutusuna gider; her işlem loglanır |
| Ayar önerileri | journald boyutu, snap sürüm sayısı, swappiness vb. için gerekçeli öneriler |
| Hafiflik | Daemon < 30 MB RAM, boşta ~%0 CPU |

**Kapsam dışı (bilinçli olarak):** root olarak çalışmak, serbest shell komutu çalıştırmak,
sistem dosyalarını (`/usr`, `/etc`, `/boot`, `/var/lib`) değiştirmek, paket kaldırmak (Faz 3'e kadar).

## 2. Mimari

```
┌──────────────────────────────┐         ┌───────────────────────────────────────┐
│  Bosun (GTK 4 + libadwaita)  │         │  bosund (Rust, systemd --user)         │
│                              │  Unix   │                                       │
│  Genel bakış · Temizlik      │ socket  │  Örnekleyici (5 sn) ── Halka tampon     │
│  Onay · Sohbet · Geçmiş      │◀──────▶│  Tarayıcı (10 dk) ──── Temizlik adayları │
│                              │ NDJSON  │  Kural motoru ──────── notify-send      │
│  Yalnızca izin listesindeki  │         │  Claude ajanı (tool use, salt okuma)    │
│  IPC yöntemleri              │         │  Plan deposu ─ Onay ─ Uygulayıcı        │
└──────────────────────────────┘         │  Denetim günlüğü (JSONL)              │
                                         └───────────────────────────────────────┘
                 bosun-core (ortak kütüphane): veri modelleri, toplayıcılar,
                 temizlik kategorileri, güvenlik politikası, IPC protokolü
```

### Cargo workspace

```
bosun/
├── crates/
│   ├── bosun-core/     # saf mantık, I/O soyut; test edilebilir
│   │   ├── metrics.rs      # sysinfo ile CPU/RAM/swap/disk/sıcaklık/uptime
│   │   ├── scan.rs         # temizlik kategorilerinin boyut taraması
│   │   ├── categories.rs   # kategori tanımları + güvenlik seviyeleri
│   │   ├── policy.rs       # korumalı yollar, yol doğrulama
│   │   ├── plan.rs         # CleanupPlan, onay belirteci, son kullanma
│   │   ├── execute.rs      # planı uygulama (çöp kutusu / journal vacuum)
│   │   ├── rules.rs        # eşik kuralları → Bulgu (Finding)
│   │   ├── advice.rs       # ayar önerileri (sysctl, journald, snap)
│   │   └── ipc.rs          # istek/yanıt/olay tipleri
│   └── bosund/         # daemon
│       ├── main.rs         # tokio, socket dinleme, döngüler
│       ├── state.rs        # paylaşılan durum (RwLock)
│       ├── notify.rs       # notify-send, tekrarlama koruması
│       ├── audit.rs        # ~/.local/state/bosun/audit.jsonl
│       └── agent.rs        # Claude Messages API + tool-use döngüsü
│   ├── bosun-helper/   # root yardımcısı (pkexec, sabit eylemler)
│   └── bosun-gtk/      # masaüstü uygulaması (gtk-rs + libadwaita), locales/*.json
├── packaging/
│   ├── build-deb.sh    # .deb paketi
│   └── bosund.service  # systemd kullanıcı servisi
└── PLAN.md
```

## 3. Güvenlik modeli (pazarlığa kapalı kurallar)

1. **İki katmanlı yetki.** Model ve arayüz yalnızca *okuyabilir* ve *plan önerebilir*.
   Planı **uygulamak** için arayüzde kullanıcının bastığı "Onayla" düğmesi gerekir.
   Claude'un elinde "uygula" aracı **yoktur**.
2. **Serbest shell yok.** Her aksiyon kodda tanımlı bir kategoriye karşılık gelir.
   Model yol uyduramaz; yalnızca tarayıcının bulduğu kategori kimliklerini seçebilir.
3. **Korumalı yollar.** `/`, `/usr`, `/etc`, `/boot`, `/var/lib`, `/bin`, `/sbin`, `/lib*`,
   `/proc`, `/sys`, `/dev`, `$HOME` kökü ve kişisel klasörler (Belgeler, Resimler, Masaüstü, …)
   hiçbir kategori tarafından silinemez. Tüm yollar `canonicalize` edilir, sembolik bağlar takip
   edilmez (symlink ile kaçış engellenir).
4. **Geri alınabilirlik.** Dosya silme = XDG çöp kutusu (`trash` crate). journald için
   `journalctl --vacuum-size` (kalıcı ama güvenli, kullanıcıya açıkça belirtilir).
5. **Plan bütünlüğü.** Plan oluşturulduğunda içeriği SHA-256 ile özetlenir; onay bu özete
   bağlıdır ve 10 dk sonra geçersizdir. Plan ile onay arasında dosyalar değişirse uygulama
   yalnızca hâlâ kategoride olan dosyaları işler.
6. **Root yok.** Yetki gerektiren işlemler (apt önbelleği) Faz 2'de `pkexec` + polkit kuralı
   ile tek komuta sınırlanır. Faz 1'de bunlar yalnızca *öneri* olarak gösterilir.
7. **Denetim günlüğü.** Her plan, onay ve uygulama sonucu `audit.jsonl`'a eklenir (append-only).
8. **Gizlilik.** API'ye yalnızca özet metrikler ve kategori boyutları gider; dosya içerikleri
   asla gönderilmez. "Büyük dosyalar" listesinde yol gönderimi ayarlardan kapatılabilir.
   API anahtarı `~/.config/bosun/config.toml` (0600) içinde ya da `ANTHROPIC_API_KEY` ortam
   değişkeninde tutulur.
9. **Socket güvenliği.** `$XDG_RUNTIME_DIR/bosun.sock`, izin 0600; bağlanan sürecin UID'si
   `SO_PEERCRED` ile doğrulanır.

## 4. Temizlik kategorileri (Faz 1–2)

| Kimlik | Konum | Seviye | Yöntem | Yetki |
|---|---|---|---|---|
| `thumbnails` | `~/.cache/thumbnails` | Güvenli | Çöp kutusu | Kullanıcı |
| `user_cache` | `~/.cache/*` (tarayıcılar hariç alt klasörler, 7 günden eski) | Dikkat | Çöp kutusu | Kullanıcı |
| `browser_cache` | Firefox/Chromium/LibreWolf cache klasörleri | Güvenli | Çöp kutusu | Kullanıcı |
| `trash` | `~/.local/share/Trash` | Dikkat (kalıcı) | Kalıcı silme | Kullanıcı |
| `flatpak_cache` | `~/.var/app/*/cache` (tarayıcılar hariç) | Güvenli | Çöp kutusu | Kullanıcı |
| `flatpak_unused` | `/var/lib/flatpak/runtime` | Güvenli | `flatpak uninstall --unused` | pkexec |
| `broken_links` | Ev klasörü (derinlik 5, büyük klasörler hariç) | Güvenli | Yalnızca bağlantıyı sil | Kullanıcı |
| `orphan_launchers` | `~/.local/share/applications`, `~/.config/autostart` | Dikkat | Çöp kutusu | Kullanıcı |
| `leftover_configs` | `~/.config`, `~/.local/share` | Tek tek seçilir | Çöp kutusu | Kullanıcı |
| `orphan_packages` | `apt-get -s autoremove` | Dikkat | `apt-get autoremove --purge` | pkexec |
| `residual_packages` | dpkg `rc` durumu | Güvenli | `dpkg --purge` | pkexec |
| `journal` | `/var/log/journal` | Güvenli | `journalctl --vacuum-size=200M` | pkexec (Faz 2) |
| `apt_cache` | `/var/cache/apt/archives/*.deb` | Güvenli | `apt-get clean` | pkexec (Faz 2) |
| `npm_cache` / `cargo_cache` | `~/.npm/_cacache`, `~/.cargo/registry/cache` | Güvenli | Çöp kutusu | Kullanıcı |
| `old_logs` | `~/.local/state/**/*.log` > 30 gün | Dikkat | Çöp kutusu | Kullanıcı |
| `large_files` | `~/Downloads`, `~/İndirilenler` içinde > 500 MB | Yalnızca bilgi | Kullanıcı tek tek seçer | Kullanıcı |

## 5. IPC protokolü

Satır başına bir JSON (NDJSON). İstek: `{"id":1,"method":"snapshot","params":{}}`
Yanıt: `{"id":1,"ok":true,"result":{...}}` · Olay (sunucudan itme): `{"event":"metrics","data":{...}}`

| Yöntem | Açıklama |
|---|---|
| `snapshot` | Anlık metrikler + son 1 saatlik geçmiş |
| `scan` | Temizlik adaylarını (yeniden) tara |
| `findings` | Kural motorunun güncel bulguları + ayar önerileri |
| `plan.create {categories}` | Plan oluştur → plan, özet, son kullanma |
| `plan.approve {plan_id, digest}` | Planı uygula (yalnızca arayüzden) |
| `history` | Denetim günlüğünden son N kayıt |
| `chat {conversation_id, message}` | Claude ile konuş; yanıt + varsa önerilen plan |
| `settings.get` / `settings.set {language}` | Ayarları oku / dili değiştir |
| `subscribe` | Olay akışını aç |

## 6. Claude ajanı

- Model: yapılandırılabilir (varsayılan Sonnet sınıfı; ucuz/hızlı).
- Sistem istemi: Türkçe, kısa, somut sayılarla konuşan bir "lostromo"; asla komut uydurmaz,
  silmeyi kendisi yapamayacağını bilir, öneriyi `propose_cleanup` ile sunar.
- **Araçlar (hepsi salt okuma):**
  - `get_system_snapshot` — CPU/RAM/swap/disk/sıcaklık
  - `get_cleanup_candidates` — kategoriler, boyutlar, seviyeler
  - `get_findings` — uyarılar ve ayar önerileri
  - `get_failed_services` — `systemctl --user/--system --failed`
  - `get_large_files` — (gizlilik ayarı açıksa)
  - `propose_cleanup {categories, reason}` — plan oluşturur, **uygulamaz**; arayüzde onay kartı çıkar
- Döngü en fazla 6 tur; zaman aşımı 60 sn; API anahtarı yoksa sohbet ekranı ayar yönergesi gösterir,
  geri kalan her şey (izleme, kurallar, temizlik) LLM olmadan çalışır.

## 7. Kural motoru (LLM'siz, her 5 sn)

| Kural | Eşik | Önem |
|---|---|---|
| Disk doluluğu | ≥ %85 uyarı, ≥ %95 kritik | Sarı / Kırmızı |
| RAM | ≥ %90 (5 dk boyunca) | Sarı |
| Swap | ≥ %50 | Sarı |
| Sıcaklık | ≥ 85 °C | Kırmızı |
| Temizlenebilir alan | ≥ 2 GB | Bilgi |
| Başarısız servis | ≥ 1 | Sarı |

Bildirimler aynı bulgu için 6 saatte bir defadan fazla gönderilmez.

## 8. Arayüz

- **Durum çubuğu:** sağlık rozeti (yeşil/sarı/kırmızı), daemon bağlantısı, son tarama.
- **Genel bakış:** CPU, RAM, swap, disk kartları + 1 saatlik çizgi grafikler; bulgular listesi.
- **Temizlik:** kategori satırları (boyut, seviye, gerekçe) → seçim → "Planı gözden geçir".
- **Onay:** planın içeriği, toplam boyut, geri alınabilirlik; "Onayla" / "Vazgeç"; sonuç özeti.
- **Sohbet:** mesajlar; asistan bir plan önerdiğinde mesaj içinde onay kartı.
- **Geçmiş:** denetim günlüğü.
- **Ayarlar:** dil seçimi ve yapılandırma özeti.
- Tema: koyu varsayılan, sistem temasını takip eder.
- Dil: varsayılan İngilizce, Ayarlar'dan Türkçe. Seçim servis yapılandırmasında (`language`) tutulur; servis kategori adlarını, bulguları, önerileri ve asistanın yanıt dilini buna göre üretir (`settings.get` / `settings.set`).

## 9. Fazlar ve kabul ölçütleri

### Faz 1 — Temel (bu teslimat)
- [x] Workspace, core, daemon, Tauri iskeleti
- [x] Metrikler, tarama, kurallar, bildirim, socket IPC
- [x] Kullanıcı seviyesindeki kategoriler için plan → onay → çöp kutusu
- [x] Claude sohbeti + salt okuma araçları + `propose_cleanup`
- [x] Denetim günlüğü
- Kabul: `cargo test` geçer; daemon gerçek makinede çalışır; arayüz canlı veri gösterir;
  korumalı yol testleri geçer.

### Faz 2 — Yetkili işlemler, ayarlar ve kalıntılar ✓
- `bosun-helper`: bağımlılıksız, sabit eylemli root yardımcısı; `pkexec` + polkit (`auth_admin_keep`).
  Eylemler: `journal-vacuum`, `apt-clean`, `flatpak-unused`, `apt-autoremove`, `purge-residual`,
  `journald-limit`/`-undo`, `swappiness`/`-undo`, `snap-retain`/`-undo`.
- pkexec'i grafik oturumdaki arayüz çağırır (polkit ajanı orada); servis yalnızca sonucu kaydeder
  (`admin.report`) ve yeniden tarar. Arayüz yardımcıyı yalnızca root'a aitse ve başkası yazamıyorsa çalıştırır.
- Onay penceresi: ne yapılacağı, root olarak çalışacak komutlar, etkilenen öğeler (ör. paket listesi).
- Ayar önerilerini uygulama ve Ayarlar > "Bosun'un yaptığı değişiklikler"den geri alma.
- Flatpak: uygulama önbellekleri (kullanıcı, çöp kutusu) ve kullanılmayan çalışma ortamları (yönetici).
- Kalıntılar: kırık sembolik bağlantılar (yalnızca bağlantı silinir), programı olmayan `.desktop`
  kısayolları, `autoremove` adayları, `rc` durumundaki paketler, sahipsiz görünen ayar klasörleri
  (tahmin; asla otomatik seçilmez).
- Sistem tepsisi; pencere kapatılınca tepside kalır.
- Tek `.deb`: uygulama + bosund + helper + polkit kuralı + systemd kullanıcı birimi.

### Faz 2.1 — Çoklu sağlayıcı ✓
- `provider`: anthropic, xai, google, openai, custom; `[api_keys]` sağlayıcı başına.
- Anthropic Messages API ve OpenAI uyumlu Chat Completions (Grok, Gemini, OpenAI, Ollama…) için
  ayrı araç-kullanım döngüleri; sohbet geçmişi sağlayıcının biçiminde tutulur, sağlayıcı değişince sıfırlanır.
- Anahtar arayüzden girilir, `settings.set` ile 0600 dosyaya yazılır, asla geri okunmaz.
  Uzak sunucularda yalnızca `https://` kabul edilir (`http` yalnızca localhost).
- Düzeltme: systemd kullanıcı servisindeki `ProtectSystem`/`PrivateTmp` ayrı bir kullanıcı ad alanı
  açıyor ve root'a ait yardımcıyı "nobody" sahipli gösteriyordu; bu seçenekler kaldırıldı ve
  yardımcı durumu artık pkexec'i çalıştıran uygulama sürecinden okunuyor.

### Faz 2.2 — Denetim düzeltmeleri (0.3.0) ✓
- Tarama ilerlemesi/iptali, kırpma uyarısı, geliştirici önbellekleri, klasör önizlemesi.
- Ayarlanabilir `stale_days` / `large_file_mb`, tek journald hedefi, denetim günlüğü rotasyonu,
  kalıcı sohbet, araç çıktısı sınırı, ayar anahtarları, tema, tanılama, çevrimdışı görünüm.
- CI ve socket entegrasyon testleri. IPC hız sınırlaması bilerek eklenmedi: socket 0600 ve
  yalnızca aynı UID'ye açık.

### Faz 3 — Genişleme ✓ (0.3.1–0.4.0)
- Akışlı sohbet: SSE ayrıştırma (`stream.rs`), `chat_delta` / `chat_tool` olayları.
- Temizlik listesinde arama/filtre; bulgu başına sessize alma (`muted_findings`).
- Steam gölgelendirici önbellekleri; Docker/Podman sarkan imajlar + derleme önbelleği (sabit komutlar).
- Ağ hızı, pil, GPU (NVIDIA/AMD), düşük pil bulgusu.
- AppImage: uygulama ikilisi `--daemon` ile servis olur; birim dosyasını uygulama yazar.
- Sürüm iş akışı: .deb + AppImage, SHA256SUMS, isteğe bağlı GPG imzası.

### Faz 4 — Sonraki
- Zamanlanmış bakım raporu (haftalık özet bildirimi)
- Yinelenen dosya bulucu
- Eski çekirdekler için ayrıntılı görünüm
- Zamanlanmış bakım (haftalık rapor), yinelenen dosya bulucu, eski çekirdekler
- .deb / AppImage paketleme, imzalı sürüm

## 10. Riskler

| Risk | Önlem |
|---|---|
| Yanlış dosyanın silinmesi | Korumalı yollar + canonicalize + çöp kutusu + onay + testler |
| LLM'nin yanlış bilgi vermesi | Model sayıları yalnızca araç çıktısından alır; arayüz gerçek veriyi ayrıca gösterir |
| Büyük dizinlerde yavaş tarama | Arka planda, derinlik sınırlı, `spawn_blocking` |
| API maliyeti | LLM yalnızca sohbette; izleme LLM'siz |
| GTK sürüm farkları | GTK 4.14 + libadwaita 1.5 (Ubuntu 24.04 / Mint 22 depolarında) |
