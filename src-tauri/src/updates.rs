use crate::Shared;
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: &'static str,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

#[derive(Default)]
struct Data {
    status: Status,
    candidate: Option<Update>,
    last_check: Option<Instant>,
}

#[derive(Default)]
pub struct Updates {
    data: Mutex<Data>,
    busy: AtomicBool,
    menu_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
    russian: AtomicBool,
}

struct Operation<'a>(&'a AtomicBool);
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Updates {
    pub fn set_menu_item(&self, item: tauri::menu::MenuItem<tauri::Wry>) {
        *self.menu_item.lock().unwrap() = Some(item);
    }

    pub fn set_russian(&self, russian: bool) {
        self.russian.store(russian, Ordering::Relaxed);
        self.refresh_menu();
    }

    fn refresh_menu(&self) {
        let version = self.status().version;
        let russian = self.russian.load(Ordering::Relaxed);
        if let Some(item) = self.menu_item.lock().unwrap().as_ref() {
            let title = match (version, russian) {
                (None, false) => "Check for updates…".into(),
                (None, true) => "Проверить обновления…".into(),
                (Some(v), false) => format!("Update available: {v}…"),
                (Some(v), true) => format!("Доступно обновление: {v}…"),
            };
            item.set_text(title).ok();
        }
    }
    fn begin(&self) -> Option<Operation<'_>> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Operation(&self.busy))
    }

    pub fn status(&self) -> Status {
        self.data.lock().unwrap().status.clone()
    }

    fn error(&self, message: &str) {
        let mut data = self.data.lock().unwrap();
        data.status.phase = "error";
        data.status.message = Some(message.into());
    }
}

fn due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.saturating_duration_since(last) >= CHECK_INTERVAL)
}

// No endpoint, public key or download URL can be supplied by the webview.
fn trusted_download(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .path()
            .starts_with("/kttglv/CodeSwitch/releases/download/")
}

pub async fn check(app: &AppHandle, automatic: bool) -> Status {
    let updates = app.state::<Updates>();
    let Some(_operation) = updates.begin() else {
        return updates.status();
    };
    {
        let mut data = updates.data.lock().unwrap();
        if automatic && !due(data.last_check, Instant::now()) {
            return data.status.clone();
        }
        data.last_check = Some(Instant::now());
        data.status.phase = "checking";
        data.status.message = None;
    }
    let result = async {
        app.updater_builder()
            .timeout(Duration::from_secs(20))
            .build()?
            .check()
            .await
    }
    .await;
    match result {
        Ok(Some(mut candidate)) if trusted_download(&candidate.download_url) => {
            candidate.timeout = Some(Duration::from_secs(10 * 60));
            let mut data = updates.data.lock().unwrap();
            data.status = Status {
                phase: "available", version: Some(candidate.version.clone()),
                notes: candidate.body.clone(), ..Status::default()
            };
            data.candidate = Some(candidate);
        }
        Ok(Some(_)) => updates.error("The release uses an unsupported download address. Installation was blocked."),
        Ok(None) => {
            let mut data = updates.data.lock().unwrap();
            data.candidate = None;
            data.status = Status { phase: "current", ..Status::default() };
        }
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => updates.error(
            "The update feed is unavailable. A public signed GitHub Release may not be published yet. Try again later."),
        Err(_) => updates.error("Could not check for updates. Check your connection and try again."),
    }
    updates.refresh_menu();
    updates.status()
}

