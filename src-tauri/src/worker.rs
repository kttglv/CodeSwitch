use super::{automatic_available, Shared};
use codeswitch_engine::{Action, Input, Session, Snapshot};
use codeswitch_platform::{self as platform, Event, EventKind, Focus};
use std::{
    sync::{atomic::Ordering, Arc},
    thread,
    time::{Duration, Instant},
};
use tauri::Emitter;

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
                return;
            }
        };
        let mut session = Session::default();
        let mut pending: Option<Pending> = None;
        let mut previous: Option<Snapshot> = None;
        let mut setting_revision = String::new();
        while !shared.stopping.load(Ordering::Relaxed) {
            let received = receiver.recv_timeout(Duration::from_millis(25));
            let settings = shared.settings.lock().unwrap().clone();
            let signature = serde_json::to_string(&settings).unwrap_or_default();
            if signature != setting_revision {
                platform::arm_undo(false);
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
                        platform::arm_undo(!transaction.action.undo);
                        platform::finish(true);
                        status(&shared, &app, "active");
                    } else {
                        platform::finish(false);
                        session.reset();
                        previous = None;
                        // A partially applied injection must never be retried blindly.
                        pause(&shared, "The edited field changed before replacement could be verified. Review the text before resuming.");
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
                status(&shared, &app, "unsupported-field");
                continue;
            };
            if settings.excluded(&focus.app) {
                session.reset();
                previous = None;
                status(&shared, &app, "excluded-app");
                continue;
            }
            if focus.snapshot.revision != revision {
                session.reset();
                previous = None;
                continue;
            }
            status(&shared, &app, "active");
            let input = match input {
                Input::Text(_) => {
                    let Some(character) = focus.snapshot.suffix.chars().last() else {
                        continue;
                    };
                    Input::Text(character.to_string())
                }
                other => other,
            };
            let action = session.accept(input, &focus.snapshot, &shared.detector);
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
                        pause(&shared, &error);
                    }
                }
            }
        }
        if pending.is_some() {
            platform::finish(false);
        }
        platform::capture(false);
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
        if let Some(tray) = app.tray_by_id("codeswitch") {
            let label = match value {
                "active" => "Ready to switch",
                "paused" => "Paused",
                "excluded-app" => "Paused in excluded app",
                "unsupported-field" => "Waiting for a supported field",
                _ => "Needs keyboard access",
            };
            let _ = tray.set_tooltip(Some(format!("CodeSwitch · {label}")));
        }
        let _ = app.emit("status-changed", value);
    }
}

fn pause(shared: &Shared, error: &str) {
    platform::arm_undo(false);
    let mut settings = shared.settings.lock().unwrap();
    settings.enabled = false;
    let _ = settings.save(&shared.path);
    *shared.last_error.lock().unwrap() = Some(error.into());
}
