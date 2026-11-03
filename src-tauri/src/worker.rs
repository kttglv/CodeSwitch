use super::{automatic_available, Shared};
use codeswitch_engine::{Action, ActionKind, Input, Layout, Session, Snapshot};
use codeswitch_platform::{self as platform, Event, EventKind, Focus};
use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;

/// Unverified edits in this many different apps pause correction everywhere.
const MAX_SUSPENDED_APPS: usize = 3;

struct Pending {
    before: Focus,
    action: Action,
    started: Instant,
}

pub fn spawn(shared: Arc<Shared>, app: tauri::AppHandle) {
    thread::spawn(move || {
        let receiver = match platform::start() {
            Ok(receiver) => receiver,
            Err(error) => {
                *shared.last_error.lock().unwrap() = Some(error);
                shared.update_paused.store(true, Ordering::Release);
                return;
            }
        };
        shared.worker_stopped.store(false, Ordering::Release);
        let mut session = Session::default();
        let mut pending: Option<Pending> = None;
        let mut previous: Option<Snapshot> = None;
        let mut setting_revision = String::new();
        while !shared.stopping.load(Ordering::Relaxed) {
            let received = receiver.recv_timeout(Duration::from_millis(25));
            if shared.updating.load(Ordering::Acquire) && pending.is_none() {
                platform::capture(false);
                session.reset();
                previous = None;
                shared.update_paused.store(true, Ordering::Release);
                continue;
            }
            shared.update_paused.store(false, Ordering::Release);
            let settings = shared.settings.lock().unwrap().clone();
            let signature = serde_json::to_string(&settings).unwrap_or_default();
            if signature != setting_revision {
                platform::arm_undo(false);
                platform::arm_convert(false);
                session.reset();
                previous = None;
                setting_revision = signature;
                if pending.take().is_some() {
                    platform::finish(false);
                }
            }
            if !settings.enabled
                || !automatic_available(&settings)
                || !platform::permission(false)
                || !platform::hook_active()
            {
                platform::capture(false);
                session.reset();
                previous = None;
                if pending.take().is_some() {
                    platform::finish(false);
                }
                status(
                    &shared,
                    &app,
                    if settings.enabled {
                        "needs-permission"
                    } else {
                        "paused"
                    },
                );
                continue;
            }
            platform::capture(true);
            if *shared.status.lock().unwrap() == "paused" {
                status(&shared, &app, "active");
            }
            if let Some(transaction) = &pending {
                let marker = received
                    .as_ref()
                    .is_ok_and(|event| matches!(event.kind, EventKind::Complete));
                if marker || transaction.started.elapsed() > Duration::from_millis(100) {
                    let transaction = pending.take().unwrap();
                    let result = verified_result(&transaction);
                    if let Some(after) = result {
                        session.committed(
                            &transaction.before.snapshot,
                            &after.snapshot,
                            &transaction.action,
                        );
                        previous = Some(after.snapshot.position());
                        platform::arm_undo(!matches!(
                            transaction.action.kind,
                            ActionKind::Undo { .. }
                        ));
                        platform::finish(true);
                        feedback(&shared, &app, &transaction.action);
                        learn(&shared, &app, &transaction.action);
                        status(&shared, &app, "active");
                    } else {
                        platform::finish(false);
                        session.reset();
                        previous = None;
                        // A partially applied injection must never be retried blindly.
                        fail(
                            &shared,
                            &app,
                            &transaction.before.app,
                            "An edit could not be verified because the field changed.",
                        );
                    }
                }
                continue;
            }
            let Ok(Event {
                revision,
                kind: EventKind::Input(input),
            }) = received
            else {
                continue;
            };
            if matches!(input, Input::Reset) {
                session.reset();
                previous = None;
                continue;
            }
            if revision != platform::revision() {
                session.reset();
                previous = None;
                continue;
            }
            let Some(focus) = settled_focus(&input, revision, previous.as_ref()) else {
                session.reset();
                previous = None;
                platform::arm_convert(false);
                status(&shared, &app, "unsupported-field");
                continue;
            };
            if settings.excluded(&focus.app) {
                session.reset();
                previous = None;
                platform::arm_convert(false);
                status(&shared, &app, "excluded-app");
                continue;
            }
            if suspended(&shared.suspended_apps.lock().unwrap(), &focus.app) {
                session.reset();
                previous = None;
                platform::arm_convert(false);
                status(&shared, &app, "suspended-app");
                continue;
            }
            if focus.snapshot.revision != revision {
                session.reset();
                previous = None;
                continue;
            }
            status(&shared, &app, "active");
            // The convert shortcut is consumed only in a verified, allowed field. A
            // native reset (click, focus change) since this event already disarmed it.
            platform::arm_convert(platform::revision() == revision);
            let input = match input {
                Input::Text(_) => {
                    let Some(character) = focus.snapshot.suffix.chars().last() else {
                        continue;
                    };
                    Input::Text(character.to_string())
                }
                other => other,
            };
            let action = session
                .accept(input, &focus.snapshot, &shared.detector)
                .filter(|action| action.kind != ActionKind::Automatic || !settings.ignores(action));
            previous = Some(focus.snapshot.position());
            if let Some(action) = action {
                match focus.replace(&action) {
                    Ok(true) => {
                        pending = Some(Pending {
                            before: focus,
                            action,
                            started: Instant::now(),
                        });
                    }
                    Ok(false) => {
                        session.reset();
                        previous = None;
                    }
                    Err(error) => {
                        session.reset();
                        previous = None;
                        fail(&shared, &app, &focus.app, &error);
                    }
                }
            }
        }
        if pending.is_some() {
            platform::finish(false);
        }
        platform::capture(false);
        shared.worker_stopped.store(true, Ordering::Release);
    });
}

