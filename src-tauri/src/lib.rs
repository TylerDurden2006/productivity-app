use std::path::{Path, PathBuf};
use std::sync::Mutex;

use flow_core::model::{
    AreaInput, Band, ClarifyInput, GoalInput, HabitInput, HorizonKind, IfFull, PlanAssignment,
    QuickAddInput, ReactivateInput, RecurrenceInput, RecurrencePatch, ResolveInput, SessionKind,
    SettingsPatch, Slot, SplitChild, TaskPatch, TimeOfDay, TriageInput, TryItemInput,
};
use flow_core::{api, db, AppError, ErrorCode, SystemClock};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupView {
    ok: bool,
    error: Option<String>,
    db_path: String,
    backups_dir: String,
    newest_backup: Option<String>,
    data_dir: String,
    configured_data_dir: String,
    old_data_dir: Option<String>,
    data_warning: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    data_dir: Option<String>,
    old_data_dir: Option<String>,
    pending_move: Option<String>,
}

struct AppState {
    conn: Mutex<Option<Connection>>,
    clock: SystemClock,
    startup: StartupView,
}

fn newest_backup(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("db"))
        .max_by_key(|path| path.file_name().map(|value| value.to_owned()))
}

fn prune_backups(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = entries
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with("flow-") && name.ends_with(".db"))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|path| path.file_name().map(|value| value.to_owned()));
    while files.len() > 14 {
        let oldest = files.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
}

fn daily_backup(
    conn: &Connection,
    clock: &dyn flow_core::Clock,
    dir: &Path,
) -> flow_core::Result<()> {
    let today = db::today_string(conn, clock)?;
    let destination = dir.join(format!("flow-{today}.db"));
    if !destination.exists() {
        api::backup_to(conn, &destination)?;
        prune_backups(dir);
    }
    Ok(())
}

fn config_path(app: &tauri::AppHandle) -> PathBuf {
    let config_dir = app
        .path()
        .app_config_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&config_dir);
    config_dir.join("flow-config.json")
}

fn read_config(path: &Path) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

fn write_config(path: &Path, config: &AppConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, path).map_err(|error| error.to_string())
}

fn validate_data_dir(path: &Path, current: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Choose a folder.".into());
    }
    if path == current {
        return Err("Flow already uses this folder.".into());
    }
    std::fs::create_dir_all(path).map_err(|error| format!("Cannot create the folder: {error}"))?;
    let probe = path.join(".flow-write-test");
    std::fs::write(&probe, b"ok")
        .map_err(|error| format!("This folder is not writable: {error}"))?;
    let _ = std::fs::remove_file(&probe);
    if path.join("flow.db").exists() {
        return Err("This folder already contains a Flow database. Choose an empty folder.".into());
    }
    Ok(())
}

