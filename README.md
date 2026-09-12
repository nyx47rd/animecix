# AnimeciX Linux 

[![Web](https://img.shields.io/badge/Web-nyx47rd.github.io%2Fanimecix-0969da?style=flat-square)](https://nyx47rd.github.io/animecix/)
[![Releases](https://img.shields.io/github/v/release/nyx47rd/animecix?style=flat-square)](https://github.com/nyx47rd/animecix/releases/latest)
[![License: MIT](https://img.shields.io/github/license/nyx47rd/animecix?style=flat-square)](LICENSE)

<img src="assets/hicolor/256x256/apps/tr.com.animecix.png" align="right" width="96" height="96" alt="AnimeciX">

**Linux için GTK4/libadwaita ile yazılmış anime, dizi ve film izleme masaüstü istemcisi.**

Tek dosyalık taşınabilir **AppImage** olarak dağıtılır; uygulama başlatmada yeni sürümü
kontrol eder ve kendini otomatik güncelleyebilir.

> ⚠️ Bu uygulama **gayriresmî** ve eğitim/kişisel kullanım amaçlıdır; herkese açık bir API
> kullanır. Hizmet sağlayıcıya zarar vermeden, kendi sorumluluğunda kullanın.

---

## Özellikler

- **İndirme yöneticisi**: bölüm ve film indirme; 6 bağlantıyla hızlı indirme, kaldığı yerden devam, toplu indirme sihirbazı
- **Araçlar menüsü**: Favoriler, Maraton, Geçmiş, İndirilenler ve Ayarlar tek menüde; Ctrl+T ile anında erişim (kısayol değiştirilebilir)
- **5 koyu tema**: Koyu, Bordo, Orman, Lacivert, Mor; karşılama ekranında canlı önizleme
- **Hızlı arama**: ana ekranda ortalı arama çubuğu (Ctrl+S); bölüm ekranında hızlı bölüm arama
- **Oynatıcı**: MPV ile oynatma, otomatik tam ekran, resmi intro/outro atlama (S/E), çalan şarkı bilgisi ve Shift+M ile tarayıcıda açma, isteğe bağlı oynatma kalite seçici
- **Takip**: Favoriler, sürükle-bırak sıralamalı Maraton, izleme Geçmişi, kaldığın yerden devam
- **Akıllı kaynak seçimi**: kaynaklar paralel çözülür, en kalitelisi seçilir; ölü kaynak elenir, açılmayan kaynakta sonrakine geçilir. manuel kaynak seçimi mevcuttur.
- **Otomatik güncelleme**: AppImage sürümü başlatmada yeni sürümü denetler, tek tıkla günceller
- **Kurulum sihirbazı**: bağımlılık kontrolü ve masaüstü başlatıcı kurulumu
- **Ayarlar**: tema, kısayollar, indirme klasörü, masaüstü başlatıcı, veri sıfırlama
- **VPN proxy desteği** (isteğe bağlı): yerelde çalışan bir proxy varsa
  (`127.0.0.1:10808`, ör. sing-box + ProtonVPN WireGuard) video trafiğini oradan çıkarır ve
  ISS kısıtlamalarını aşar; proxy kapalıysa uygulama normal çalışır, hiçbir şey bozulmaz
- **Hızlı yükleme**: HTTP/2 multiplexing, bağlantı havuzu (keep-alive) ve DNS önbelleği;
  kapak görselleri paralel (12 worker) indirilir

---

## Ekran Görüntüleri

<p align="center">
  <img src="screenshots/temalar.png" width="100%">
  <b>5 koyu tema: Koyu, Bordo, Orman, Lacivert, Mor</b>
</p>

<div align="center">

| | |
|:---:|:---:|
| <img src="screenshots/home.png" width="100%"> | <img src="screenshots/episodes.png" width="100%"> |
| **Ana Sayfa** | **Bölümler** |
| <img src="screenshots/film.png" width="100%"> | <img src="screenshots/favorites.png" width="100%"> |
| **Film** | **Favoriler** |
| <img src="screenshots/marathon.png" width="100%"> | <img src="screenshots/history.png" width="100%"> |
| **İzleme Maratonu** | **Geçmiş** |
| <img src="screenshots/downloads.png" width="100%"> | <img src="screenshots/settings.png" width="100%"> |
| **İndirilenler** | **Ayarlar** |
| <img src="screenshots/welcome.png" width="100%"> | |
| **Karşılama** | |

</div>

---

## Kurulum

### AppImage (önerilen)

1. [Releases](https://github.com/nyx47rd/animecix/releases) sayfasından `AnimeciX-x86_64.AppImage` dosyasını indirin.
2. Çalıştırılabilir yapın ve açın:

   ```bash
   chmod +x AnimeciX-x86_64.AppImage
   ./AnimeciX-x86_64.AppImage
   ```

AppImage tek dosyadır; taşınabilir, kurulum gerektirmez. İstersen masaüstü başlatıcısını
uygulama içindeki **Ayarlar → Masaüstü Başlatıcısını Sistemime Kur** ile ekleyebilirsin.

### Kaynaktan derleme

Gerekli sistem bağımlılıkları:

- **Debian/Ubuntu:** `sudo apt install libgtk-4-dev libadwaita-1-dev mpv pkg-config`
- **Fedora:** `sudo dnf install gtk4-devel libadwaita-devel mpv pkgconf-pkg-config`
- **Arch:** `sudo pacman -S gtk4 libadwaita mpv pkgconf`

Rust (1.74+) kurulu olmalı:

```bash
git clone https://github.com/nyx47rd/animecix.git
cd animecix
cargo build --release
./target/release/animecix
```

---

## Güncelleme

Uygulama bir **AppImage** olarak çalışıyorsa başlangıçta yeni sürümü kontrol eder:

- **Otomatik:** *Ayarlar → Güncelleme → Otomatik Güncelleme* açıkken yeni sürüm bulunursa
  onay kutusu çıkar; “Güncelle ve Yeniden Başlat” deyince indirir, kurar ve uygulamayı yeniden başlatır.
- **Elle:** *Ayarlar → Şimdi Güncelle* ile istediğin an kontrol edebilirsin.
Kaynaktan derlenen sürümde otomatik güncelleme devre dışıdır.

---

## İsteğe bağlı: Daha hızlı video (VPN proxy)

> **Not:** VPN Proxy, Flatpak sürümünde bulunmaz (sandbox, host'ta süreç
> başlatmaya izin vermez). AppImage ve AUR sürümlerinde kullanılabilir.

ISS'n video trafiğini kısıtlıyorsa yerelde bir proxy çalıştırman yeterli: uygulama
`127.0.0.1:10808` portunu görünce mpv video trafiğini **otomatik** oradan geçirir;
proxy yoksa hiçbir şey değişmez.

Kullanılan araç: [sing-box](https://github.com/SagerNet/sing-box) (root'suz, kullanıcı
alanında çalışır) + [ProtonVPN](https://protonvpn.com) ücretsiz WireGuard config'i.

### Kurulum (tek seferlik, ~2 dakika)

1. **sing-box indir:** [GitHub Releases](https://github.com/SagerNet/sing-box/releases)
   sayfasından **Linux x86_64** (`amd64`) `.tar.gz` dosyasını indir. Arşivi aç ve
   binary'yi koy:
   ```bash
   mkdir -p ~/.local/share/singbox
   tar xzf sing-box-*-linux-amd64.tar.gz
   cp sing-box-*/sing-box ~/.local/share/singbox/
   chmod +x ~/.local/share/singbox/sing-box
   ```
2. **ProtonVPN WireGuard config al:** protonvpn.com → Giriş → **Downloads** →
   "WireGuard configuration" → platform **GNU/Linux** → ücretsiz ülke (ör. NL-FREE) →
   indirilen `.conf` dosyasını şuraya kaydet:
   ```bash
   cp ~/İndirilenler/wireguard-config.conf ~/.local/share/singbox/config.json
   ```
   (Config dosya adı tam olarak `config.json` olmalı.)
3. **Başlat:** Uygulamada **Ayarlar → VPN Proxy → Başlat**'a bas. Durum satırı
   "Çalışıyor"a dönerse ve çıkan pencerede **Yeniden Başlat**'a basarsan API trafiği
   (ana sayfa, arama) de tüneleden geçer — ISS engelleri tamamen aşılır. Yeniden
   başlatmadan yalnızca video trafiği tüneleden geçer. (Terminal severler için elle
   komut:
   `~/.local/share/singbox/sing-box run -c ~/.local/share/singbox/config.json &`
   — bu durumda da tünel açıldıktan sonra uygulamayı elle yeniden başlat.)

### Doğrulama

Durum satırı "Çalışıyor" gösteriyorsa mpv, videoları 127.0.0.1:10808 üzerinden
çıkarır. Çıkış IP'ni kontrol etmek için:
```bash
curl -x socks5h://127.0.0.1:10808 https://www.gstatic.com/generate_204 -o /dev/null -w "%{http_code}\n"
```
`204` dönüyorsa tünel aktif demektir.

### Notlar

- Uygulama config'i şu sırayla arar: sing-box binary'sinin yanındaki `config.json`,
  `~/.local/share/singbox/config.json`, `~/vpn-config.json`, `~/sing-box-config.json`.
- Proxy'yi durdurmak için **Ayarlar → VPN Proxy → Durdur**.
- Proxy kapatılırsa uygulama normal bağlantıya döner; hiçbir ayarın bozulmaz.

---

## Geliştiriciler için derleme & yayın

AppImage üretmek ve GitHub Release oluşturmak için:

```bash
# GITHUB_TOKEN (repo için içerik/yayın yetkisi olan PAT) tanımlıysa
# betik sürümü otomatik artırır, derler ve release olarak yayınlar:
export GITHUB_TOKEN=ghp_xxxxxxxx
bash build_appimage.sh
```

`build_appimage.sh` her çalıştığında `Cargo.toml` sürümünü otomatik artırır, `cargo build --release`
çalıştırır ve `GITHUB_TOKEN` tanımlıysa `AnimeciX-x86_64.AppImage` dosyasını
`v<surum>` etiketli bir GitHub Release olarak yükler.

---

## Klavye Kısayolları

| Kısayol | İşlev |
|---|---|
| `/` | Bölüm ekranında hızlı bölüm arama |
| `Ctrl+S` | Ana ekranda arama çubuğuna odaklan |
| `Ctrl+T` | Araçlar menüsünü aç/kapat |
| `s` | Oynatıcıda intro sonuna atla |
| `e` | Oynatıcıda outro sonuna atla |
| `Shift+M` (`M`) | Çalan şarkıyı tarayıcıda aç (şarkı bilgisi varsa) |
| `Esc` | Geri / aramayı kapat |

Kısayollar *Ayarlar* ekranından değiştirilebilir.

---

## Yapılandırma & Veri

Tüm veriler (geçmiş, ayarlar, kapak önbelleği) şurada tutulur:

```
~/.local/share/animecix/
~/.cache/animecix/
```

*Ayarlardan* “Tüm Verileri Sıfırla ve Temizle” ile sıfırlanabilir.

---

## Performans

Uygulama, ağ gecikmesini azaltmak için aşağıdaki teknikleri kullanır:

- **HTTP/2 multiplexing**: Tek TCP/TLS bağlantısı üzerinden çoklu eşzamanlı akış; özellikle
  ana sayfadaki onlarca kapak görselini sıraya sokmadan paralel getirir.
- **Bağlantı havuzu & keep-alive**: Boşta bağlantılar 60sn boyunca sıcak tutulur
  (`pool_idle_timeout`), böylece her istekte tekrar TLS/DNS el sıkışması yapılmaz.
- **DNS önbelleği** (`hickory-dns`): Çözümlenen adresler saklanır, tekrarlı `getaddrinfo`
  engellenir.
- **Brotli sıkıştırma**: JSON yanıtları `br` ile sıkıştırılarak aktarılır.
- **Paralel kapak indirme**: 12 worker ile kapaklar eşzamanlı çekilir (diskte 7 gün önbellekli).
- **Stale-while-revalidate API önbelleği**: Süresi dolmuş veri anında gösterilir, arka planda
  tazelenir; çevrimdışıyken bile eski veri kullanılır.

> Geliştiriciler: `ANIMECIX_BENCH=1 ./target/release/animecix` ile her isteğin süresini
> stderr'a loglayabilir (davranışı etkilemez).

---

## Resmi İntro/Outro Verisi

AnimeciX, intro ve outro süreleri ile açılış/kapanış şarkı bilgilerini
AnimeciX'in resmi video altyapısından alır.
Video açılmadan önce çözülür; sonuç 6 saat önbelleğe alınır.

**Davranış**:
- `s` / `e` tuşları intro/outro sonuna atlar; atlayınca MPV OSD'de bildirim çıkar
- Şarkı varsa bölüm boyunca sağ üstte görünür; `Shift+M` tuşu şarkıyı tarayıcıda açar
- İntro/outro bildirimleri ve `Shift+M` ipucu Ayarlar'dan kapatılabilir (tuşlar çalışmaya devam eder)

---

## Lisans

[MIT](LICENSE) © 2026 nyx47rd