fn settled_focus(input: &Input, revision: u64, previous: Option<&Snapshot>) -> Option<Focus> {
    // Hooks run before the target app receives its event. Give its accessibility
    // provider a bounded interval to expose the new caret/text; never use stale text.
    for _ in 0..6 {
        if platform::revision() != revision {
            return None;
        }
        thread::sleep(Duration::from_millis(3));
        let Some(focus) = platform::focus() else {
            continue;
        };
        let state = &focus.snapshot;
        let content_matches = match input {
            Input::Text(_) => state
                .suffix
                .chars()
                .last()
                .is_some_and(|c| c.len_utf16() == 1),
            _ => true,
        };
        let caret_matches = previous.is_none_or(|old| {
            if old.field != state.field || old.layout != state.layout {
                return true;
            }
            match input {
                Input::Text(_) => state.caret == old.caret + 1,
                Input::Backspace => state.caret == old.caret - 1,
                _ => true,
            }
        });
        if content_matches && caret_matches && state.revision == revision {
            return Some(focus);
        }
    }
    None
}

fn verified_result(pending: &Pending) -> Option<Focus> {
    for _ in 0..6 {
        let before = &pending.before.snapshot;
        if let Some(after) = platform::focus() {
            if pending.action.matches_result(before, &after.snapshot) {
                return Some(after);
            }
        }
        thread::sleep(Duration::from_millis(3));
    }
    None
}

fn status(shared: &Shared, app: &tauri::AppHandle, value: &str) {
    let mut state = shared.status.lock().unwrap();
    if *state != value {
        *state = value.into();
        drop(state);
        tooltip(shared, app);
        let _ = app.emit("status-changed", value);
    }
}

pub(crate) fn tooltip(shared: &Shared, app: &tauri::AppHandle) {
    let Some(tray) = app.tray_by_id("codeswitch") else {
        return;
    };
    let russian = shared.russian_ui.load(Ordering::Relaxed);
    let label = match (shared.status.lock().unwrap().as_str(), russian) {
        ("active", false) => "Ready to switch",
        ("active", true) => "Готово к работе",
        ("paused", false) => "Paused",
        ("paused", true) => "На паузе",
        ("excluded-app", false) => "Paused in excluded app",
        ("excluded-app", true) => "Пауза в исключённом приложении",
        ("suspended-app", false) => "Paused in this app after an unverified edit",
        ("suspended-app", true) => "Пауза в этом приложении после непроверенной правки",
        ("unsupported-field", false) => "Waiting for a supported field",
        ("unsupported-field", true) => "Ожидание поддерживаемого поля",
        (_, false) => "Needs keyboard access",
        (_, true) => "Нужен доступ к клавиатуре",
    };
    let count = shared.corrections.load(Ordering::Relaxed);
    let _ = tray.set_tooltip(Some(match (count, russian) {
        (0, _) => format!("CodeSwitch · {label}"),
        (n, true) => format!("CodeSwitch · {label} · исправлено слов: {n}"),
        (1, false) => format!("CodeSwitch · {label} · 1 word corrected"),
        (n, false) => format!("CodeSwitch · {label} · {n} words corrected"),
    }));
}