fn copy_backups(source: &Path, target: &Path) -> Result<(), String> {
    let source = source.join("backups");
    if !source.exists() {
        return Ok(());
    }
    let destination = target.join("backups");
    std::fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    let entries = std::fs::read_dir(&source).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if path.extension().and_then(|value| value.to_str()) != Some("db") {
            continue;
        }
        if name.to_string_lossy().starts_with("flow-") {
            std::fs::copy(&path, destination.join(name)).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn apply_pending_move(pending: &str, old_dir: &Path) -> Result<PathBuf, String> {
    let target = PathBuf::from(pending);
    std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let target_db = target.join("flow.db");
    let source_db = old_dir.join("flow.db");
    if source_db.exists() {
        let conn = db::open(&source_db).map_err(|error| error.message)?;
        if target_db.exists() {
            let _ = std::fs::remove_file(&target_db);
        }
        api::backup_to(&conn, &target_db).map_err(|error| error.message)?;
        drop(conn);
    }
    copy_backups(old_dir, &target)?;
    if target_db.exists() {
        api::validate_database(&target_db).map_err(|error| error.message)?;
    }
    Ok(target)
}

impl AppState {
    fn initialize(app: &tauri::AppHandle) -> Self {
        let config_path = config_path(app);
        let mut config = read_config(&config_path);
        let default_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        let configured_dir = config
            .data_dir
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| default_dir.clone());
        let mut data_dir = configured_dir.clone();
        let mut data_warning = None;

        if let Some(pending) = config.pending_move.clone() {
            match apply_pending_move(&pending, &configured_dir) {
                Ok(target) => {
                    config.old_data_dir = Some(configured_dir.display().to_string());
                    config.data_dir = Some(target.display().to_string());
                    config.pending_move = None;
                    if let Err(error) = write_config(&config_path, &config) {
                        data_warning = Some(format!(
                            "Data moved, but the folder preference was not saved: {error}"
                        ));
                    }
                    data_dir = target;
                }
                Err(error) => {
                    config.pending_move = None;
                    let _ = write_config(&config_path, &config);
                    data_warning = Some(format!(
                        "The data move failed: {error}. Flow is using the previous folder."
                    ));
                }
            }
        }

        let backups_dir = data_dir.join("backups");
        let db_path = data_dir.join("flow.db");
        let _ = std::fs::create_dir_all(&backups_dir);
        let clock = SystemClock;
        let newest = newest_backup(&backups_dir);
        let newest_display = newest.as_ref().map(|path| path.display().to_string());

        let mut startup = StartupView {
            ok: false,
            error: None,
            db_path: db_path.display().to_string(),
            backups_dir: backups_dir.display().to_string(),
            newest_backup: newest_display,
            data_dir: data_dir.display().to_string(),
            configured_data_dir: config
                .data_dir
                .clone()
                .unwrap_or_else(|| data_dir.display().to_string()),
            old_data_dir: config.old_data_dir.clone(),
            data_warning,
        };

        let mut conn = match db::open(&db_path) {
            Ok(conn) => conn,
            Err(error) => {
                startup.error = Some(match &startup.newest_backup {
                    Some(path) => format!("{} Newest backup: {path}", error.message),
                    None => format!("{} No backup was found.", error.message),
                });
                return Self {
                    conn: Mutex::new(None),
                    clock,
                    startup,
                };
            }
        };

        let status = api::startup_status(&conn);
        if !status.ok {
            let message = status
                .error
                .unwrap_or_else(|| "The database failed its integrity check.".into());
            startup.error = Some(match &startup.newest_backup {
                Some(path) => format!("{message} Newest backup: {path}"),
                None => format!("{message} No backup was found."),
            });
            return Self {
                conn: Mutex::new(None),
                clock,
                startup,
            };
        }

        if let Err(error) = api::reconcile(&mut conn, &clock) {
            eprintln!("startup reconcile failed: {}", error.message);
        }
        if let Err(error) = daily_backup(&conn, &clock, &backups_dir) {
            eprintln!("daily backup failed: {}", error.message);
        }
        startup.ok = true;
        Self {
            conn: Mutex::new(Some(conn)),
            clock,
            startup,
        }
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Option<Connection>>, AppError> {
        self.conn
            .lock()
            .map_err(|_| AppError::internal("database lock was poisoned"))
    }
}

fn required<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<T, AppError> {
    let value = payload
        .get(key)
        .cloned()
        .ok_or_else(|| AppError::missing(format!("{key} is required")))?;
    if value.is_null() {
        return Err(AppError::missing(format!("{key} is required")));
    }
    serde_json::from_value(value)
        .map_err(|error| AppError::missing(format!("invalid {key}: {error}")))
}

fn optional<T: DeserializeOwned>(payload: &Value, key: &str) -> Result<Option<T>, AppError> {
    match payload.get(key) {
        None => Ok(None),
        Some(value) if value.is_null() => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| AppError::missing(format!("invalid {key}: {error}"))),
    }
}

fn value<T: Serialize>(input: T) -> Result<Value, AppError> {
    serde_json::to_value(input).map_err(AppError::from)
}

fn is_read_action(action: &str) -> bool {
    matches!(
        action,
        "get_mode"
            | "get_plan_view"
            | "get_do_view"
            | "get_wrapup_view"
            | "get_stats"
            | "get_settings"
            | "get_default_target"
            | "get_budget"
            | "get_daily_view"
            | "get_horizon_view"
            | "get_dashboard"
            | "get_note"
            | "list_areas"
            | "list_habits"
            | "get_habit_today"
            | "get_habit_history"
            | "get_habit_stats"
            | "list_try_items"
            | "list_recurrences"
            | "suggest_plan"
            | "export_json"
            | "validate_database"
    )
}

#[tauri::command]
fn get_startup(state: State<'_, AppState>) -> StartupView {
    state.startup.clone()
}

#[tauri::command]
fn prepare_data_move(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Value, AppError> {
    let current = PathBuf::from(&state.startup.data_dir);
    let target = PathBuf::from(&path);
    validate_data_dir(&target, &current)
        .map_err(|message| AppError::hard(ErrorCode::InvalidSetting, message))?;
    {
        let guard = state.connection()?;
        if let Some(conn) = guard.as_ref() {
            if db::active_session(conn, &state.clock)?.is_some() {
                return Err(AppError::invalid_transition(
                    "stop the active session before moving Flow data",
                ));
            }
        }
    }
    let config_path = config_path(&app);
    let mut config = read_config(&config_path);
    if config.data_dir.is_none() {
        config.data_dir = Some(state.startup.data_dir.clone());
    }
    config.pending_move = Some(target.display().to_string());
    write_config(&config_path, &config)
        .map_err(|message| AppError::hard(ErrorCode::InvalidSetting, message))?;
    Ok(json!({ "ok": true, "target": target.display().to_string() }))
}

#[tauri::command]
fn cancel_data_move(app: tauri::AppHandle) -> Result<Value, AppError> {
    let path = config_path(&app);
    let mut config = read_config(&path);
    config.pending_move = None;
    write_config(&path, &config)
        .map_err(|message| AppError::hard(ErrorCode::InvalidSetting, message))?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn open_data_folder(state: State<'_, AppState>) -> Result<Value, AppError> {
    let target = PathBuf::from(&state.startup.data_dir);
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("explorer").arg(&target).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&target).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&target).spawn()
    };
    result.map_err(|error| AppError::internal(format!("could not open the folder: {error}")))?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

#[tauri::command]
fn run_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    action: String,
    payload: Option<Value>,
) -> Result<Value, AppError> {
    let payload = payload.unwrap_or_else(|| json!({}));
    let mut guard = state.connection()?;
    let conn = guard.as_mut().ok_or_else(|| {
        AppError::internal(
            state
                .startup
                .error
                .clone()
                .unwrap_or_else(|| "database is unavailable".into()),
        )
    })?;
    let clock = &state.clock;
    let result = match action.as_str() {
        "get_mode" => value(api::mode(conn, clock)?)?,
        "get_plan_view" => {
            let day = optional::<String>(&payload, "day")?;
            value(api::plan_view(conn, clock, day)?)?
        }
        "get_daily_view" => {
            let day = optional::<String>(&payload, "day")?;
            value(api::daily_view(conn, clock, day)?)?
        }
        "reorder_task" => {
            let day: String = required(&payload, "day")?;
            let id: String = required(&payload, "id")?;
            let target_slot: Slot = required(&payload, "targetSlot")?;
            let target_index: i64 = required(&payload, "targetIndex")?;
            api::reorder_task(conn, clock, &day, &id, target_slot, target_index)?;
            json!({ "ok": true })
        }
        "reorder_goal" => {
            let kind: HorizonKind = required(&payload, "kind")?;
            let period_start: String = required(&payload, "periodStart")?;
            let id: String = required(&payload, "id")?;
            let target_band: Band = required(&payload, "targetBand")?;
            let target_index: i64 = required(&payload, "targetIndex")?;
            api::reorder_goal(conn, kind, &period_start, &id, target_band, target_index)?;
            json!({ "ok": true })
        }
        "get_dashboard" => {
            let range = payload
                .get("range")
                .and_then(Value::as_str)
                .unwrap_or("WEEK")
                .to_string();
            let date = optional::<String>(&payload, "date")?;
            value(api::dashboard_view(conn, clock, &range, date)?)?
        }
        "get_note" => {
            let scope: String = required(&payload, "scope")?;
            let period_start: String = required(&payload, "periodStart")?;
            value(api::note(conn, &scope, &period_start)?)?
        }
        "save_note" => {
            let scope: String = required(&payload, "scope")?;
            let period_start: String = required(&payload, "periodStart")?;
            let body: String = required(&payload, "bodyMarkdown")?;
            value(api::save_note(conn, clock, &scope, &period_start, &body)?)?
        }
        "get_horizon_view" => {
            let kind: HorizonKind = required(&payload, "kind")?;
            let period_start: Option<String> = optional(&payload, "periodStart")?;
            value(api::horizon_view(conn, clock, kind, period_start)?)?
        }
        "create_goal" => {
            let input: GoalInput = required(&payload, "input")?;
            value(api::create_goal(conn, clock, input)?)?
        }
        "create_goal_quick" => {
            let input: GoalInput = required(&payload, "input")?;
            value(api::create_goal_quick(conn, clock, input)?)?
        }
        "update_goal" => {
            let id: String = required(&payload, "id")?;
            let input: GoalInput = required(&payload, "input")?;
            value(api::update_goal(conn, clock, &id, input)?)?
        }
        "complete_goal" => {
            let id: String = required(&payload, "id")?;
            value(api::complete_goal(conn, clock, &id)?)?
        }
        "reopen_goal" => {
            let id: String = required(&payload, "id")?;
            value(api::reopen_goal(conn, clock, &id)?)?
        }
        "link_task_goal" => {
            let task_id: String = required(&payload, "taskId")?;
            let goal_id: String = required(&payload, "goalId")?;
            api::link_task(conn, clock, &task_id, &goal_id)?;
            json!({ "ok": true })
        }
        "unlink_task_goal" => {
            let task_id: String = required(&payload, "taskId")?;
            let goal_id: String = required(&payload, "goalId")?;
            api::unlink_task(conn, clock, &task_id, &goal_id)?;
            json!({ "ok": true })
        }
        "set_horizon_note" => {
            let kind: HorizonKind = required(&payload, "kind")?;
            let period_start: String = required(&payload, "periodStart")?;
            let intent: Option<String> = optional(&payload, "intent")?;
            let review_note: Option<String> = optional(&payload, "reviewNote")?;
            api::set_horizon_note(
                conn,
                clock,
                kind,
                period_start.as_str(),
                intent,
                review_note,
            )?;
            json!({ "ok": true })
        }
        "list_areas" => value(api::areas(conn)?)?,
        "create_area" => {
            let input: AreaInput = required(&payload, "input")?;
            value(api::create_area(conn, clock, input)?)?
        }
        "list_habits" => value(api::habits(conn)?)?,
        "create_habit" => {
            let input: HabitInput = required(&payload, "input")?;
            value(api::create_habit(conn, clock, input)?)?
        }
        "update_habit" => {
            let id: String = required(&payload, "id")?;
            let input: HabitInput = required(&payload, "input")?;
            value(api::update_habit(conn, clock, &id, input)?)?
        }
        "archive_habit" => {
            let id: String = required(&payload, "id")?;
            api::archive_habit(conn, clock, &id)?;
            json!({ "ok": true })
        }
        "get_habit_today" => {
            let day = optional::<String>(&payload, "day")?;
            value(api::habits_today(conn, clock, day)?)?
        }
        "complete_habit" => {
            let id: String = required(&payload, "id")?;
            let day: String = required(&payload, "day")?;
            let count: Option<i64> = optional(&payload, "count")?;
            let note: Option<String> = optional(&payload, "note")?;
            value(api::complete_habit(conn, clock, &id, &day, count, note)?)?
        }
        "skip_habit" => {
            let id: String = required(&payload, "id")?;
            let day: String = required(&payload, "day")?;
            let note: Option<String> = optional(&payload, "note")?;
            value(api::skip_habit(conn, clock, &id, &day, note)?)?
        }
        "reopen_habit" => {
            let id: String = required(&payload, "id")?;
            let day: String = required(&payload, "day")?;
            api::reopen_habit(conn, &id, &day)?;
            json!({ "ok": true })
        }
        "get_habit_history" => {
            let id: String = required(&payload, "id")?;
            let from: String = required(&payload, "from")?;
            let to: String = required(&payload, "to")?;
            value(api::habit_history(conn, &id, &from, &to)?)?
        }
        "get_habit_stats" => {
            let id: String = required(&payload, "id")?;
            let day = optional::<String>(&payload, "day")?;
            value(api::habit_stats(conn, clock, &id, day)?)?
        }
        "list_try_items" => value(api::try_items(conn)?)?,
        "create_try_item" => {
            let input: TryItemInput = required(&payload, "input")?;
            value(api::create_try_item(conn, clock, input)?)?
        }
        "update_try_item" => {
            let id: String = required(&payload, "id")?;
            let input: TryItemInput = required(&payload, "input")?;
            value(api::update_try_item(conn, clock, &id, input)?)?
        }
        "schedule_try_item" => {
            let id: String = required(&payload, "id")?;
            let day: String = required(&payload, "day")?;
            let day_part: TimeOfDay = required(&payload, "dayPart")?;
            value(api::schedule_try_item(conn, clock, &id, &day, day_part)?)?
        }
        "unschedule_try_item" => {
            let id: String = required(&payload, "id")?;
            value(api::unschedule_try_item(conn, clock, &id)?)?
        }
        "mark_try_item_tried" => {
            let id: String = required(&payload, "id")?;
            let rating: Option<i64> = optional(&payload, "rating")?;
            let reflection: Option<String> = optional(&payload, "reflection")?;
            value(api::mark_try_item_tried(
                conn, clock, &id, rating, reflection,
            )?)?
        }
        "skip_try_item" => {
            let id: String = required(&payload, "id")?;
            value(api::skip_try_item(conn, clock, &id)?)?
        }
        "set_try_item_task" => {
            let id: String = required(&payload, "id")?;
            let task_id: Option<String> = optional(&payload, "taskId")?;
            value(api::set_try_item_task(conn, clock, &id, task_id)?)?
        }
        "get_do_view" => value(api::do_view(conn, clock)?)?,
        "get_wrapup_view" => value(api::wrapup_view(conn, clock)?)?,
        "get_stats" => value(api::stats_view(conn, clock)?)?,
        "get_settings" => value(api::settings(conn)?)?,
        "get_default_target" => value(api::default_target(conn, clock)?)?,
        "get_budget" => {
            let day: String = required(&payload, "day")?;
            value(api::budget(conn, &day)?)?
        }
        "list_recurrences" => value(api::recurrences(conn, clock)?)?,
        "suggest_plan" => {
            let day: String = required(&payload, "day")?;
            value(api::suggest_plan(conn, clock, &day)?)?
        }
        "update_settings" => {
            let patch: SettingsPatch = required(&payload, "patch")?;
            value(api::update_settings(conn, clock, patch)?)?
        }
        "capture_task" => {
            let title: String = required(&payload, "title")?;
            value(api::capture_task(conn, clock, &title)?)?
        }
        "add_task" => {
            let input: QuickAddInput = required(&payload, "input")?;
            value(api::add_task(conn, clock, input)?)?
        }
        "clarify_task" => {
            let id: String = required(&payload, "id")?;
            let input: ClarifyInput = required(&payload, "input")?;
            let if_full: Option<IfFull> = optional(&payload, "ifFull")?;
            let reason: Option<String> = optional(&payload, "overrideReason")?;
            value(api::clarify_task(conn, clock, &id, input, if_full, reason)?)?
        }
        "reactivate_task" => {
            let id: String = required(&payload, "id")?;
            let input: Option<ReactivateInput> = optional(&payload, "input")?;
            let if_full: Option<IfFull> = optional(&payload, "ifFull")?;
            value(api::reactivate_task(conn, clock, &id, input, if_full)?)?
        }
        "split_task" => {
            let id: String = required(&payload, "id")?;
            let children: Vec<SplitChild> = required(&payload, "children")?;
            value(api::split_task(conn, clock, &id, children)?)?
        }
        "update_task" => {
            let id: String = required(&payload, "id")?;
            let patch: TaskPatch = required(&payload, "patch")?;
            value(api::update_task(conn, clock, &id, patch)?)?
        }
        "triage_task" => {
            let id: String = required(&payload, "id")?;
            let input: TriageInput = required(&payload, "input")?;
            value(api::triage_task(conn, clock, &id, input)?)?
        }
        "restore_task" => {
            let id: String = required(&payload, "id")?;
            value(api::restore_task(conn, clock, &id)?)?
        }
        "set_pinned" => {
            let id: String = required(&payload, "id")?;
            let pinned: bool = required(&payload, "pinned")?;
            value(api::set_pinned(conn, clock, &id, pinned)?)?
        }
        "assign_task" => {
            let day: String = required(&payload, "day")?;
            let id: String = required(&payload, "id")?;
            let slot: Slot = required(&payload, "slot")?;
            let reason: Option<String> = optional(&payload, "overrideReason")?;
            value(api::assign_task(conn, clock, &day, &id, slot, reason)?)?
        }
        "assign_task_quick" => {
            let day: String = required(&payload, "day")?;
            let id: String = required(&payload, "id")?;
            let slot: Slot = required(&payload, "slot")?;
            value(api::assign_task_quick(conn, clock, &day, &id, slot)?)?
        }
        "unassign_task" => {
            let id: String = required(&payload, "id")?;
            let reason: Option<String> = optional(&payload, "overrideReason")?;
            value(api::unassign_task(conn, clock, &id, reason)?)?
        }
        "swap_plan" => {
            let day: String = required(&payload, "day")?;
            let out_id: String = required(&payload, "outId")?;
            let in_id: String = required(&payload, "inId")?;
            let reason: Option<String> = optional(&payload, "reason")?;
            api::swap_plan(conn, clock, &day, &out_id, &in_id, reason)?;
            json!({ "ok": true })
        }
        "pull_extra" => {
            let id: String = required(&payload, "id")?;
            value(api::pull_extra(conn, clock, &id)?)?
        }
        "lock_plan" => {
            let day: String = required(&payload, "day")?;
            let reason: Option<String> = optional(&payload, "overrideReason")?;
            value(api::lock_plan(conn, clock, &day, reason)?)?
        }
        "apply_plan" => {
            let day: String = required(&payload, "day")?;
            let assignments: Vec<PlanAssignment> = required(&payload, "assignments")?;
            value(api::apply_plan(conn, clock, &day, &assignments)?)?
        }
        "complete_task" => {
            let id: String = required(&payload, "id")?;
            let manual: Option<i64> = optional(&payload, "manualMins")?;
            value(api::complete_task(conn, clock, &id, manual)?)?
        }
        "drop_task" => {
            let id: String = required(&payload, "id")?;
            let reason: Option<String> = optional(&payload, "reason")?;
            value(api::drop_task(conn, clock, &id, reason)?)?
        }
        "undo_last" => value(api::undo_last(conn, clock)?)?,
        "resolve_task" => {
            let id: String = required(&payload, "id")?;
            let input: ResolveInput = required(&payload, "input")?;
            value(api::resolve_task(conn, clock, &id, input)?)?
        }
        "start_session" => {
            let kind: SessionKind = required(&payload, "kind")?;
            let task_id: Option<String> = optional(&payload, "taskId")?;
            value(api::start_session(conn, clock, kind, task_id)?)?
        }
        "pause_session" => value(api::pause_session(conn, clock)?)?,
        "resume_session" => value(api::resume_session(conn, clock)?)?,
        "stop_session" => {
            let breadcrumb: Option<String> = optional(&payload, "breadcrumb")?;
            value(api::stop_session(conn, clock, breadcrumb)?)?
        }
        "extend_session" => value(api::extend_session(conn, clock)?)?,
        "heartbeat" => {
            api::heartbeat(conn, clock)?;
            json!({ "ok": true })
        }
        "reconcile" => value(api::reconcile(conn, clock)?)?,
        "carry_all" => value(api::carry_all(conn, clock)?)?,
        "complete_wrapup" => {
            let day: String = required(&payload, "day")?;
            let win_note: Option<String> = optional(&payload, "winNote")?;
            value(api::complete_wrapup(conn, clock, &day, win_note)?)?
        }
        "reopen_wrapup" => {
            let day: Option<String> = optional(&payload, "day")?;
            value(api::reopen_wrapup(conn, clock, day)?)?
        }
        "set_win_note" => {
            let note: Option<String> = optional(&payload, "note")?;
            value(api::set_win_note(conn, clock, note)?)?
        }
        "create_recurrence" => {
            let input: RecurrenceInput = required(&payload, "input")?;
            value(api::create_recurrence(conn, clock, input)?)?
        }
        "update_recurrence" => {
            let id: String = required(&payload, "id")?;
            let patch: RecurrencePatch = required(&payload, "patch")?;
            value(api::update_recurrence(conn, clock, &id, patch)?)?
        }
        "delete_recurrence" => {
            let id: String = required(&payload, "id")?;
            api::delete_recurrence(conn, &id)?;
            json!({ "ok": true })
        }
        "mark_weekly_review" => {
            api::mark_weekly_review(conn, clock)?;
            json!({ "ok": true })
        }
        "export_json" => {
            let path: String = required(&payload, "path")?;
            let exported = api::export_json(conn, clock)?;
            let path = PathBuf::from(path);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&path, serde_json::to_string_pretty(&exported)?)?;
            json!({ "path": path.display().to_string() })
        }
        "backup_db" => {
            let path: String = required(&payload, "path")?;
            let path = PathBuf::from(path);
            api::backup_to(conn, &path)?;
            json!({ "path": path.display().to_string() })
        }
        "validate_database" => {
            let path: String = required(&payload, "path")?;
            api::validate_database(Path::new(&path))?;
            json!({ "ok": true })
        }
        other => {
            return Err(AppError::hard(
                ErrorCode::Internal,
                format!("unknown action {other}"),
            ))
        }
    };
    if !is_read_action(&action) {
        let _ = app.emit("data-changed", ());
    }
    Ok(result)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_startup,
            run_action,
            prepare_data_move,
            cancel_data_move,
            open_data_folder,
            restart_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flow");
}