struct Pause<'a>(&'a AtomicBool);
impl Drop for Pause<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub async fn install(app: AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let _operation = updates
        .begin()
        .ok_or("Another update operation is running")?;
    let candidate = {
        let mut data = updates.data.lock().unwrap();
        let candidate = data.candidate.clone().ok_or("Check for an update first")?;
        data.status.phase = "downloading";
        data.status.message = None;
        data.status.downloaded = 0;
        data.status.total = None;
        candidate
    };
    // The plugin authenticates both bytes and the announced version before this
    // returns. A failed download leaves correction running and can be retried.
    let bytes = match candidate
        .download(
            |chunk, total| {
                let mut data = updates.data.lock().unwrap();
                data.status.downloaded = data.status.downloaded.saturating_add(chunk as u64);
                data.status.total = total;
            },
            || {},
        )
        .await
    {
        Ok(bytes) => bytes,
        Err(_) => {
            let message =
                "Download or signature verification failed. Nothing was installed. Try again.";
            updates.error(message);
            return Err(message.into());
        }
    };
    let shared = app.state::<std::sync::Arc<Shared>>();
    shared.updating.store(true, Ordering::Release);
    shared.update_paused.store(false, Ordering::Release);
    let _pause = Pause(&shared.updating);
    // Let the serialized worker finish any pending correction and replay before
    // replacing files or restarting. Never stop an in-flight text transaction.
    let deadline = Instant::now() + Duration::from_secs(2);
    while !shared.update_paused.load(Ordering::Acquire)
        && !shared.worker_stopped.load(Ordering::Acquire)
    {
        if Instant::now() >= deadline {
            let message = "Correction is still busy. The update was not installed. Try again.";
            updates.error(message);
            return Err(message.into());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    updates.data.lock().unwrap().status.phase = "installing";
    let result = tauri::async_runtime::spawn_blocking(move || candidate.install(bytes)).await;
    match result {
        Ok(Ok(())) => app.restart(), // Windows installer exits/restarts on its own.
        _ => {
            let message = "Could not install the update. Correction will resume. Try again or install the release manually.";
            updates.error(message);
            Err(message.into())
        }
    }
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let shared = app.state::<std::sync::Arc<Shared>>();
            if shared.stopping.load(Ordering::Relaxed) {
                break;
            }
            let enabled = shared.settings.lock().unwrap().automatic_update_checks;
            if enabled {
                check(&app, true).await;
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_packages_reject_tampering_and_falsified_versions_without_network() {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({
                "pubkey": include_str!("../tests/fixtures/updater/public.key").trim(),
                "endpoints": ["https://example.invalid/update"],
                "requireSignedVersion": true
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        let updater = app
            .updater_builder()
            .target("fixture")
            .executable_path("CodeSwitch.app/Contents/MacOS/codeswitch")
            .configure_client(|_| panic!("this test must not use the network"))
            .build()
            .unwrap();
        let mut manifest = serde_json::json!({
            "version": "9.9.9", "platforms": { "fixture": {
                "url": "https://github.com/kttglv/CodeSwitch/releases/download/v9.9.9/fixture",
                "signature": include_str!("../tests/fixtures/updater/payload.sig").trim()
            }}
        });
        let payload = include_bytes!("../tests/fixtures/updater/payload.txt");
        let candidate = updater.restore_update(manifest.clone()).unwrap().unwrap();
        candidate.verify(payload).unwrap();
        assert!(candidate.verify(b"modified package").is_err());
        manifest["version"] = "9.9.10".into();
        let candidate = updater.restore_update(manifest.clone()).unwrap().unwrap();
        assert!(matches!(
            candidate.verify(payload),
            Err(tauri_plugin_updater::Error::SignedVersionMismatch { .. })
        ));
        manifest["version"] = "0.0.0".into();
        assert!(updater.restore_update(manifest).unwrap().is_none());
    }

    #[test]
    fn update_checks_are_throttled_and_operations_release_after_failure() {
        let now = Instant::now();
        assert!(due(None, now));
        assert!(!due(
            Some(now),
            now + CHECK_INTERVAL - Duration::from_secs(1)
        ));
        assert!(due(Some(now), now + CHECK_INTERVAL));
        let updates = Updates::default();
        let operation = updates.begin().unwrap();
        assert!(updates.begin().is_none());
        updates.error("retry");
        drop(operation);
        assert!(updates.begin().is_some());
    }

    #[test]
    fn only_https_release_assets_from_the_app_repository_are_accepted() {
        for url in [
            "http://github.com/kttglv/CodeSwitch/releases/download/v1/app.tar.gz",
            "https://github.com/other/repo/releases/download/v1/app.tar.gz",
            "https://github.com.evil.test/kttglv/CodeSwitch/releases/download/v1/app.tar.gz",
            "https://user@github.com/kttglv/CodeSwitch/releases/download/v1/app.tar.gz",
            "https://github.com:444/kttglv/CodeSwitch/releases/download/v1/app.tar.gz",
            "file:///tmp/app.tar.gz",
        ] {
            assert!(!trusted_download(&url.parse().unwrap()), "{url}");
        }
        assert!(trusted_download(
            &"https://github.com/kttglv/CodeSwitch/releases/download/v1/app.tar.gz"
                .parse()
                .unwrap()
        ));
    }
}
