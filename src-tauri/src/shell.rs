//! Pencere, tepsi simgesi ve "Windows açılışında başlat". İş mantığı dpi.rs'tedir; burası onu masaüstüne bağlar.

use crate::dpi::Dpi;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;

/// tauri.conf.json'daki kimlik; uygulamanın veri klasörünün adıdır.
const APP_IDENTIFIER: &str = "com.robz.dpi";
const TRAY_ID: &str = "main";
const WINDOW: &str = "main";

struct Strings {
    open: &'static str,
    turn_on: &'static str,
    turn_off: &'static str,
    quit: &'static str,
    tip_on: &'static str,
    tip_off: &'static str,
}

fn strings(lang: &str) -> Strings {
    if lang == "ru" {
        Strings {
            open: "Открыть Robz DPI",
            turn_on: "Включить",
            turn_off: "Выключить",
            quit: "Выход",
            tip_on: "Robz DPI: включено",
            tip_off: "Robz DPI: выключено",
        }
    } else if lang == "en" {
        Strings {
            open: "Open Robz DPI",
            turn_on: "Turn on",
            turn_off: "Turn off",
            quit: "Quit",
            tip_on: "Robz DPI: on",
            tip_off: "Robz DPI: off",
        }
    } else {
        Strings {
            open: "Robz DPI'ı aç",
            turn_on: "Aç",
            turn_off: "Kapat",
            quit: "Çıkış",
            tip_on: "Robz DPI: açık",
            tip_off: "Robz DPI: kapalı",
        }
    }
}

// ---- dil ------------------------------------------------------------------------------------------------------------

/// "Windows dili": görüntü dili, tercih edilen diller ya da bölge Türkiye ise Türkçe; Rusça ise Rusça; değilse
/// İngilizce.
/// (Türkiye'de İngilizce Windows kullanmak yaygındır; kullanıcı üst çubuktan tek tıkla değiştirebilir.)
#[cfg(windows)]
pub fn system_language() -> &'static str {
    use windows_sys::Win32::Globalization::{
        GetUserDefaultGeoName, GetUserDefaultLocaleName, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME,
    };
    let mut texts = Vec::new();
    // SAFETY: her çağrıya yeterli boyutta tampon ve boyutu verilir.
    unsafe {
        let mut buf = [0u16; 85];
        let n = GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32);
        if n > 1 {
            texts.push(String::from_utf16_lossy(&buf[..(n - 1) as usize]));
        }
        let mut geo = [0u16; 16];
        let n = GetUserDefaultGeoName(geo.as_mut_ptr(), geo.len() as i32);
        if n > 1 && String::from_utf16_lossy(&geo[..(n - 1) as usize]).eq_ignore_ascii_case("TR") {
            return "tr";
        }
        let mut count = 0u32;
        let mut list = [0u16; 512];
        let mut len = list.len() as u32;
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, list.as_mut_ptr(), &mut len) != 0 {
            texts.extend(String::from_utf16_lossy(&list[..len as usize]).split('\0').map(str::to_string));
        }
    }
    let speaks = |code: &str| texts.iter().any(|t| t.to_ascii_lowercase().starts_with(code));
    if speaks("tr") {
        "tr"
    } else if speaks("ru") {
        "ru"
    } else {
        "en"
    }
}

/// Windows dışında: ortamın dili (LC_ALL, LC_MESSAGES, LANG: "tr_TR.UTF-8" gibi).
#[cfg(not(windows))]
pub fn system_language() -> &'static str {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default()
        .to_lowercase();
    if locale.starts_with("tr") {
        "tr"
    } else if locale.starts_with("ru") {
        "ru"
    } else {
        "en"
    }
}

// ---- taşınabilir sürüm ----------------------------------------------------------------------------------------------

/// Taşınabilir sürümün işareti: programın yanındaki bu dosya (sürüm iş akışı zip'e koyar).
const PORTABLE_MARKER: &str = "portable.txt";

