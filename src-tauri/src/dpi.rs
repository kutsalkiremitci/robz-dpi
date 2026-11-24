//! Uygulamanın durumu ve iş mantığı: ayarlar, motoru açma/kapama, değişiklikleri uygulama, bağlantı testi.
//!
//! Arayüze tek olay gider: "dpi:state" (her değişiklikte tüm durum). Motor işlemlerinin hatası durumun içinde
//! (`engine.error`) taşınır; böylece pencere kapalıyken oluşan hata da açıldığında görünür.

use crate::catalog;
use crate::engine::{self, Conflict, Status};
use crate::error::{DpiError, DpiResult};
use crate::plan::EngineRequest;
use crate::probe;
use crate::settings::Settings;
use crate::shell;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter};

/// Arayüzün "Hakkında" bölümündeki bağlantılar.
const LINKS: [(&str, &str); 3] = [
    ("github", "https://github.com/kutsalkiremitci/robz-dpi"),
    ("discord", "https://discord.gg/UyyEn7SHm4"),
    ("disclaimer", "https://github.com/kutsalkiremitci/robz-dpi/blob/main/DISCLAIMER.md"),
];

/// Test edilecek en fazla özel site sayısı.
const MAX_PROBED_DOMAINS: usize = 6;

struct Inner {
    settings: Settings,
    /// Süren işlem: "starting" | "stopping" | "applying" | "removing" | "fixing"
    busy: Option<&'static str>,
    error: Option<DpiError>,
    status: Status,
    applied: Option<String>,
    conflicts: Vec<Conflict>,
    probing: bool,
    probe: Vec<probe::Outcome>,
    /// Son bakışta motor çalışıyor muydu? (Kendiliğinden durmayı yakalamak için.)
    was_running: bool,
    last_emitted: String,
}

pub struct Dpi {
    pub app: AppHandle,
    data_dir: PathBuf,
    pub system_language: &'static str,
    inner: Mutex<Inner>,
    /// Motor işlemleri sırayla yapılır.
    op: tokio::sync::Mutex<()>,
}

impl Dpi {
    pub fn new(app: AppHandle, data_dir: PathBuf) -> Arc<Self> {
        let settings = Settings::load(&data_dir.join("settings.json"));
        let status = engine::status();
        Arc::new(Self {
            app,
            data_dir,
            system_language: shell::system_language(),
            inner: Mutex::new(Inner {
                settings,
                busy: None,
                error: None,
                status,
                applied: engine::applied_fingerprint(),
                conflicts: engine::conflicts(status.pid),
                probing: false,
                probe: Vec::new(),
                was_running: status.running(),
                last_emitted: String::new(),
            }),
            op: tokio::sync::Mutex::new(()),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn save(&self, settings: &Settings) {
        settings.save(&self.data_dir.join("settings.json"));
    }

    /// Ayarların ve yardımcı iş dosyalarının durduğu klasör.
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn language(&self) -> &'static str {
        match self.lock().settings.language.as_str() {
            "tr" => "tr",
            "en" => "en",
            "ru" => "ru",
            _ => self.system_language,
        }
    }

    /// Motor çalışıyor mu? (Tepsi menüsü için.)
    pub fn running(&self) -> bool {
        self.lock().status.running()
    }

    // ---- durum --------------------------------------------------------------------------------------------------

    pub fn snapshot(&self) -> Value {
        let inner = self.lock();
        let request = EngineRequest::from_settings(&inner.settings);
        let fingerprint = request.as_ref().ok().map(|r| r.fingerprint(&engine::engine_id()));
        let matches = inner.status.installed && fingerprint.is_some() && fingerprint == inner.applied;
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "packaged": !cfg!(debug_assertions),
            "systemLanguage": self.system_language,
            // "windows" | "linux": arayüz sisteme göre değişen metinleri buna göre seçer.
            "platform": std::env::consts::OS,
            // Taşınabilir sürüm: kaldırma ve ayarların yeri farklıdır (shell::portable).
            "portable": crate::shell::portable().is_some(),
            // Linux'ta AppImage: paketin tersine uygulama kendini kaldırabilir (shell::remove_appimage).
            "appImage": crate::shell::appimage().is_some(),
            // Linux'ta uygulamanın kendi kaldırabildiği sistem paketi (.rpm, .deb): shell::remove_package.
            "systemPackage": crate::shell::system_package().is_some(),
            "supported": engine::SUPPORTED,
            "engineReady": engine::available(),
            "settings": inner.settings.public(),
            "catalog": catalog::public(),
            "links": LINKS.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
            "engine": {
                "installed": inner.status.installed,
                "state": inner.status.state,
                "busy": inner.busy,
                // Çalışıyor ama ayarlar sonradan değişti: yeniden uygulamak gerekir.
                "pending": inner.status.running() && request.is_ok() && !matches,
                // Açmak için yönetici onayı istenecek mi?
                "needsAdmin": !matches,
                // Şimdiki ayarlarla motor kurulamıyorsa nedeni: "nothing-selected" | "bad-dns"
                "problem": request.as_ref().err().map(|e| e.code.clone()),
                "error": inner.error,
                // Motorun sayaçları: işlenen bağlantılar, yönlendirilen sorgular, son alan adları.
                "stats": if inner.status.running() { engine::stats() } else { None },
            },
            "conflicts": inner.conflicts,
            "probe": { "running": inner.probing, "results": inner.probe },
        })
    }