/// Count kept corrections and briefly show the new layout next to the tray icon
/// (macOS menu bar; Windows shows the count in the tray tooltip).
fn feedback(shared: &Arc<Shared>, app: &tauri::AppHandle, action: &Action) {
    if matches!(action.kind, ActionKind::Undo { .. }) {
        // Only this worker thread writes the count, so load-then-subtract is exact.
        if shared.corrections.load(Ordering::Relaxed) > 0 {
            shared.corrections.fetch_sub(1, Ordering::Relaxed);
        }
    } else {
        shared.corrections.fetch_add(1, Ordering::Relaxed);
    }
    tooltip(shared, app);
    let Some(tray) = app.tray_by_id("codeswitch") else {
        return;
    };
    let generation = shared.feedback.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = tray.set_title(Some(match action.target {
        Layout::English => "EN",
        Layout::Russian => "RU",
    }));
    let shared = shared.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1200));
        // A newer correction owns the title until its own timer clears it.
        if shared.feedback.load(Ordering::Relaxed) == generation {
            let _ = tray.set_title(None::<&str>);
        }
    });
}

fn learn(shared: &Shared, app: &tauri::AppHandle, action: &Action) {
    let mut settings = shared.settings.lock().unwrap();
    if !settings.learn_from(action) {
        return;
    }
    if settings.save(&shared.path).is_err() {
        *shared.last_error.lock().unwrap() =
            Some("CodeSwitch could not save the word it learned from your undo.".into());
    }
    drop(settings);
    let _ = app.emit("settings-changed", ());
}

#[derive(Debug, PartialEq)]
enum Suspension {
    App,
    Everywhere,
}

fn suspended(apps: &[String], app: &str) -> bool {
    apps.iter().any(|entry| entry.eq_ignore_ascii_case(app))
}

fn suspend(apps: &mut Vec<String>, app: &str) -> Suspension {
    if !suspended(apps, app) {
        apps.push(app.into());
    }
    if apps.len() >= MAX_SUSPENDED_APPS {
        Suspension::Everywhere
    } else {
        Suspension::App
    }
}

pub fn resume(apps: &mut Vec<String>, app: &str) {
    apps.retain(|entry| !entry.eq_ignore_ascii_case(app));
}

/// Stop correcting in the app where an edit failed, so the user can check its text.
/// Failures across several apps suggest a system-wide problem and pause everything.
fn fail(shared: &Shared, app: &tauri::AppHandle, failed_app: &str, reason: &str) {
    platform::arm_undo(false);
    platform::arm_convert(false);
    let scope = suspend(&mut shared.suspended_apps.lock().unwrap(), failed_app);
    let message = if scope == Suspension::Everywhere {
        let mut settings = shared.settings.lock().unwrap();
        settings.enabled = false;
        let _ = settings.save(&shared.path);
        format!("{reason} Edits failed in several apps, so correction is paused everywhere. Review your recent text before turning it back on.")
    } else {
        format!("{reason} Correction is paused in {failed_app}. Check the text there, then resume.")
    };
    *shared.last_error.lock().unwrap() = Some(message);
    let _ = app.emit("settings-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_suspend_one_app_until_too_many_apps_fail() {
        let mut apps = Vec::new();
        assert_eq!(suspend(&mut apps, "com.apple.TextEdit"), Suspension::App);
        assert!(suspended(&apps, "com.apple.textedit"), "case-insensitive");
        assert!(!suspended(&apps, "com.apple.Notes"));
        assert_eq!(
            suspend(&mut apps, "com.apple.textedit"),
            Suspension::App,
            "the same app is counted once"
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(suspend(&mut apps, "com.apple.Notes"), Suspension::App);
        assert_eq!(
            suspend(&mut apps, "notepad.exe"),
            Suspension::Everywhere,
            "failures in several apps suggest a system-wide problem"
        );
        resume(&mut apps, "COM.APPLE.NOTES");
        assert!(!suspended(&apps, "com.apple.Notes"));
    }
}
