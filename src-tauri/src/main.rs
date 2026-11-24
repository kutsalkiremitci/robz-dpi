//! Robz DPI — eğitim ve test amaçlı, açık kaynak DPI aracı: yalnız seçilen sitelerde çalışır.
//!
//! Arayüz (ui/) WebView2'de çalışır ve bu süreçle tek bir komutla konuşur: dpi_call(method, args).
//! Aynı exe `--helper …` ile çağrılırsa pencere açmaz: yönetici yardımcısı olarak çalışır (engine/helper.rs).
//! `--engine` ile çağrılırsa motorun kendisidir: Windows servisi olarak paketleri işler (engine/service.rs).
//!
//! Geliştirme için ortam değişkeni:
//!   ROBZDPI_DATA_DIR    ayarlar için ayrı bir klasör

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod dpi;
mod engine;
mod error;
mod plan;
mod probe;
mod settings;
mod shell;

use dpi::Dpi;
use error::{DpiError, DpiResult};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt as _;

fn arg_str(args: &[Value], index: usize) -> DpiResult<String> {
    args.get(index).and_then(Value::as_str).map(str::to_string).ok_or_else(|| DpiError::new("bad-request"))
}

fn arg_bool(args: &[Value], index: usize) -> DpiResult<bool> {
    args.get(index).and_then(Value::as_bool).ok_or_else(|| DpiError::new("bad-request"))
}

fn open_external(app: &AppHandle, url: &str) -> DpiResult<()> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_whitespace() || c == '"') {
        return Err(DpiError::new("bad-request"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| DpiError::with("unknown", e))
}

async fn dispatch(app: &AppHandle, dpi: &Arc<Dpi>, method: &str, args: &[Value]) -> DpiResult<Value> {
    match method {
        "getState" => return Ok(dpi.snapshot()),
        "turnOn" => dpi.turn_on().await,
        "turnOff" => dpi.turn_off().await,
        "apply" => dpi.apply().await,
        "removeAll" => {
            dpi.remove_all().await;
            // Kaldırma başarılıysa Windows açılışındaki kayıt da silinir.
            if !dpi.settings().enabled && !engine::status().installed {
                let _ = dpi.set_setting("launchAtLogin", &Value::Bool(false));
                shell::apply_autostart(app, false);
            }
        }
        // Her şeyi kaldırır: motor (yönetici onayıyla), Windows açılışındaki kayıt, ayarlar ve uygulamanın kendisi.
        // false: motor kaldırılamadı ya da onay verilmedi; hiçbir şey silinmedi.
        "uninstallApp" => {
            // Linux'ta sistem paketi (.rpm, .deb): tek yönetici onayıyla paket kaldırılır (kaldırma betiği motoru da
            // kaldırır), sonra ayarlar silinir ve uygulama kapanır.
            if let Some((kind, package)) = shell::system_package() {
                let removed = tauri::async_runtime::spawn_blocking(move || shell::remove_package(kind, package))
                    .await
                    .map_err(|e| DpiError::with("unknown", e))?
                    .map_err(|e| DpiError::with("uninstall-failed", e))?;
                if !removed {
                    return Ok(Value::Bool(false));
                }
                shell::apply_autostart(app, false);
                let _ = shell::remove_user_data(dpi.data_dir());
                app.exit(0);
                return Ok(Value::Null);
            }
            if engine::status().installed {
                dpi.remove_all().await;
                if engine::status().installed {
                    return Ok(Value::Bool(false));
                }
            }
            shell::apply_autostart(app, false);
            if let Some(uninstaller) = shell::uninstaller() {
                shell::run_uninstaller(&uninstaller, dpi.data_dir()).map_err(|e| DpiError::with("uninstall-failed", e))?;
                app.exit(0);
            }
            // Linux'ta AppImage: dosya, ayarlar ve (Robz Hub kurduysa) menü girişi silinir.
            if let Some(appimage) = shell::appimage() {
                shell::remove_appimage(&appimage, dpi.data_dir()).map_err(|e| DpiError::with("uninstall-failed", e))?;
                app.exit(0);
            }
            // Geliştirme kipinde kaldırma programı yoktur: yalnız motor kaldırılmış olur.
            return Ok(Value::Bool(true));
        }
        "stopConflicts" => dpi.stop_conflicts().await,
        "test" => dpi.test().await,
        "dismissError" => dpi.dismiss_error(),
        "setSetting" => {
            let key = arg_str(args, 0)?;
            let value = args.get(1).cloned().unwrap_or(Value::Null);
            dpi.set_setting(&key, &value)?;
            if key == "launchAtLogin" {
                shell::apply_autostart(app, value.as_bool().unwrap_or(false));
            }
        }
        "setService" => dpi.set_service(&arg_str(args, 0)?, arg_bool(args, 1)?)?,
        "setCustom" => dpi.set_custom(&arg_str(args, 0)?, args.get(1).unwrap_or(&Value::Null))?,
        "getLog" => return Ok(Value::String(engine::log_tail(48 * 1024))),
        "addDomains" => return Ok(dpi.add_domains(&arg_str(args, 0)?)),
        "removeDomain" => dpi.remove_domain(&arg_str(args, 0)?),
        "setDomain" => dpi.set_domain(&arg_str(args, 0)?, arg_bool(args, 1)?)?,
        "addExcluded" => return Ok(dpi.add_excluded(&arg_str(args, 0)?)),
        "removeExcluded" => dpi.remove_excluded(&arg_str(args, 0)?),
        "openExternal" => open_external(app, &arg_str(args, 0)?)?,
        _ => return Err(DpiError::with("bad-request", method)),
    }
    Ok(Value::Null)
}

#[tauri::command]
async fn dpi_call(app: AppHandle, dpi: State<'_, Arc<Dpi>>, method: String, args: Vec<Value>) -> Result<Value, DpiError> {
    let dpi = dpi.inner().clone();
    dispatch(&app, &dpi, &method, &args).await
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Yönetici yardımcısı ve motor: arayüz yok.
    match args.get(1).map(String::as_str) {
        Some("--helper") => std::process::exit(engine::helper_main(&args[2..])),
        Some("--engine") => std::process::exit(engine::engine_main()),
        _ => {}
    }
    let start_hidden = args.iter().any(|a| a == "--hidden");

    tauri::Builder::default()
        // İkinci kez açılırsa yeni pencere yerine var olan öne gelir (ilk eklenen eklenti olmalı).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .invoke_handler(tauri::generate_handler![dpi_call])
        .setup(move |app| {
            // Ayarların yeri: geliştirme için ortam değişkeni, taşınabilir sürümde programın yanı, yoksa
            // %LOCALAPPDATA%\com.robz.dpi.
            let data_dir = match (std::env::var_os("ROBZDPI_DATA_DIR"), shell::portable()) {
                (Some(dir), _) => PathBuf::from(dir),
                (None, Some(dir)) => dir.to_path_buf(),
                (None, None) => app.path().app_local_data_dir()?,
            };
            let dpi = Dpi::new(app.handle().clone(), data_dir);
            app.manage(dpi.clone());

            shell::create_tray(app.handle(), &dpi)?;
            // Windows açılışında (--hidden) yalnız tepside başlar.
            if !start_hidden {
                shell::create_window(app.handle())?;
            }
            shell::start_timers(dpi.clone());
            // Kullanıcı motoru açık bıraktıysa (ve yönetici onayı gerekmiyorsa) yeniden başlat.
            tauri::async_runtime::spawn(async move { dpi.resume().await });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Robz DPI başlatılamadı");
}
