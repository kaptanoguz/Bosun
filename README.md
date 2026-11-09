# Bosun

Linux masaüstünüzün lostromosu. Sistemi sürekli izler, ne gördüğünü seçtiğiniz dilde (İngilizce ya da Türkçe) anlatır, gereksiz dosyaları bulur, ayar önerir ve **yalnızca sizin onayınızla** temizler.

- **Genel bakış:** işlemci, bellek, swap ve disk için canlı göstergeler, son bir saatin grafiği, bulgular ve ayar önerileri.
- **Temizlik:** önbellekler (tarayıcı, Flatpak, npm, Cargo…), eski günlükler, çöp kutusu, indirilenlerdeki büyük dosyalar ve eski programlardan kalanlar (kırık bağlantılar, kırık kısayollar, sahipsiz ayar klasörleri); her biri boyutu ve güvenlik seviyesiyle.
- **Yönetici işleri:** sistem günlükleri, APT önbelleği, kullanılmayan Flatpak ortamları, artık paketler ve kaldırılmış paketlerin ayarları tek tıkla temizlenir. journald sınırı, swappiness ve Snap sürüm sayısı önerileri uygulanabilir ve Ayarlar'dan geri alınabilir. Şifreyi sisteminizin kendi penceresi sorar.
- **Sohbet:** sisteminiz hakkında konuşun. Sağlayıcı seçilebilir: Anthropic Claude, xAI Grok, Google Gemini, OpenAI ya da OpenAI uyumlu yerel bir sunucu (Ollama, LM Studio). Bosun sayıları uydurmaz, ölçümlerden okur; bir şey silmeyi önerirse onay kartı gösterir.
- **Geçmiş:** yapılan her işlemin kaydı.
- **Ayarlar:** dil seçimi (varsayılan İngilizce, Türkçe seçilebilir). Dil; arayüz, bildirimler, bulgular ve Bosun'un sohbet yanıtları için birlikte değişir.

## Mimari

```
Bosun (Tauri 2 + React)  ⇄  Unix socket (NDJSON)  ⇄  bosund (Rust, systemd --user)
                                                        ├─ örnekleyici (5 sn)
                                                        ├─ tarayıcı (10 dk)
                                                        ├─ kural motoru → notify-send
                                                        ├─ Claude ajanı (salt okuma araçları)
                                                        └─ plan → onay → çöp kutusu
```

| Klasör | İçerik |
|---|---|
| `crates/bosun-core` | Metrikler, tarama, güvenlik politikası, plan/onay, kurallar, öneriler, IPC tipleri |
| `crates/bosund` | Arka plan servisi |
| `crates/bosun-helper` | Yönetici yetkisiyle çalışan küçük yardımcı (bağımlılıksız, sabit eylem listesi) |
| `app/` | Masaüstü arayüzü (`app/src-tauri` Rust tarafı) |
| `packaging/` | systemd servisi ve örnek yapılandırma |

Ayrıntılı plan ve güvenlik modeli: [PLAN.md](PLAN.md).

## Güvenlik

- Hiçbir dosya onaysız silinmez. Plan, SHA-256 özetine bağlıdır ve 10 dakikada geçersiz olur; aynı plan iki kez uygulanamaz.
- Dosyalar çöp kutusuna taşınır. Tek istisna, açıkça "kalıcı" olarak işaretlenen çöp kutusunu boşaltma işlemidir.
- Sistem klasörleri (`/usr`, `/etc`, `/boot`, …) ve kişisel klasörler (Belgeler, Resimler, Masaüstü, …) korumalıdır. Her yol silinmeden hemen önce yeniden doğrulanır; sembolik bağlar ve `..` ile kaçışlar reddedilir.
- Asistanın elindeki araçların hiçbiri sistemi değiştirmez. Yalnızca plan önerebilir; uygulamak için arayüzdeki onay düğmesi gerekir.
- Servis root olarak çalışmaz. Yönetici yetkisi gereken işleri `/usr/lib/bosun/bosun-helper` yapar: `pkexec` ile, her seferinde şifre sorularak çalışır, yalnızca sabit bir eylem listesini kabul eder (yol, sayı ya da komut almaz) ve çalıştırmadan önce onay penceresinde tam olarak hangi komutların yürütüleceğini görürsünüz. Yardımcı root'a ait değilse ya da başkaları yazabiliyorsa Bosun onu çalıştırmaz.
- Yardımcı `/usr/bin/pkexec` (yoksa `/bin/pkexec`) ile çalıştırılır; PATH'e bilerek bakılmaz, çünkü yönetici yetkisi isteyen programın başkasının yerleştirebileceği bir yoldan bulunması istenmez.
- `~/.config`, `~/.local/share` ve `~/.cache` gibi klasörlerin **kendisi** korumalıdır; Bosun yalnızca içlerindeki öğeleri, her biri silinmeden hemen önce yeniden doğrulanarak işler (yol kanonikleştirilir, sembolik bağlar ve `..` reddedilir).
- Kırık bağlantılarda yalnızca bağlantının kendisi silinir. "Eski programlardan kalmış olabilecek klasörler" bir tahmindir; hiçbiri otomatik seçilmez ve seçilenler çöp kutusuna gider.
- Yapay zekâ sağlayıcısına dosya içerikleri gönderilmez; büyük dosya yolları yalnızca `share_file_paths = true` ise paylaşılır.
- Socket `0600` izinlidir ve yalnızca aynı kullanıcıdan gelen bağlantıları kabul eder.