/// Taşınabilir sürümde ayarların ve arayüz önbelleğinin klasörü: programın yanındaki "data". Böylece uygulama
/// bilgisayarda iz bırakmaz; klasörü silmek uygulamayı siler. İşaret yoksa ya da klasöre yazılamıyorsa (örneğin
/// salt okunur bir yerden çalıştırıldıysa) None: olağan yer (%LOCALAPPDATA%\com.robz.dpi) kullanılır.
pub fn portable() -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        if !dir.join(PORTABLE_MARKER).is_file() {
            return None;
        }
        let data = dir.join("data");
        std::fs::create_dir_all(&data).ok()?;
        let probe = data.join(".yazma-denemesi");
        std::fs::write(&probe, b"").ok()?;
        let _ = std::fs::remove_file(probe);
        Some(data)
    })
    .as_deref()
}

// ---- pencere --------------------------------------------------------------------------------------------------------

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    } else if let Err(error) = create_window(app) {
        eprintln!("pencere açılamadı: {error}");
    }
}

pub fn create_window(app: &AppHandle) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html".into()));
    // Taşınabilir sürümde arayüzün önbelleği de programın yanındaki klasörde durur.
    if let Some(data) = portable() {
        builder = builder.data_directory(data.join("EBWebView"));
    }
    let window = builder
        .title("Robz DPI")
        .inner_size(520.0, 800.0)
        .min_inner_size(440.0, 600.0)
        .center()
        .theme(Some(tauri::Theme::Dark))
        .background_color(Color(13, 17, 23, 255))
        // Arayüz yalnızca kendi sayfasında kalır; bağlantılar varsayılan tarayıcıda açılır (ui/js/main.js).
        .on_navigation(|url| url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost"))
        .build()?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            // Motor ayrı bir Windows servisidir: pencere kapansa da çalışmayı sürdürür. Tepside kalmak yalnızca
            // durumu görmeyi ve tek tıkla açıp kapatmayı sağlar.
            if handle.state::<Arc<Dpi>>().settings().close_to_tray {
                api.prevent_close();
                if let Some(window) = handle.get_webview_window(WINDOW) {
                    let _ = window.hide();
                }
            }
        }
    });
    Ok(())
}

// ---- tepsi ----------------------------------------------------------------------------------------------------------

static TRAY_SIGNATURE: Mutex<String> = Mutex::new(String::new());

fn build_tray_menu(app: &AppHandle, lang: &str, running: bool) -> tauri::Result<Menu<tauri::Wry>> {
    let t = strings(lang);
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "open", t.open, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let (id, label) = if running { ("off", t.turn_off) } else { ("on", t.turn_on) };
    menu.append(&MenuItem::with_id(app, id, label, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?)?;
    Ok(menu)
}

pub fn create_tray(app: &AppHandle, dpi: &Dpi) -> tauri::Result<()> {
    let (lang, running) = (dpi.language(), dpi.running());
    let t = strings(lang);
    let menu = build_tray_menu(app, lang, running)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(if running { t.tip_on } else { t.tip_off })
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            let dpi = app.state::<Arc<Dpi>>().inner().clone();
            match event.id().as_ref() {
                "open" => show_window(app),
                "on" => {
                    // Yönetici onayı ya da bir hata çıkarsa kullanıcı görsün diye pencere de açılır.
                    show_window(app);
                    tauri::async_runtime::spawn(async move { dpi.turn_on().await });
                }
                "off" => {
                    tauri::async_runtime::spawn(async move { dpi.turn_off().await });
                }
                "quit" => app.exit(0),
                _ => {}
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    *TRAY_SIGNATURE.lock().unwrap_or_else(|p| p.into_inner()) = format!("{lang}|{running}");
    Ok(())
}

/// Durum değişince: tepsi menüsü ve ipucu (motor açıldı/kapandı ya da dil değiştiyse).
pub fn state_changed(dpi: &Dpi) {
    let (lang, running) = (dpi.language(), dpi.running());
    let signature = format!("{lang}|{running}");
    let mut last = TRAY_SIGNATURE.lock().unwrap_or_else(|p| p.into_inner());
    if *last == signature {
        return;
    }
    if let Some(tray) = dpi.app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_tray_menu(&dpi.app, lang, running) {
            let t = strings(lang);
            let _ = tray.set_menu(Some(menu));
            let _ = tray.set_tooltip(Some(if running { t.tip_on } else { t.tip_off }));
            *last = signature;
        }
    }
}

// ---- uygulamayı kaldırma --------------------------------------------------------------------------------------------

/// Kurulum programının bıraktığı kaldırma programı (yalnız kurulu sürümde vardır).
pub fn uninstaller() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join("uninstall.exe");
    path.is_file().then_some(path)
}

