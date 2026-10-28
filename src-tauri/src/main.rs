#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod worker;

use codeswitch_engine::{Candidate, Detector, Layout};
use serde::Serialize;
use settings::Settings;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};
use tauri_plugin_autostart::ManagerExt;

pub struct Shared {
    settings: Mutex<Settings>,
    pub detector: Detector,
    path: PathBuf,
    pub stopping: AtomicBool,
    pub status: Mutex<String>,
    pub last_error: Mutex<Option<String>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    settings: Settings,
    platform: &'static str,
    permission_granted: bool,
    hook_active: bool,
    english_installed: bool,
    russian_installed: bool,
    public_release_approved: bool,
    development_build: bool,
    state: String,
    last_error: Option<String>,
    default_exclusions: Vec<&'static str>,
}

fn release_approved() -> bool {
    env!("CODESWITCH_RELEASE_APPROVED") == "true"
}
fn automatic_available(settings: &Settings) -> bool {
    release_approved() || (cfg!(debug_assertions) && settings.preview_consent)
}

#[tauri::command]
fn get_status(state: State<'_, Arc<Shared>>) -> Status {
    let (english, russian) = codeswitch_platform::layouts();
    Status {
        settings: state.settings.lock().unwrap().clone(),
        platform: std::env::consts::OS,
        permission_granted: codeswitch_platform::permission(false),
        hook_active: codeswitch_platform::hook_active(),
        english_installed: english,
        russian_installed: russian,
        public_release_approved: release_approved(),
        development_build: cfg!(debug_assertions),
        state: state.status.lock().unwrap().clone(),
        last_error: state.last_error.lock().unwrap().clone(),
        default_exclusions: settings::DEFAULT_EXCLUSIONS.to_vec(),
    }
}

#[tauri::command]
fn request_permission() -> bool {
    codeswitch_platform::permission(true)
}

#[tauri::command]
fn preview_word(
    word: String,
    layout: Layout,
    state: State<'_, Arc<Shared>>,
) -> Result<Option<Candidate>, String> {
    if word.len() > 512 {
        return Err("Try a single word of up to 64 characters".into());
    }
    Ok(state.detector.analyze(&word, layout))
}

#[tauri::command]
fn save_settings(
    settings: Settings,
    app: tauri::AppHandle,
    state: State<'_, Arc<Shared>>,
) -> Result<(), String> {
    if settings.enabled && !automatic_available(&settings) {
        return Err("Automatic correction is awaiting release validation. A development build can opt into the preview.".into());
    }
    if settings.enabled && !settings.onboarding_complete {
        return Err("Complete setup before enabling correction".into());
    }
    let (english, russian) = codeswitch_platform::layouts();
    if settings.enabled && (!english || !russian || !codeswitch_platform::permission(false)) {
        return Err(
            "Allow keyboard access and install both standard keyboard layouts first".into(),
        );
    }
    if settings.enabled && !codeswitch_platform::hook_active() {
        return Err(
            "The keyboard input service is not ready. Restart CodeSwitch after granting access."
                .into(),
        );
    }
    let mut current = state.settings.lock().unwrap();
    codeswitch_platform::shortcut(&settings.undo_shortcut)?;
    let old = current.clone();
    let manager = app.autolaunch();
    if settings.launch_at_login != old.launch_at_login {
        if let Err(error) = if settings.launch_at_login {
            manager.enable()
        } else {
            manager.disable()
        } {
            let _ = codeswitch_platform::shortcut(&old.undo_shortcut);
            return Err(format!("Could not change launch at login: {error}"));
        }
    }
    if let Err(error) = settings.save(&state.path) {
        if settings.launch_at_login != old.launch_at_login {
            let _ = if old.launch_at_login {
                manager.enable()
            } else {
                manager.disable()
            };
        }
        let _ = codeswitch_platform::shortcut(&old.undo_shortcut);
        return Err(error);
    }
    *current = settings;
    drop(current);
    *state.last_error.lock().unwrap() = None;
    app.emit("settings-changed", ()).ok();
    Ok(())
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .args(["--background"])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_status,
            request_permission,
            preview_word,
            save_settings
        ])
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let (mut settings, error) = match Settings::load(&path) {
                Ok(value) => (value, None),
                Err(error) => (Settings::default(), Some(error)),
            };
            if !automatic_available(&settings) {
                settings.enabled = false;
            }
            if codeswitch_platform::shortcut(&settings.undo_shortcut).is_err() {
                settings.undo_shortcut = Settings::default().undo_shortcut;
                codeswitch_platform::shortcut(&settings.undo_shortcut).ok();
            }
            let shared = Arc::new(Shared {
                settings: Mutex::new(settings),
                detector: Detector::bundled()?,
                path,
                stopping: AtomicBool::new(false),
                status: Mutex::new("paused".into()),
                last_error: Mutex::new(error),
            });
            app.manage(shared.clone());
            let show = MenuItem::with_id(app, "show", "Open CodeSwitch", true, None::<&str>)?;
            let toggle = MenuItem::with_id(
                app,
                "toggle",
                "Enable / pause correction",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit CodeSwitch", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &toggle, &quit])?;
            TrayIconBuilder::with_id("codeswitch")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/tray.png"
                ))?)
                .icon_as_template(true)
                .tooltip("CodeSwitch · English ↔ Russian")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "toggle" => {
                        let shared = app.state::<Arc<Shared>>();
                        let mut settings = shared.settings.lock().unwrap().clone();
                        settings.enabled = !settings.enabled;
                        if let Err(error) = save_settings(settings, app.clone(), shared) {
                            *app.state::<Arc<Shared>>().last_error.lock().unwrap() = Some(error);
                            show_window(app);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            worker::spawn(shared, app.handle().clone());
            if std::env::args().any(|arg| arg == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    window.hide()?;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("CodeSwitch could not start");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<Arc<Shared>>()
                .stopping
                .store(true, Ordering::Relaxed);
            codeswitch_platform::stop();
        }
    });
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
