# Sürüm notları

Her başlık bir sürümdür; sürüm iş akışı GitHub'daki sürüm notunu buradan alır.

## 0.6.0

Linux'ta tek adımda kaldırma.

- **Linux'ta "Robz DPI'ı kaldır" artık uygulamayı da kaldırır** (Windows'taki gibi). `.deb` ve `.rpm`'de paket tek
  yönetici onayıyla kaldırılır; paketin kaldırma betiği motoru da siler. AppImage'da önce motor kaldırılır, sonra
  AppImage dosyası ve ayarlar silinir; dosyayı Robz Hub kurduysa onun klasörü ve uygulama menüsü girişi de silinir.
  Onay verilmezse hiçbir şey silinmez. Katkı: [@bberkay](https://github.com/bberkay)
  ([#2](https://github.com/kutsalkiremitci/robz-dpi/pull/2)).
- Paket kaldırma yalnız uygulamanın kendi paketiyle (`robz-dpi`) sınırlıdır.

## 0.5.0

Site listesi ve hariç tutulan siteler.

- **Eklenen siteler listede:** "Seçtiklerimde" kipinde eklenen her site Discord ve Roblox'un altında kendi anahtarıyla
  listelenir; geçici olarak kapatılabilir ya da kaldırılabilir. Kapalı siteler listede kalır, motora gitmez.
- **Hariç tutulan siteler:** "Bütün sitelerde" kipinde dokunulmayacak siteler eklenebilir. Robz DPI bu sitelere ve
  alt adreslerine dokunmaz, DNS sorgularını da yönlendirmez; diğer bütün sitelerde çalışır.
- **Logolar:** Discord ve Roblox listede kendi logolarıyla, eklenen siteler adlarının ilk iki harfiyle görünür.
  Sorumluluk reddine logoların yalnız seçimi göstermek için kullanıldığı ve projenin bu şirketlerle bağlantılı
  olmadığı notu eklendi.

## 0.4.0

Windows için taşınabilir sürüm.

- **Taşınabilir sürüm:** `RobzDPI-Portable.zip`; kurulum gerektirmez. Bir klasöre çıkarılıp çalıştırılır; ayarlar ve
  arayüzün önbelleği programın yanındaki `data` klasöründe tutulur, bilgisayarda iz kalmaz. Motor yine Windows
  hizmeti olarak kurulur (yönetici onayıyla); Ayarlar → "Robz DPI'ı kaldır" onu kaldırır, uygulamanın kendisi klasör
  silinerek kaldırılır.

## 0.3.0

AppImage ve daha geniş Linux uyumu.

- **AppImage:** `.deb` ya da `.rpm` kullanmayan dağıtımlar (Arch, Manjaro ve benzerleri) için kurulum gerektirmeyen tek
  dosya. Motor yine sistemin systemd, nftables ve polkit araçlarını kullanır; açılması için FUSE 2 gerekir.
- **Motor ayrı bir program:** Linux'ta motor ve yönetici yardımcısı artık arayüzden ayrı, küçük bir programdır
  (`robzdpi-engine`). Root yetkisiyle çalışan servis arayüz kitaplıklarını yüklemez.
- **Daha eski dağıtımlar:** Linux paketleri artık Ubuntu 22.04 tabanında derlenir ve glibc 2.35 ile sonrasında çalışır
  (Ubuntu 22.04, Debian 12, Fedora 36 ve sonrası). 0.2.0 paketleri daha yeni bir tabanda derlenmişti ve bu
  sistemlerde açılmayabiliyordu.

Bilinen sınırlar (Linux):

- AppImage musl tabanlı dağıtımlarda (Alpine gibi) çalışmaz.
- Motorun ürettiği paketler otomatik testlerle doğrulanmıştır; Linux sürümü her dağıtımda ve her servis sağlayıcıda
  denenmemiştir.

## 0.2.0

Linux desteği.

- **Linux:** `.deb` (Debian, Ubuntu, Linux Mint) ve `.rpm` (Fedora, openSUSE) paketleri. Ek sürücü gerekmez:
  paketleri çekirdeğin kendi paket kuyruğu (nfqueue) yakalar, kuralları nftables yükler. Motor bir systemd birimi
  olarak çalışır; yönetici onayı polkit ile istenir. Paket kaldırılınca motor da kaldırılır.
- **Arayüz:** "Bütün sitelerde" açıklaması sadeleştirildi; sisteme göre değişen metinler Linux'a uyarlandı.
- **Windows:** "Robz DPI'ı kaldır" ayar klasörünü, dosyaları hâlâ kullanılıyorsa birkaç kez yeniden deneyerek siler.

Bilinen sınırlar (Linux):

- systemd, nftables ve polkit gerektirir; yalnız x86_64.
- Motorun ürettiği paketler otomatik testlerle (iki makine arasında ve gerçek bir sitede) doğrulanmıştır; ancak Linux
  sürümü her dağıtımda ve her servis sağlayıcıda denenmemiştir.

## 0.1.0

İlk sürüm. Motor her servis sağlayıcıda denenmemiştir.

- **Tek düğme:** motoru açar ve kapatır. Windows ilk açılışta ve ayar değişikliğinde yönetici onayı ister; açıp
  kapamak onay gerektirmez.
- **Yalnız seçili siteler:** Discord ve Roblox hazır gelir, başka siteler adresi yazılarak eklenir. "Bütün sitelerde"
  kipi her bağlantıya uygular.
- **Kendi motoru:** paketleri işleyen kod bu depodadır; yöntemler GoodbyeDPI'dan alınmıştır. Dört hazır yöntem ve
  alan alan düzenlenebilen "Özel" yöntem.
- **DNS yönlendirme:** seçili sitelerin sorguları Quad9, Yandex, AdGuard, OpenDNS ya da belirlediğiniz sunucuya
  yönlendirilir.
- **Bağlantı testi:** seçili sitelere ulaşılıp ulaşılamadığını gösterir.
- **Gelişmiş bölüm:** yöntemin ayrıntıları, motorun sayaçları ve günlüğü.
- **Arka planda çalışma:** Windows açılışında tepside başlar ve son durumunu hatırlar.
- **Çakışma uyarısı:** çalışan GoodbyeDPI'ı algılar ve onayınızla durdurur.
- **Tek adımda kaldırma:** Ayarlar → "Robz DPI'ı kaldır" uygulamayı, arka planda çalışan parçasını ve ayarları
  birlikte siler. Windows'un Uygulamalar listesinden kaldırmak da arka plandaki parçayı siler.
- **İlk açılış adımları:** dil, sorumluluk reddinin kabulü ve nerede çalışacağı; bir kez sorulur.
- Türkçe, İngilizce ve Rusça arayüz.

Bilinen sınırlar:

- Kurulum dosyası imzalı değildir; Windows SmartScreen uyarı gösterebilir.
- Bazı antivirüs yazılımları (Kaspersky başta olmak üzere) WinDivert sürücüsünü engelleyebilir; ayrıntılar README'de.
- Yalnız 64 bit Windows 10 ve 11.

Eğitim ve test amaçlıdır; kullanmadan önce [sorumluluk reddini](https://github.com/kutsalkiremitci/robz-dpi/blob/main/DISCLAIMER.md) okuyunuz.