/// Bu süreç kapandıktan sonra uygulamayı sessizce kaldırır ve ayar klasörünü siler. Ayrı, penceresiz bir komut
/// satırı başlatır ve hemen döner; çağıran ardından uygulamadan çıkmalıdır.
#[cfg(windows)]
pub fn run_uninstaller(uninstaller: &Path, data_dir: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut script = format!("/d /s /c \"ping -n 3 127.0.0.1 >nul & \"{}\" /S", uninstaller.display());
    // Yalnız uygulamanın kendi veri klasörü silinir (geliştirme için başka bir klasör verilmişse dokunulmaz).
    // Arayüzün (WebView2) arka plan süreçleri kapanırken dosyalarını bir süre açık tutabilir; klasör tümüyle
    // gidene kadar birkaç saniye arayla yeniden denenir (en çok ~20 saniye).
    if data_dir.file_name().is_some_and(|name| name == APP_IDENTIFIER) {
        let dir = data_dir.display();
        script.push_str(&format!(
            " & for /l %i in (1,1,8) do @(ping -n 3 127.0.0.1 >nul & if exist \"{dir}\" rmdir /s /q \"{dir}\")"
        ));
    }
    script.push('"');
    let cmd = std::env::var_os("SystemRoot").map(|root| Path::new(&root).join("System32").join("cmd.exe"));
    std::process::Command::new(cmd.as_deref().unwrap_or(Path::new("cmd.exe")))
        .raw_arg(script)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(drop)
}

#[cfg(not(windows))]
pub fn run_uninstaller(_uninstaller: &Path, _data_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("yalnız Windows"))
}

// ---- AppImage'ı kaldırma (Linux) ------------------------------------------------------------------------------------

/// AppImage olarak çalışıyorsak dosyanın yolu. AppImage çalışırken APPIMAGE değişkenine kendi yolunu, APPDIR'e
/// içeriğini bağladığı klasörü yazar. Bu değişkenler başka bir AppImage'dan (örneğin AppImage olan bir uçbirimden)
/// miras kalmış olabilir; o yüzden yalnız program gerçekten APPDIR'in içinden çalışıyorsa geçerlidir.
#[cfg(target_os = "linux")]
pub fn appimage() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let (file, appdir) = (var("APPIMAGE")?, var("APPDIR")?);
    let inside = std::env::current_exe().is_ok_and(|exe| exe.starts_with(&appdir));
    (inside && file.is_absolute() && file.is_file()).then_some(file)
}

#[cfg(not(target_os = "linux"))]
pub fn appimage() -> Option<PathBuf> {
    None
}