    /// Durum değiştiyse arayüze ve tepsiye bildirir.
    pub fn emit(&self) {
        let snapshot = self.snapshot();
        let text = snapshot.to_string();
        {
            let mut inner = self.lock();
            if inner.last_emitted == text {
                return;
            }
            inner.last_emitted = text;
        }
        let _ = self.app.emit("dpi:state", snapshot);
        shell::state_changed(self);
    }

    /// Servisin durumunu yeniden okur. `quiet`: süren bir işlem varken çağrıldı, kendiliğinden durma aranmaz.
    fn refresh(&self, quiet: bool) {
        let status = engine::status();
        let applied = engine::applied_fingerprint();
        let conflicts = engine::conflicts(status.pid);
        let mut inner = self.lock();
        let stopped_alone = inner.was_running && status.state == "stopped" && inner.busy.is_none() && !quiet;
        if stopped_alone && inner.settings.enabled {
            // Motor biz istemeden durdu: çoğunlukla WinDivert sürücüsü yüklenemedi ya da antivirüs engelledi.
            inner.settings.enabled = false;
            inner.error = Some(match engine::stop_reason() {
                Some(code) => DpiError::with("engine-stopped", code),
                None => DpiError::new("engine-stopped"),
            });
            let settings = inner.settings.clone();
            drop(inner);
            self.save(&settings);
            inner = self.lock();
        }
        inner.was_running = status.running();
        inner.status = status;
        inner.applied = applied;
        inner.conflicts = conflicts;
    }

    /// Zamanlayıcıdan (2 sn'de bir): servis dışarıdan durdurulmuş ya da başlatılmış olabilir.
    pub fn poll(&self) {
        if self.lock().busy.is_some() {
            return;
        }
        self.refresh(false);
        self.emit();
    }

