# Bosun

Linux masaüstünüzün lostromosu. Sistemi sürekli izler, ne gördüğünü Türkçe anlatır, gereksiz dosyaları bulur, ayar önerir ve **yalnızca sizin onayınızla** temizler.

- **Genel bakış:** işlemci, bellek, swap ve disk için canlı göstergeler, son bir saatin grafiği, bulgular ve ayar önerileri.
- **Temizlik:** önbellekler, eski günlükler, çöp kutusu ve indirilenlerdeki büyük dosyalar; her biri boyutu ve güvenlik seviyesiyle.
- **Sohbet:** Claude ile sisteminiz hakkında konuşun. Bosun sayıları uydurmaz, ölçümlerden okur; bir şey silmeyi önerirse onay kartı gösterir.
- **Geçmiş:** yapılan her işlemin kaydı.

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
| `app/` | Masaüstü arayüzü (`app/src-tauri` Rust tarafı) |
| `packaging/` | systemd servisi ve örnek yapılandırma |

Ayrıntılı plan ve güvenlik modeli: [PLAN.md](PLAN.md).

## Güvenlik

- Hiçbir dosya onaysız silinmez. Plan, SHA-256 özetine bağlıdır ve 10 dakikada geçersiz olur; aynı plan iki kez uygulanamaz.
- Dosyalar çöp kutusuna taşınır. Tek istisna, açıkça "kalıcı" olarak işaretlenen çöp kutusunu boşaltma işlemidir.
- Sistem klasörleri (`/usr`, `/etc`, `/boot`, …) ve kişisel klasörler (Belgeler, Resimler, Masaüstü, …) korumalıdır. Her yol silinmeden hemen önce yeniden doğrulanır; sembolik bağlar ve `..` ile kaçışlar reddedilir.
- Claude'un elindeki araçların hiçbiri sistemi değiştirmez. Yalnızca plan önerebilir; uygulamak için arayüzdeki onay düğmesi gerekir.
- Servis root olarak çalışmaz. Yönetici yetkisi gereken işler için Bosun size çalıştırabileceğiniz komutu verir.
- Claude'a dosya içerikleri gönderilmez; büyük dosya yolları yalnızca `share_file_paths = true` ise paylaşılır.
- Socket `0600` izinlidir ve yalnızca aynı kullanıcıdan gelen bağlantıları kabul eder.

## Kurulum (Linux Mint / Ubuntu)

```bash
git clone https://github.com/kaptanoguz/Bosun.git && cd Bosun
./install.sh --app          # servisi kurar ve arayüzü .deb olarak derler
sudo apt install ./target/release/bundle/deb/Bosun_*.deb
```

Yalnızca servisi kurmak için `./install.sh`.

### Sohbeti açmak

`~/.config/bosun/config.toml` dosyasına anahtarınızı ekleyin ve servisi yeniden başlatın:

```toml
api_key = "sk-ant-..."
```

```bash
systemctl --user restart bosund
```

## Geliştirme

```bash
cargo test                     # tüm Rust testleri
cargo run -p bosund            # servisi ön planda çalıştır (BOSUN_LOG=debug)
cd app && npm install
npm run dev                    # arayüz tarayıcıda, sahte verilerle
npm run tauri dev              # gerçek masaüstü uygulaması (servis çalışıyor olmalı)
```

## Yol haritası

- **Faz 2:** pkexec/polkit ile journald ve APT temizliği, ayar önerilerini tek tıkla uygulama, sistem tepsisi.
- **Faz 3:** zamanlanmış bakım raporu, yinelenen dosya bulucu, eski çekirdekler, yerel model (Ollama) seçeneği.

## Lisans

MIT