/// Uygulama kaldırılırken kullanıcının klasörlerinden silinecekler: motorun önbellekteki kopyası (engine/linux.rs)
/// ve ayar klasörü (yalnız uygulamanın kendi klasörüyse; geliştirme için başka bir klasör verilmişse dokunulmaz).
/// AppImage'da (`appimage`) dosyanın kendisi de; onu Robz Hub kurduysa (<veri>/robz-hub/apps/robz-dpi/) Robz Hub'ın
/// klasörü ve uygulama menüsü girişi de silinir, yoksa menüde açılmayan bir giriş kalır.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn leftovers(appimage: Option<&Path>, data_dir: &Path, data_home: Option<&Path>, cache_home: Option<&Path>) -> Vec<PathBuf> {
    let mut targets: Vec<PathBuf> = appimage.iter().map(|file| file.to_path_buf()).collect();
    if let (Some(appimage), Some(data_home)) = (appimage, data_home) {
        let hub_dir = data_home.join("robz-hub").join("apps").join("robz-dpi");
        if appimage.parent() == Some(hub_dir.as_path()) {
            targets.push(hub_dir);
            targets.push(data_home.join("applications").join("robz-hub-robz-dpi.desktop"));
        }
    }
    if let Some(cache_home) = cache_home {
        targets.push(cache_home.join("robz-dpi"));
    }
    if data_dir.file_name().is_some_and(|name| name == APP_IDENTIFIER) {
        targets.push(data_dir.to_path_buf());
    }
    targets
}

/// Bu süreç kapandıktan sonra AppImage'ı ve ayarları siler. Ayrı bir kabuk başlatır ve hemen döner; çağıran ardından
/// uygulamadan çıkmalıdır. Silme çıkıştan sonradır: arayüz kapanırken ayar klasörüne yeniden yazabilir.
#[cfg(target_os = "linux")]
pub fn remove_appimage(appimage: &Path, data_dir: &Path) -> std::io::Result<()> {
    delete_after_exit(Some(appimage), data_dir)
}

/// Paket kaldırıldıktan sonra: bu süreç kapanınca ayarları ve motorun önbellekteki kopyasını siler.
#[cfg(target_os = "linux")]
pub fn remove_user_data(data_dir: &Path) -> std::io::Result<()> {
    delete_after_exit(None, data_dir)
}

#[cfg(target_os = "linux")]
fn delete_after_exit(appimage: Option<&Path>, data_dir: &Path) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    // $1: beklenecek süreç (en çok ~10 sn), kalanlar: silinecek yollar. Yollar komut satırına yazılmaz, argümandır.
    const SCRIPT: &str = "pid=$1; shift; n=0; while kill -0 \"$pid\" 2>/dev/null && [ $n -lt 50 ]; do sleep 0.2; n=$((n+1)); done; \
                          sleep 1; rm -rf -- \"$@\"";
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
    let xdg = |name: &str, fallback: &str| {
        let dir = std::env::var_os(name).map(PathBuf::from).filter(|dir| dir.is_absolute());
        dir.or_else(|| home.as_ref().map(|home| home.join(fallback)))
    };
    let (data_home, cache_home) = (xdg("XDG_DATA_HOME", ".local/share"), xdg("XDG_CACHE_HOME", ".cache"));
    std::process::Command::new("/bin/sh")
        .args(["-c", SCRIPT, "sh", &std::process::id().to_string()])
        .args(leftovers(appimage, data_dir, data_home.as_deref(), cache_home.as_deref()))
        // Kabuk ve rm, AppImage'ın içindeki kitaplıkları değil sisteminkileri kullansın.
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
}

#[cfg(not(target_os = "linux"))]
pub fn remove_appimage(_appimage: &Path, _data_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("yalnız Linux"))
}

// ---- açılışta başlatma ve zamanlayıcı -------------------------------------------------------------------------------

/// Yalnız kurulu sürümde: geliştirme kipinde geçici exe'yi Windows açılışına eklemeyelim.
pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let autostart = app.autolaunch();
    let _ = if enabled { autostart.enable() } else { autostart.disable() };
}

/// Servisin durumu 2 sn'de bir okunur (pencere kapalı ya da tepsideyken de). Okuma ucuzdur: Servis Denetim
/// Yöneticisi'ne tek sorgu ve süreç listesine bir bakış.
pub fn start_timers(dpi: Arc<Dpi>) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            dpi.poll();
        }
    });
}