    /// Bir motor işlemini sırayla çalıştırır; sonucunu duruma yazar. "cancelled" (yönetici onayı reddedildi) hata
    /// sayılmaz. `enabled`: işlem başarılı olursa kullanıcının istediği durum.
    async fn operate<F>(self: &Arc<Self>, busy: &'static str, enabled: Option<bool>, work: F)
    where
        F: FnOnce() -> DpiResult<()> + Send + 'static,
    {
        let _turn = self.op.lock().await;
        {
            let mut inner = self.lock();
            inner.error = None;
            inner.busy = Some(busy);
        }
        self.emit();

        let result = tauri::async_runtime::spawn_blocking(work).await.unwrap_or_else(|e| Err(DpiError::with("unknown", e)));

        self.refresh(true);
        let settings = {
            let mut inner = self.lock();
            inner.busy = None;
            match result {
                Ok(()) => {
                    if let Some(enabled) = enabled {
                        inner.settings.enabled = enabled;
                    }
                }
                Err(error) if error.is("cancelled") => {}
                Err(error) => inner.error = Some(error),
            }
            // Açma başarısız olduysa "açık" isteği de düşer; yoksa her açılışta yeniden denenirdi.
            if !inner.status.running() && enabled == Some(true) {
                inner.settings.enabled = false;
            }
            inner.settings.clone()
        };
        self.save(&settings);
        self.emit();
    }

    // ---- motor --------------------------------------------------------------------------------------------------

    /// Şimdiki ayarlar için: (istek, uygulanmış ayarla aynı mı?)
    fn current_request(&self) -> DpiResult<(EngineRequest, bool)> {
        let inner = self.lock();
        let request = EngineRequest::from_settings(&inner.settings)?;
        let matches = inner.status.installed && inner.applied.as_deref() == Some(request.fingerprint(&engine::engine_id()).as_str());
        Ok((request, matches))
    }

    fn fail(&self, error: DpiError) {
        self.lock().error = Some(error);
        self.emit();
    }

    /// Motoru açar. Ayarlar daha önce uygulandıysa servis doğrudan başlar; değilse yönetici onayı istenir.
    pub async fn turn_on(self: &Arc<Self>) {
        // Sorumluluk reddi kabul edilmeden (ilk açılış adımları bitmeden) motor açılmaz; tepsi menüsünden de.
        if !self.lock().settings.onboarded {
            return self.fail(DpiError::new("onboarding-required"));
        }
        if !engine::available() {
            return self.fail(DpiError::new(if engine::SUPPORTED { "engine-missing" } else { "unsupported" }));
        }
        let (request, matches) = match self.current_request() {
            Ok(pair) => pair,
            Err(error) => return self.fail(error),
        };
        let jobs = self.data_dir.clone();
        self.operate("starting", Some(true), move || {
            if matches {
                match engine::start() {
                    // Servisin erişim listesi değişmişse yönetici onayıyla yeniden kur.
                    Err(error) if error.is("access-denied") || error.is("not-installed") => {}
                    other => return other,
                }
            }
            engine::elevated("apply", Some(&request), &jobs)
        })
        .await;
    }

    pub async fn turn_off(self: &Arc<Self>) {
        let jobs = self.data_dir.clone();
        self.operate("stopping", Some(false), move || match engine::stop() {
            Err(error) if error.is("access-denied") => engine::elevated("stop", None, &jobs),
            other => other,
        })
        .await;
    }

    /// Motor çalışırken değişen ayarları uygular (yönetici onayı ister).
    pub async fn apply(self: &Arc<Self>) {
        let (request, _) = match self.current_request() {
            Ok(pair) => pair,
            Err(error) => return self.fail(error),
        };
        let jobs = self.data_dir.clone();
        self.operate("applying", Some(true), move || engine::elevated("apply", Some(&request), &jobs)).await;
    }

    /// Servisi, sürücüyü ve korumalı klasörü kaldırır.
    pub async fn remove_all(self: &Arc<Self>) {
        let jobs = self.data_dir.clone();
        self.operate("removing", Some(false), move || engine::elevated("remove", None, &jobs)).await;
    }

    /// Bilgisayarda çalışan GoodbyeDPI'ı durdurur (kullanıcı onay verdikten sonra).
    pub async fn stop_conflicts(self: &Arc<Self>) {
        let jobs = self.data_dir.clone();
        self.operate("fixing", None, move || engine::elevated("conflicts", None, &jobs)).await;
    }

    /// Açılışta: kullanıcı motoru açık bıraktıysa ve ayarlar uygulanmışsa servisi (onay gerekmeden) başlatır.
    pub async fn resume(self: &Arc<Self>) {
        let wanted = {
            let inner = self.lock();
            inner.settings.enabled && inner.settings.onboarded
        };
        let running = self.lock().status.running();
        if !wanted || running {
            return;
        }
        match self.current_request() {
            Ok((_, true)) => self.operate("starting", Some(true), engine::start).await,
            _ => {
                // Yönetici onayı gerekir: açılışta sormayız, kullanıcı düğmeye basınca sorulur.
                let settings = {
                    let mut inner = self.lock();
                    inner.settings.enabled = false;
                    inner.settings.clone()
                };
                self.save(&settings);
                self.emit();
            }
        }
    }

    pub fn dismiss_error(&self) {
        self.lock().error = None;
        self.emit();
    }

    // ---- ayarlar ------------------------------------------------------------------------------------------------

    fn change(&self, edit: impl FnOnce(&mut Settings) -> bool) -> DpiResult<()> {
        let settings = {
            let mut inner = self.lock();
            if !edit(&mut inner.settings) {
                return Err(DpiError::new("bad-request"));
            }
            inner.settings.clone()
        };
        self.save(&settings);
        self.emit();
        Ok(())
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> DpiResult<()> {
        self.change(|settings| settings.set(key, value))
    }

    pub fn set_service(&self, id: &str, on: bool) -> DpiResult<()> {
        self.change(|settings| settings.set_service(id, on))
    }

    /// "Gelişmiş" ekranından tek alanlık strateji değişikliği (yöntemi "özel"e çevirir).
    pub fn set_custom(&self, field: &str, value: &Value) -> DpiResult<()> {
        self.change(|settings| settings.set_custom(field, value))
    }

    /// Kullanıcının yazdığı adresleri ekler: { added: [...], rejected: [...] }
    pub fn add_domains(&self, text: &str) -> Value {
        let mut outcome = (Vec::new(), Vec::new());
        let _ = self.change(|settings| {
            outcome = settings.add_domains(text);
            true
        });
        json!({ "added": outcome.0, "rejected": outcome.1 })
    }

    pub fn remove_domain(&self, domain: &str) {
        let _ = self.change(|settings| {
            settings.remove_domain(domain);
            true
        });
    }

    /// Eklenen bir sitenin anahtarı.
    pub fn set_domain(&self, domain: &str, on: bool) -> DpiResult<()> {
        self.change(|settings| settings.set_domain(domain, on))
    }

    /// "Bütün siteler" kipinde hariç tutulacak adresleri ekler: { added: [...], rejected: [...] }
    pub fn add_excluded(&self, text: &str) -> Value {
        let mut outcome = (Vec::new(), Vec::new());
        let _ = self.change(|settings| {
            outcome = settings.add_excluded(text);
            true
        });
        json!({ "added": outcome.0, "rejected": outcome.1 })
    }

    pub fn remove_excluded(&self, domain: &str) {
        let _ = self.change(|settings| {
            settings.remove_excluded(domain);
            true
        });
    }

    // ---- bağlantı testi -----------------------------------------------------------------------------------------

    /// Seçili uygulamaları ve (ilk birkaç) özel siteyi dener. "Bütün siteler" kipinde hazır uygulamaların hepsi denenir.
    pub async fn test(self: &Arc<Self>) {
        let targets = {
            let mut inner = self.lock();
            if inner.probing {
                return;
            }
            inner.probing = true;
            let all = inner.settings.scope == "all";
            let mut targets: Vec<probe::Target> = catalog::SERVICES
                .iter()
                .filter(|s| all || inner.settings.services.iter().any(|id| id == s.id))
                .map(|s| probe::Target { id: s.id.to_string(), label: s.name.to_string(), url: s.probe.to_string() })
                .collect();
            targets.extend(inner.settings.enabled_domains().take(MAX_PROBED_DOMAINS).map(|domain| probe::Target {
                id: domain.clone(),
                label: domain.clone(),
                url: format!("https://{domain}/"),
            }));
            targets
        };
        self.emit();

        let results = probe::run(targets).await;
        {
            let mut inner = self.lock();
            inner.probing = false;
            inner.probe = results;
        }
        self.emit();
    }
}