## Kurulum (Linux Mint / Ubuntu)

```bash
git clone https://github.com/kaptanoguz/Bosun.git && cd Bosun
./install.sh --app                                   # her şeyi içeren .deb paketini derler
sudo apt install ./target/release/bundle/deb/Bosun_*.deb
systemctl --user enable --now bosund                 # ya da uygulamadaki "Start service" düğmesi
```

Paket şunları kurar: uygulama, `/usr/bin/bosund`, `/usr/lib/bosun/bosun-helper`, polkit kuralı ve systemd kullanıcı servisi.

Paketsiz kurulum: `./install.sh` (yalnızca servis) ya da `./install.sh --helper` (servis + yönetici yardımcısı).

Uygulama penceresini kapattığınızda Bosun sistem tepsisinde çalışmaya devam eder; çıkmak için tepsi menüsünden **Quit**.

### Dil

Uygulama İngilizce açılır. **Settings → Language → Türkçe** ile değiştirilebilir; seçim `~/.config/bosun/config.toml` dosyasına `language = "tr"` olarak yazılır (dosyadaki diğer ayarlar korunur).

### Sohbeti açmak

**Settings → Chat** bölümünde sağlayıcıyı seçin ve API anahtarını yapıştırın:

| Sağlayıcı | Varsayılan model | Not |
|---|---|---|
| Anthropic Claude | `claude-sonnet-5-5` | |
| xAI Grok | `grok-4.5` | OpenAI uyumlu API (`https://api.x.ai/v1`) |
| Google Gemini | `gemini-3.8-flash` | Gemini'nin OpenAI uyumlu uç noktası |
| OpenAI | `gpt-5` | |
| Özel sunucu | `llama3.1` | Ör. yerel Ollama: `http://localhost:11434/v1`; anahtar gerekmez |

Model alanı boş bırakılırsa varsayılan kullanılır; istediğiniz model adını yazabilirsiniz. Anahtarlar `~/.config/bosun/config.toml` dosyasında (`chmod 600`) sağlayıcı başına saklanır ve arayüze asla geri gönderilmez. Yerel bir model seçerseniz hiçbir veri bilgisayarınızdan çıkmaz.

## Sorun bildirme

**Settings → Report a problem → Copy diagnostics** sürümleri, servis durumunu ve son günlük satırlarını panoya kopyalar (API anahtarları gizlenir). Bunu bir GitHub issue'ya yapıştırabilirsiniz.

## Geliştirme

```bash
cargo test --workspace         # birim testleri + gerçek servisle socket entegrasyon testleri
cargo run -p bosund            # servisi ön planda çalıştır (BOSUN_LOG=debug)
cd app && npm install
npm run dev                    # arayüz tarayıcıda, sahte verilerle
npm run tauri dev              # gerçek masaüstü uygulaması (servis çalışıyor olmalı)
```

## Yol haritası

- ~~**Faz 2:** yönetici işleri, ayar önerilerini uygulama/geri alma, Flatpak, kalıntılar, sistem tepsisi, tam .deb~~ ✓
- **Faz 3:** zamanlanmış bakım raporu, yinelenen dosya bulucu, eski çekirdekler, yerel model (Ollama) seçeneği.

## Lisans

MIT