// ---- paketi kaldırma (Linux) ----------------------------------------------------------------------------------------

/// Bir programı bilinen sistem klasörlerinde arar (PATH kullanıcıya aittir; yönetici olarak çalışacak programda ona
/// güvenilmez).
#[cfg(target_os = "linux")]
fn system_program(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/sbin", "/sbin"].iter().map(|dir| Path::new(dir).join(name)).find(|path| path.is_file())
}

/// Paket adı mı: yalnız paket adlarında geçen karakterler (yönetici olarak çalışacak komuta argüman olur).
fn plain_package_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// `rpm -qf --qf '%{NAME}\n'` çıktısından paketin adı: tek satır.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn rpm_owner(output: &str) -> Option<String> {
    let mut lines = output.lines();
    let name = lines.next()?.trim();
    (plain_package_name(name) && lines.next().is_none()).then(|| name.to_string())
}

/// `dpkg -S <dosya>` çıktısından paketin adı: "robz-dpi: /usr/bin/robz-dpi" ya da "robz-dpi:amd64: /usr/bin/robz-dpi".
/// Dosya birden çok paketinse ("a, b: /yol") None.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn dpkg_owner(output: &str) -> Option<String> {
    let mut lines = output.lines();
    let (owner, _) = lines.next()?.split_once(": ")?;
    let name = owner.split(':').next()?.trim();
    (plain_package_name(name) && lines.next().is_none()).then(|| name.to_string())
}

/// Sistem paketinin türü: kaldıran program buna göre seçilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum PackageKind {
    Rpm,
    Deb,
}

/// Uygulama sistem paketi olarak kuruluysa (.rpm: Fedora, openSUSE; .deb: Debian, Ubuntu) paketin türü ve adı:
/// çalışan programın sahibi olan paket. AppImage'da ve geliştirme kipinde None. Bir kez sorulur.
#[cfg(target_os = "linux")]
pub fn system_package() -> Option<(PackageKind, &'static str)> {
    static PACKAGE: OnceLock<Option<(PackageKind, String)>> = OnceLock::new();
    let owner = |program: &str, arguments: &[&str], parse: fn(&str) -> Option<String>| {
        let exe = std::env::current_exe().ok()?;
        let output = std::process::Command::new(system_program(program)?)
            .args(arguments)
            .arg(exe)
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        output.status.success().then(|| parse(&String::from_utf8_lossy(&output.stdout))).flatten()
    };
    PACKAGE
        .get_or_init(|| {
            if appimage().is_some() {
                return None;
            }
            let rpm = || owner("rpm", &["-qf", "--qf", "%{NAME}\n"], rpm_owner).map(|name| (PackageKind::Rpm, name));
            let deb = || owner("dpkg", &["-S"], dpkg_owner).map(|name| (PackageKind::Deb, name));
            rpm().or_else(deb)
        })
        .as_ref()
        .map(|(kind, name)| (*kind, name.as_str()))
}

#[cfg(not(target_os = "linux"))]
pub fn system_package() -> Option<(PackageKind, &'static str)> {
    None
}

/// Paketi yönetici onayıyla (pkexec) kaldırır ve bitene kadar bekler; paketin kaldırma betiği motoru da kaldırır
/// (linux/prerm.sh). `rpm -e` ve `dpkg -r` yalnız bu paketi siler: `dnf remove` ve `apt autoremove` artık
/// kullanılmayan bağımlılıkları da götürürdü. Ok(false): kullanıcı onay vermedi, hiçbir şey silinmedi.
#[cfg(target_os = "linux")]
pub fn remove_package(kind: PackageKind, package: &str) -> std::io::Result<bool> {
    use std::process::Stdio;
    let program = |name: &str| system_program(name).ok_or_else(|| std::io::Error::other(format!("{name} bulunamadı")));
    let (remover, flag) = match kind {
        PackageKind::Rpm => ("rpm", "-e"),
        PackageKind::Deb => ("dpkg", "-r"),
    };
    let status = std::process::Command::new(program("pkexec")?)
        .arg(program(remover)?)
        .args([flag, "--", package])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        // pkexec: 126 kullanıcı onay vermedi.
        Some(126) => Ok(false),
        code => Err(std::io::Error::other(format!("çıkış kodu {}", code.unwrap_or(-1)))),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn remove_package(_kind: PackageKind, _package: &str) -> std::io::Result<bool> {
    Err(std::io::Error::other("yalnız Linux"))
}

#[cfg(not(target_os = "linux"))]
pub fn remove_user_data(_data_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("yalnız Linux"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_what_an_uninstall_deletes() {
        let (data_home, cache_home) = (Path::new("/home/a/.local/share"), Path::new("/home/a/.cache"));
        let data_dir = data_home.join(APP_IDENTIFIER);
        let of = |appimage: Option<&str>, data_dir: &Path| leftovers(appimage.map(Path::new), data_dir, Some(data_home), Some(cache_home));

        // Kullanıcının kendi indirdiği dosya: dosya, motorun kopyası ve ayarlar.
        let own = of(Some("/home/a/Downloads/RobzDPI-Linux-x86_64.AppImage"), &data_dir);
        assert_eq!(own, ["/home/a/Downloads/RobzDPI-Linux-x86_64.AppImage", "/home/a/.cache/robz-dpi", "/home/a/.local/share/com.robz.dpi"].map(PathBuf::from));

        // Robz Hub'ın kurduğu dosya: klasörü ve menü girişi de.
        let hub = of(Some("/home/a/.local/share/robz-hub/apps/robz-dpi/robz-dpi.AppImage"), &data_dir);
        assert!(hub.contains(&PathBuf::from("/home/a/.local/share/robz-hub/apps/robz-dpi")));
        assert!(hub.contains(&PathBuf::from("/home/a/.local/share/applications/robz-hub-robz-dpi.desktop")));
        // Başka bir uygulamanın klasörü Robz Hub'ınki sayılmaz.
        assert_eq!(of(Some("/home/a/.local/share/robz-hub/apps/robz-play/x.AppImage"), &data_dir).len(), 3);

        // Paket olarak kurulu sürüm: yalnız motorun kopyası ve ayarlar.
        assert_eq!(of(None, &data_dir), ["/home/a/.cache/robz-dpi", "/home/a/.local/share/com.robz.dpi"].map(PathBuf::from));

        // Geliştirme için verilen ayar klasörüne dokunulmaz.
        assert!(!of(Some("/home/a/x.AppImage"), Path::new("/tmp/deneme")).contains(&PathBuf::from("/tmp/deneme")));
    }

    #[test]
    fn reads_the_owning_package() {
        assert_eq!(rpm_owner("robz-dpi\n"), Some("robz-dpi".to_string()));
        assert_eq!(rpm_owner("lib++x_1.2\n"), Some("lib++x_1.2".to_string()));
        for bad in ["", "\n", "-e", "file /x is not owned by any package\n", "a\nb\n"] {
            assert_eq!(rpm_owner(bad), None, "{bad:?}");
        }

        assert_eq!(dpkg_owner("robz-dpi: /usr/bin/robz-dpi\n"), Some("robz-dpi".to_string()));
        assert_eq!(dpkg_owner("robz-dpi:amd64: /usr/bin/robz-dpi\n"), Some("robz-dpi".to_string()));
        for bad in ["", "robz-dpi\n", "a, b: /usr/bin/x\n", "-r: /usr/bin/x\n", "a: /x\nb: /x\n"] {
            assert_eq!(dpkg_owner(bad), None, "{bad:?}");
        }
    }
}
