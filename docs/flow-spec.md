# Flow: Build Specification for Coding Agents (v2)

*Local-first focus app · Rust core + Tauri v2 + SQLite · Spec v2.1 · 23 Oct 2026*

## 0. How to use this document

This is the single source of truth for building **Flow** (working name). It defines behavior, data, rules and tests. It deliberately contains almost no implementation code: you write the code, this document defines what must be true.

Working rules for the agent:

1. Build in the slice order of section 9. Finish and verify one slice before starting the next.
2. Before implementing a rule, write its acceptance test (section 8) in `core/tests/`. A slice is done when its listed tests pass and the CI gates below are green.
3. All business logic lives in the `flow-core` crate. The Tauri layer and the UI contain no rules; they call commands and render results.
4. CI gates for every slice: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, `npm run build`.
5. Fully offline: no network crates, no telemetry, no CDN fonts or scripts. Tauri CSP is `default-src 'self'`.
6. No placeholder or stub code on shipped paths. If something belongs to a later slice, do not create it yet.
7. If this document is ambiguous, pick the simplest behavior and record it in one line in `DECISIONS.md`. Do not add dependencies beyond section 3 without a DECISIONS.md entry.
8. If two parts of this document conflict, the rule IDs in section 5 win over the tests, schema and screens. Record the conflict in `DECISIONS.md` and carry on.
9. At the end of each slice, append to `PROGRESS.md`: the slice number, the tests now passing, decisions made, and known gaps. A human reads this file to review your work.

## 1. Product in one page

**Goal:** the person opens the app and always knows exactly what to do next. Planning happens once, in a calm moment (usually the evening before). During work the app shows one task, its first physical step and a timer. Everything else is hidden.

**Two modes, derived and never stored:**

- **Plan**: inbox, Ready list, building the day, and the evening wrap-up wizard.
- **Do**: one task on screen, no lists. Shown automatically when today’s plan is locked.

**The daily plan (1-3-5):**

- **ONE**: a single important task (the Apex). Done first.
- **THREE**: up to three core tasks.
- **BATCH**: up to five micro tasks (estimate 15 min) done inside one 45-minute block.
- **EXTRA**: tasks pulled in after everything planned is done (finishing early).

**A day in the app**

1. Any time: press the capture hotkey, type a title, press Enter. It lands in the Inbox without leaving the current app.
2. Evening (wrap-up wizard): settle today’s unfinished tasks, clear the Inbox (title, first step and estimate for each), press *Suggest plan* for tomorrow, adjust, lock.
3. Morning: open the app. Do mode shows the ONE with its first step. Press Space.
4. Work through ONE, THREE and the BATCH block. Done advances automatically; stopping mid-task asks where to resume.
5. When everything planned is done, the All done screen offers one more task from Ready, or the wrap-up.

**Design principles (use these whenever the spec is silent):**

1. Upkeep must stay cheap. A task needs only a title, a first step and an estimate bucket. Everything else has a default.
2. Soft gates, not walls. A rule that guards a preference can be overridden with a one-line reason, which is logged. Only rules that protect data integrity or the core idea are hard.
3. The backlog is invisible in Do mode, and this is enforced by the API (the Do view never returns non-planned tasks), not only by the UI.
4. Rust computes, the UI displays. Anything derived (mode, calibrated estimate, budget, stuck flag) is computed on read and never stored, unless this document says otherwise.
5. Propose, don’t interrogate. Wherever the user must choose, the app offers a default first (a suggested plan, a default estimate bucket, a default review date), so the common path is one tap.

## 2. Glossary

| Term | Definition |
|---|---|
| Logical day | The local date after subtracting `day_rollover_hour` (default 0; midnight). 01:30 on 5 May belongs to 4 May. All plan dates use logical days stored as `YYYY-MM-DD`. |
| Estimate bucket | One of 15, 30, 60, 90, 120 minutes. |
| Micro task | Estimate = 15. Eligible for BATCH. Derived, not stored. Every other task (estimate 30 or more) is a deep task. |
| Multiplier (M) | Global calibration factor from section 5.7. |
| Calibrated estimate | `max(5, round_to_nearest_5(estimate × M))`. |
| First step | A concrete action that starts the task and takes about 2 minutes or less, for example *open payments.rs and write the WebhookPayload struct*. |
| Breadcrumb | Where to resume: exact file, line or next action. Saved when work stops mid-task. |
| Cue | Optional if-then sentence on a planned task: *After coffee, I will open payments.rs*. |
| Rollover | A planned task that was not finished and made little progress that day. |
| Stuck | `rollover_count >= 3`. |
| Focus budget | User-set limit of planned deep-work minutes per day (default 240). |
| Session | One timed interval: FOCUS (one task), BATCH (the whole batch block) or MANUAL (time entered after the fact). |

## 3. Architecture

```
flow/
  Cargo.toml                workspace: core, src-tauri
  DECISIONS.md
  core/                     crate flow-core, NO tauri dependency
    migrations/001_init.sql
    src/lib.rs clock.rs error.rs model.rs db.rs export.rs
    src/rules/ clarify.rs plan.rs session.rs wrapup.rs review.rs calibration.rs ranking.rs
    tests/                  acceptance tests: in-memory SQLite + FixedClock
  src-tauri/                thin layer: state, commands, windows, shortcuts, tray
  src/                      React + TypeScript UI
    api.ts store.ts
    screens/ Plan.tsx Do.tsx WrapUp.tsx Settings.tsx Stats.tsx
    windows/ Capture.tsx
```

**Stack:** Tauri v2, `rusqlite` (bundled), `chrono`, `serde`, `serde_json`, `thiserror`, `uuid` (v4), `ts-rs` (TypeScript types are generated from Rust and never hand-written), `tauri-plugin-global-shortcut`, `tauri-plugin-single-instance` (plus the notification and autostart plugins in slice 7), React + TypeScript + Zustand + Tailwind. Nothing else without a DECISIONS.md entry.

**Engineering rules**

- **Clock:** all time goes through `trait Clock { now_utc(); logical_today(rollover_hour) }`. Tests use a `FixedClock` that can be advanced. No `Utc::now()` or `Local::now()` outside `SystemClock`.
- **Transactions:** every command runs in exactly one transaction, including its event-log writes. No command may half-apply.
- **Database:** set `journal_mode=WAL`, `foreign_keys=ON`, `busy_timeout=5000` on connection open (not inside migrations). Migrations run via `PRAGMA user_version` with embedded SQL. Timestamps are RFC 3339 UTC text. Dates are `YYYY-MM-DD` text. IDs are UUID v4 text.
- **Tauri state:** one `Mutex<Connection>` in managed state (`rusqlite::Connection` is not `Sync`). Commands are synchronous and never hold the lock across an await.
- **Errors:** one `AppError`, serialized as `{ code, message, overridable, hint? }`. Rust variants are CamelCase with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` so clippy stays at zero warnings.
- **Timer:** nothing ticks in Rust. A session stores timestamps, core computes elapsed time on read, and the UI renders with its own interval. This survives sleep and crashes.
- **Sync between windows:** after any command that changes data, the Tauri layer emits a `data-changed` event, and every window refetches its view when it receives it. This is how a capture made in the capture window shows up in the main window.
- **Performance targets:** cold start to a usable screen in under 1.5 s, and every command under 50 ms with 10,000 tasks in the database. Add indexes if needed; do not add caching layers.

**Platform notes**

- **Capture window:** create a hidden, always-on-top, undecorated window labelled `capture` (about 480×64, centered) at startup. The global shortcut only shows and focuses it, which is what makes opening take under 50 ms. Enter saves to INBOX and hides it; Esc hides it. Add the label to `capabilities/default.json`. The frontend picks its view from the window label.
- **Global shortcut:** default `CmdOrCtrl+Shift+J`, configurable. It works on macOS, Windows and X11 but not on Wayland. Detect `XDG_SESSION_TYPE=wayland`, show a notice in Settings, and provide a tray menu item and an in-app shortcut as the fallback.
- **Background running:** the capture shortcut only works while the app is running. Closing the main window hides it to the tray instead of quitting; Quit is in the tray menu and on Cmd/Ctrl+Q. Where no tray is available (some Linux desktops), closing quits and Settings says so.
- **Single instance:** use the single-instance plugin. Launching the app a second time focuses the running window, so two processes never use the database at once.

## 4. Data model

Schema is a contract. Column names may be adjusted for clarity, but every constraint and index below encodes an invariant and must be kept.

```sql
CREATE TABLE settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  day_rollover_hour INTEGER NOT NULL DEFAULT 0,
  daily_focus_budget_mins INTEGER NOT NULL DEFAULT 240,
  ready_cap INTEGER NOT NULL DEFAULT 10,
  focus_block_mins INTEGER NOT NULL DEFAULT 90,
  batch_block_mins INTEGER NOT NULL DEFAULT 45,
  default_multiplier REAL NOT NULL DEFAULT 1.25,
  capture_shortcut TEXT NOT NULL DEFAULT 'CmdOrCtrl+Shift+J',
  last_weekly_review_at TEXT
);
INSERT INTO settings (id) VALUES (1);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL CHECK (length(trim(title)) > 0),
  first_step TEXT,
  done_when TEXT,
  estimate_mins INTEGER CHECK (estimate_mins IN (15,30,60,90,120)),
  status TEXT NOT NULL DEFAULT 'INBOX'
    CHECK (status IN ('INBOX','READY','WAITING','SOMEDAY','DONE','DROPPED')),
  value INTEGER NOT NULL DEFAULT 3 CHECK (value BETWEEN 1 AND 5),
  pinned INTEGER NOT NULL DEFAULT 0,
  deadline_on TEXT,
  waiting_on TEXT,
  review_on TEXT,
  breadcrumb TEXT,
  cue TEXT,
  parent_id TEXT REFERENCES tasks(id),
  closed_reason TEXT CHECK (closed_reason IN ('SPLIT','NOT_DOING')),
  rollover_count INTEGER NOT NULL DEFAULT 0,
  planned_for TEXT,
  slot TEXT CHECK (slot IN ('ONE','THREE','BATCH','EXTRA')),
  slot_pos INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  CHECK ((planned_for IS NULL) = (slot IS NULL) AND (slot IS NULL) = (slot_pos IS NULL)),
  CHECK (slot IS NULL OR status IN ('READY','DONE')),
  CHECK (slot IS NULL
    OR (slot = 'ONE' AND slot_pos = 1)
    OR (slot = 'THREE' AND slot_pos BETWEEN 1 AND 3)
    OR (slot = 'BATCH' AND slot_pos BETWEEN 1 AND 5)
    OR (slot = 'EXTRA' AND slot_pos >= 1)),
  CHECK (status <> 'READY' OR (first_step IS NOT NULL AND length(trim(first_step)) >= 3
                                AND estimate_mins IS NOT NULL))
);
CREATE UNIQUE INDEX ux_slot_pos ON tasks (planned_for, slot, slot_pos) WHERE slot IS NOT NULL;
CREATE INDEX ix_tasks_status ON tasks (status);
CREATE INDEX ix_tasks_review ON tasks (review_on) WHERE review_on IS NOT NULL;

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('FOCUS','BATCH','MANUAL')),
  task_id TEXT REFERENCES tasks(id),
  day TEXT NOT NULL,
  planned_mins INTEGER NOT NULL,
  extended INTEGER NOT NULL DEFAULT 0,
  accumulated_secs INTEGER NOT NULL DEFAULT 0,
  running_since TEXT,
  last_seen_at TEXT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  CHECK (kind = 'BATCH' OR task_id IS NOT NULL)
);
CREATE UNIQUE INDEX ux_one_active_session ON sessions ((ended_at IS NULL)) WHERE ended_at IS NULL;

CREATE TABLE days (
  day TEXT PRIMARY KEY,
  plan_locked_at TEXT,
  multiplier_at_lock REAL,
  budget_at_lock INTEGER,
  wrapup_done_at TEXT,
  win_note TEXT
);

CREATE TABLE events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at TEXT NOT NULL,
  task_id TEXT REFERENCES tasks(id),
  kind TEXT NOT NULL,
  reason TEXT,
  before_json TEXT
);
```

`events.kind` values: CREATED, CLARIFIED, PLANNED, UNPLANNED, SWAPPED, STARTED, STOPPED, DONE, ROLLED_OVER, SPLIT, WAITING, SOMEDAY, DROPPED, REACTIVATED, OVERRIDE, LOCKED, WRAPUP. `before_json` holds the changed task fields before the change, which is what powers undo.

**Derived, never stored:** mode, calibrated estimate, planned minutes, stuck flag, ordering key, micro flag, and logged minutes per task (sum of FOCUS and MANUAL session time; elapsed = `accumulated_secs` plus `now - running_since` while running).

A planned task that finishes keeps its `planned_for` and `slot` as history. An unfinished planned task that is carried over has `planned_for`, `slot` and `slot_pos` cleared.

### Task lifecycle

Status changes happen only through the commands in this table. Any other change fails with INVALID_TRANSITION.

| From | To | Command and conditions |
|---|---|---|
| INBOX | READY | `clarify_task` (rule C2) |
| INBOX, READY, SOMEDAY | WAITING | `triage_task`; needs `waiting_on` and `review_on` |
| INBOX, READY, WAITING | SOMEDAY | `triage_task`; needs `review_on` |
| INBOX, READY | DROPPED (`NOT_DOING`) | `triage_task` |
| READY | DROPPED (`SPLIT`) | `split_task` (rule C5) |
| WAITING, SOMEDAY | READY | `reactivate_task`; the Ready cap applies; a task lacking a first step or estimate must come with them (rule C6) |
| READY | DONE | `complete_task` |
| DROPPED (`NOT_DOING`) | INBOX | `restore_task` |
| DONE, DROPPED | previous status | `undo_last` only (rule S3) |

- A planned task (slot set) can only be READY or DONE. Moving a planned task to WAITING, SOMEDAY or DROPPED clears its plan fields in the same transaction. This is not gated by PLAN_LOCKED: the waiting or drop fields are the record.
- `days` rows are created lazily. Any command that touches a day (assign, lock, session start, wrap-up) creates the row if it does not exist, so nothing needs setting up in advance.
- There is no hard delete in v1.

## 5. Rules

Every rule is testable. IDs are referenced by the acceptance tests in section 8.

### 5.1 Capture and clarify

- **C1.** Capture creates an INBOX task from a title only. It is available globally and never changes mode or navigation.
- **C2.** Clarify (INBOX to READY) requires a non-empty title, a first step of at least 3 characters, and an estimate bucket. Defaults: value 3, no deadline, no done-when. This is a hard rule.
- **C3.** Triage without clarifying: INBOX can go straight to WAITING, SOMEDAY or DROPPED with no first step or estimate.
- **C4.** Boulder nudge (soft gate BOULDER): if `round(estimate × M) > 120` the UI offers Split before accepting. Choosing *keep as is* needs a reason and is logged. The UI also offers *bigger than 2 hours*, which opens Split directly.
- **C5.** Split replaces a task with 2 to 5 children. Each child must have a title, first step and estimate bucket (no placeholders). Children inherit value and deadline, get `parent_id`, enter READY, and are exempt from the Ready cap. The parent becomes DROPPED with `closed_reason = SPLIT` and keeps its logged time.
- **C6.** Reactivating a WAITING or SOMEDAY task needs the same fields as C2. If the task lacks a first step or an estimate, the call must supply them.

### 5.2 Ready list and ordering

- **R1.** Ready cap is `settings.ready_cap` (default 10, editable) and counts deep tasks only; micro tasks are not capped. Moving a deep task into READY by user action (clarify, reactivate) when READY is at the cap trips the soft gate READY_FULL. The UI resolves it by sending either the new task or the lowest-ordered unplanned deep Ready task to SOMEDAY with `review_on = today + 14 days`. Split children and rollover returns are exempt; the list may exceed the cap and the UI shows it in amber.
- **R2.** Deep tasks are ordered on read by this key, in this precedence: pinned first; then urgency tier (`deadline_on` overdue or within 2 logical days, then within 14 days, then everything else); then value, highest first; then estimate, shortest first; then `created_at`, oldest first. Nothing is stored, so urgency never goes stale. The list answers *what matters most* first and breaks ties toward quick wins.
- **R3.** Quick (micro) tasks are ordered separately: pinned first, then nearest deadline, then `created_at`, oldest first.
- **R4.** Only unplanned READY tasks are candidates in the Plan screen. The Deep list uses R2 and the Quick list uses R3.

### 5.3 Planning

- **P1.** Plan target day: today if today’s plan is unlocked and today’s wrap-up is not done; otherwise tomorrow. The evening wizard always targets tomorrow. Planning any other day fails with INVALID_DAY.
- **P2.** Slot capacity is hard: ONE at most 1, THREE at most 3, BATCH at most 5, EXTRA unlimited. Enforced by the schema and by code. BATCH accepts only micro tasks. A task takes the lowest free `slot_pos` in its slot (a DONE task keeps its position).
- **P3.** Assign requires status READY, not already planned, and not stuck unless overridden (soft gate STUCK_TASK).
- **P4.** Budget: `planned_mins` = sum of calibrated estimates of ONE and THREE tasks, plus `batch_block_mins` once if any BATCH task exists. Assigning past `daily_focus_budget_mins` trips the soft gate BUDGET_EXCEEDED. EXTRA is not budget-checked. For a locked day, calibrated estimates use `multiplier_at_lock`; otherwise the current M.
- **P5.** Unassign returns the task to READY, unplanned, any time before lock.
- **P6.** Cue is optional text on a planned task, shown in Do mode when present. It is never required.
- **P7.** Lock requires at least one planned task. No ONE is a warning, not an error, so core-only days work. Lock stores `plan_locked_at`, `multiplier_at_lock` and `budget_at_lock`. Locking tomorrow’s plan does not enter Do mode; Do starts when that logical day arrives.
- **P8.** After lock, assign, unassign and swap need a reason (soft gate PLAN_LOCKED), which is logged. `swap(out, in, reason)` replaces a planned task in its slot atomically; the outgoing task returns to READY, unplanned. Moving a planned task to WAITING, SOMEDAY or DROPPED is not gated (see Task lifecycle).
- **P9.** Suggest plan: `suggest_plan(day)` returns a proposal and changes nothing. ONE = the first deep task in R2 order that is not stuck and has a calibrated estimate of 120 minutes or less. THREE = up to 3 further deep tasks in R2 order that fit the remaining budget (skip a task that does not fit and keep scanning). BATCH = up to 5 micro tasks in R3 order, only if at least `batch_block_mins` of budget remains. `apply_plan(day, assignments)` then fills empty slots in one transaction under the normal assign rules and never displaces an existing assignment. The UI offers it as one tap.

### 5.4 Do mode

- **D1.** Default landing mode is Do if today’s plan is locked and today’s wrap-up is not done; otherwise Plan. The user can always navigate to Plan explicitly (UI state only, not stored); a restart lands on the derived mode. Do has two states: *Working* (at least one open planned task) and *All done*.
- **D2.** The Do view returns only: the current item (title, first step, done-when, cue, breadcrumb, slot and value), counts of remaining and done, the active session, and on request today’s other planned items (for *do something else next*). It must never return a task that is not planned for today.
- **D3.** Default order: ONE, then THREE by `slot_pos`, then the BATCH block, then EXTRA. The current item is the active session’s task if a session is running, otherwise the item the user picked through the do-something-else-next action (UI state only, not stored), otherwise the first open item in this order.
- **D4.** Starting a FOCUS session needs a task planned for today and not DONE. Only one active session may exist at a time (DB-enforced). A task in the BATCH slot can only be worked through a BATCH session; every other task only through a FOCUS session. `planned_mins = focus_block_mins`.
- **D5.** A BATCH session covers the whole batch (`planned_mins = batch_block_mins`). The UI shows one micro task at a time. Completing a micro task marks it DONE and continues the same session. No time is attributed to individual micro tasks.
- **D6.** Pause and resume accumulate elapsed time correctly across pauses.
- **D7.** At `planned_mins`, a FOCUS session prompts Done, +15 minutes (once per session, tracked by `extended`), or Stop. A BATCH session has no extension; at expiry it prompts Stop, and remaining micro tasks stay planned.
- **D8.** *Stop for now* ends the session, logs the time and asks for a breadcrumb (skippable). The task stays planned and READY.
- **D9.** Complete marks the task DONE with `completed_at`, ends its session, and advances automatically to the next item. A micro task needs no session to be completed. If a session is active for a different task, complete fails with INVALID_TRANSITION (stop that session first). If a deep task never had a timer, the UI asks for the actual time in one tap (skippable); the answer creates a MANUAL session.
- **D10.** All done state: show the win-note field, *Pull one more from Ready* (one task at a time, EXTRA slot, only when every planned task is DONE), and *Wrap up the day*.
- **D11.** Break suggestion (UI only, nothing stored): after a FOCUS session of 60 or more minutes, suggest a 15-minute break with a Skip button.
- **D12.** Crash recovery: while a session runs, the UI calls `heartbeat` every 60 seconds, which updates `last_seen_at`. On startup, an unended running session whose `last_seen_at` is more than 2 minutes old is frozen at `last_seen_at`, and the UI offers Resume or Stop.
- **D13.** Blocked: from Do the user can move the current task to WAITING (it needs `waiting_on` and `review_on`). It leaves today’s plan without counting as a rollover, and the next item becomes current.

### 5.5 Wrap-up (evening wizard)

- **W1.** Available any time from Plan or Do (*Wrap up day*) and suggested by the All done state. Steps: (1) settle unfinished tasks, (2) clear the Inbox (clarify, triage or drop each item; skippable), (3) plan tomorrow with Suggest plan (the Plan screen targeting tomorrow), (4) optional win note, (5) optional lock.
- **W2.** Every unfinished planned task for today needs an outcome: CARRY (back to READY, unplanned), WAITING (needs `waiting_on` and a review date), SOMEDAY (review date), DROP, or SPLIT. *Carry all* is one tap.
- **W3.** If a task has time logged today, the UI asks for a breadcrumb. It can be skipped in one tap. Tasks with no logged time never ask.
- **W4.** Rollover: on CARRY, `rollover_count += 1` only if progress today is under 25% of the task’s estimate (`logged_today_mins < 0.25 × estimate_mins`). Batch tasks have no per-task time, so they always count. WAITING, SOMEDAY, DROP and SPLIT never increment.
- **W5.** Stuck: when `rollover_count` reaches 3, the wizard asks for a resolution immediately: SHRINK (new first step plus an estimate bucket no larger than before; counter resets to 0), SPLIT, WAITING, SOMEDAY or DROP. Skipping leaves the task stuck: still READY, but P3 blocks planning it without an override.
- **W6.** Completing wrap-up sets `days.wrapup_done_at`, stores the win note, and is idempotent. Mode afterwards is Plan, targeting tomorrow.
- **W7.** Reopen: `reopen_day` clears `wrapup_done_at` for the current logical day, which puts the app back in Do mode. Tasks already carried over stay in READY and can be planned again or pulled in as EXTRA. It is refused for any other day (INVALID_DAY).

### 5.6 Reviews and resurfacing

- **V1.** WAITING and SOMEDAY tasks with `review_on <= today` appear in a *Due for review* banner in Plan. The user chooses reactivate (Ready cap applies), push the date, or drop. Nothing moves automatically.
- **V2.** If the last weekly review was more than 7 days ago, Plan shows a *Weekly review* banner. The review is a checklist: clear the inbox, trim Ready to the cap, handle due Waiting and Someday items, resolve stuck tasks. Completing it sets `last_weekly_review_at`.

### 5.7 Calibration

- **K1.** One global multiplier M. Samples are the most recent 20 tasks that are DONE or DROPPED with `closed_reason = SPLIT`, have an estimate, and have logged FOCUS or MANUAL minutes above 0. Ratio = `logged / estimate`, clamped to [0.5, 3.0]. Micro (BATCH) tasks are excluded.
- **K2.** With fewer than 5 samples, M = `settings.default_multiplier` (1.25). Otherwise M = the median ratio, clamped to [0.8, 2.5] and rounded to 2 decimals.
- **K3.** Calibrated estimate = `max(5, round_to_nearest_5(estimate × M))`.
- **K4.** Split tasks are included on purpose. Their logged time understates their true duration, which counters the bias of learning only from tasks that finished.
- **K5.** Stats show M, the sample count, and how many DONE tasks had no time data.

### 5.8 Day boundary and reconcile

- **B1.** Logical day is computed only through the Clock.
- **B2.** `reconcile()` runs on startup and whenever the logical day changes. For planned, not-DONE tasks whose `planned_for` is before today and whose day has no `wrapup_done_at`, it applies CARRY with the W4 rule and no breadcrumb. It also freezes any unended session from a previous day at its `last_seen_at`. It is idempotent.

### 5.9 Soft gates and overrides

| Code | Trips on | Resolution |
|---|---|---|
| BOULDER | C4 | Split, or keep with a reason |
| READY_FULL | R1 | Send to Someday (no reason needed) |
| BUDGET_EXCEEDED | P4 | Reason |
| STUCK_TASK | P3 | Reason |
| PLAN_LOCKED | P8 | Reason |

Mechanism: a command that can trip a gate takes `override_reason: Option<String>`. If the gate trips and there is no reason, return `AppError { code, overridable: true }`. If a reason is present (3 or more characters), proceed and write an OVERRIDE event with the rule code and reason.

Hard errors (`overridable = false`): INVALID_TRANSITION, SLOT_FULL, BATCH_REQUIRES_MICRO, MISSING_REQUIRED_FIELD, SESSION_ALREADY_ACTIVE, EXTENSION_USED, NOT_PLANNED_FOR_TODAY, MISSING_OUTCOME, INVALID_DAY, NOTHING_TO_UNDO, NOT_FOUND.

### 5.10 Data safety

- **S1.** Backup and integrity: on the first launch of each logical day, copy the database with `VACUUM INTO` to `backups/flow-YYYY-MM-DD.db` in the app data folder and keep the 14 newest. On startup run `PRAGMA quick_check`; if it fails, do not start, and show a blocking message that names the newest backup. `backup_db(path)` writes the same kind of copy to a path the user picks. Restoring means closing the app and replacing `flow.db` with a backup (explain this in the README).
- **S2.** Export: `export_json(path)` writes one JSON file shaped `{ version, exported_at, settings, tasks, sessions, days, events }`. There is no import in v1.
- **S3.** Undo: `undo_last` reverses the most recent undoable event (complete, triage or drop, assign, unassign) using `before_json`, only if the event is under 10 minutes old and no newer event touched the same task. Split, swap, lock and wrap-up are not undoable. Otherwise it fails with NOTHING_TO_UNDO.
- **S4.** No hard delete: DROPPED is the final state. A task dropped as NOT_DOING can be restored to INBOX with `restore_task` from a Dropped drawer in the Plan screen.

## 6. Screens

Keep the UI plain. Three layouts plus the capture bar. Correctness first, polish in the last slice.

**Do screen** (full-screen dark, no sidebar, no lists, no inbox count):

```
  THE ONE  ·  ⚡ Value 5/5                         2 of 5 done

  Write webhook handler
    Resume here: payments.rs line 84, add retry      (breadcrumb, if any)
  ▶ FIRST STEP: open payments.rs, write the
    WebhookPayload struct                             (amber callout)
    After coffee, I will start.                       (cue, if any)

                     62:15 remaining
  [Space] start/pause   [D] done   [E] +15   [S] stop for now   [N] do something else next
```

**Plan screen:** left column holds the Inbox (with a clarify form: title, first step, estimate buttons 15/30/60/90/120, optional value and deadline) above the ordered Deep list (R2) and the Quick list (R3), with a `7 / 10` cap badge. Right column holds the day plan with ONE, THREE and BATCH slots, a budget bar (`planned / budget`, turning red when over), an optional cue field on the ONE slot, a Suggest plan button (P9), and a Lock button. A collapsed drawer lists Waiting, Someday and Dropped tasks (Dropped has a Restore button). Banners appear for *Due for review*, *Stuck tasks* and *Weekly review*.

**Wrap-up wizard:** five steps as in W1, one screen each, with *Carry all* and *Skip* buttons wherever a field is optional.

**Keyboard:** global `CmdOrCtrl+Shift+J` opens capture. Do: Space, D, E, S, N, B (blocked: move the task to Waiting). Plan: `1`/`2`/`3` assign the focused Ready task to ONE/THREE/BATCH, `CmdOrCtrl+Enter` locks. Wrap-up: `CmdOrCtrl+Enter` goes to the next step. Esc closes dialogs. Every action must be reachable by keyboard.

**Settings and Stats:** Settings exposes every field of the `settings` table, shows the Wayland notice where relevant, and has Export and Backup buttons. Stats shows M with its sample count, focus minutes for the last 14 days, the tasks with the most rollovers, and recent win notes.

**Testability:** give every interactive element and each key piece of state (current task title, timer, budget bar, each slot) a stable `data-testid`, so end-to-end tests can be added later.

## 7. Command API (IPC)

All commands return `Result<T, AppError>`. Request and response types are generated from Rust with `ts-rs`.

**Reads:** `get_mode`, `get_plan_view(day?)`, `get_do_view`, `get_wrapup_view`, `get_stats`, `get_settings`.

**Tasks:** `capture(title)`, `clarify_task(id, input, if_full?, override_reason?)`, `update_task(id, patch)`, `split_task(id, children)`, `triage_task(id, to)`, `reactivate_task(id, input?, if_full?)`, `restore_task(id)`, `pin_task(id, pinned)`.

**Planning:** `suggest_plan(day)`, `apply_plan(day, assignments)`, `assign_slot(id, day, slot, override_reason?)`, `unassign(id, override_reason?)`, `set_cue(id, text)`, `swap_task(day, out_id, in_id, reason)`, `lock_plan(day, override_reason?)`, `pull_extra(id)`.

**Sessions:** `start_session(kind, task_id?)`, `pause_session`, `resume_session`, `extend_session`, `heartbeat`, `stop_session(breadcrumb?)`, `complete_task(id, manual_mins?)`.

**Wrap-up and review:** `resolve_unfinished(id, outcome, breadcrumb?)`, `complete_wrapup(win_note?)`, `reopen_day`, `complete_weekly_review`.

**Data:** `update_settings(patch)`, `export_json(path)`, `backup_db(path)`, `undo_last`.

`get_do_view` must be implemented as its own query that selects only tasks planned for today. It must not be a filtered copy of the full task list.

### Key types

These are the minimum fields; add more if the UI needs them. Every type is generated with `ts-rs`.

| Type | Fields |
|---|---|
| `ClarifyInput` | `title`, `first_step`, `estimate_mins` (15, 30, 60, 90 or 120); optional `done_when`, `value` (1 to 5), `deadline_on` |
| `TaskPatch` | any of `title`, `first_step`, `done_when`, `estimate_mins`, `value`, `deadline_on`, `cue`, `breadcrumb` |
| `SplitChild` | `title`, `first_step`, `estimate_mins` |
| `TriageTo` | `WAITING { waiting_on, review_on }`, `SOMEDAY { review_on }`, `DROPPED` |
| `Outcome` | `CARRY`, `WAITING { waiting_on, review_on }`, `SOMEDAY { review_on }`, `DROP`, `SPLIT { children }`, `SHRINK { first_step, estimate_mins }` |
| `Slot` | `ONE`, `THREE`, `BATCH` (EXTRA only through `pull_extra`) |
| `PlanView` | `target_day`, `locked`, `deep_candidates[]`, `quick_candidates[]`, `inbox[]`, `plan { one, three[], batch[], extra[] }`, `budget { planned_mins, budget_mins, multiplier }`, `ready_count`, `ready_cap`, `banners { due_for_review[], stuck[], weekly_review_due }`, `dropped[]` |
| `DoView` | `state` (WORKING or ALL_DONE), `current` (task, slot, value, first_step, done_when, cue, breadcrumb), `remaining_count`, `done_count`, `session` (id, kind, planned_mins, elapsed_secs, running, extended), `others[]` only when requested |
| `WrapupView` | `unfinished[]` (task, `logged_today_mins`, `needs_breadcrumb`), `stuck[]`, `inbox[]`, `win_note`, `tomorrow` (a `PlanView`) |
| `Stats` | `multiplier`, `sample_count`, `no_time_data_count`, `focus_mins_last_14[]`, `most_rolled_over[]`, `recent_win_notes[]` |

## 8. Acceptance tests

All tests live in `core/tests/`, run against an in-memory SQLite database with a `FixedClock`, and call core functions directly (no Tauri). Default fixture: rollover hour 4, budget 240, M = 1.0 unless stated.

**Capture and clarify**

- **T01** (C1) Capturing a title creates an INBOX task with no other fields. An empty or whitespace title is rejected.
- **T02** (C2) Clarify without a first step, with a 2-character first step, or without an estimate fails with MISSING_REQUIRED_FIELD. With all three, the task becomes READY with value 3.
- **T03** (C2) An estimate outside 15/30/60/90/120 is rejected.
- **T04** (C4) With M = 1.25, clarifying an estimate of 120 (calibrated 150) without a reason returns BOULDER (overridable). With a reason it becomes READY and exactly one OVERRIDE event exists. An estimate of 90 (calibrated 115) trips nothing.
- **T05** (C5) Split into 1 child is rejected. Split with a child missing its first step is rejected. A valid split into 2 children drops the parent with `closed_reason = SPLIT`, creates two READY children with `parent_id` and inherited value and deadline, and succeeds even when READY is already at the cap.
- **T06** (R1) With 10 deep READY tasks and cap 10 (micro tasks do not count), clarifying an 11th returns READY_FULL. Retrying with `if_full = SOMEDAY` leaves it in SOMEDAY with `review_on = today + 14 days`.
- **T07** (R2) Order is pinned first, then urgency tier (overdue or due within 2 days, then within 14 days, then the rest), then value descending, then estimate ascending, then oldest first. A value-5 120-minute task sorts above a value-3 30-minute task. Advancing the FixedClock so that a 10-day deadline enters the 2-day window changes the order, which proves urgency is computed on read.

**Planning**

- **T08** (P2) A second ONE, a fourth THREE and a sixth BATCH each fail with SLOT_FULL. Assigning a 30-minute task to BATCH fails with BATCH_REQUIRES_MICRO.
- **T09** (P3) Assigning a DONE, WAITING or INBOX task, or an already planned task, fails with INVALID_TRANSITION.
- **T10** (P4) Budget 240, M = 1.0: ONE 120 and THREE 90 succeed. A THREE 60 returns BUDGET_EXCEEDED (270 > 240). With a reason it succeeds and logs an OVERRIDE. Unassigning it brings planned minutes back to 210.
- **T11** (P4) Five BATCH tasks add 45 minutes to planned minutes once, not five times.
- **T12** (P3) A task with `rollover_count = 3` fails to assign with STUCK_TASK; with a reason it succeeds.
- **T13** (P7) Locking with zero planned tasks fails. Locking with only THREE tasks succeeds and returns a *no ONE* warning. The `days` row stores `multiplier_at_lock` and `budget_at_lock`.
- **T14** (P8) After lock, assign and unassign without a reason return PLAN_LOCKED. `swap_task` with a reason puts the new task in the same slot, returns the old one to READY unplanned, and logs SWAPPED.
- **T15** (P1) Plans for today and tomorrow are independent: each day can hold its own ONE.
- **T16** (P7, D1) Today’s wrap-up is done and tomorrow’s plan is locked at 18:00: mode is Plan. After the clock passes 04:00 the next morning: mode is Do.

**Do mode**

- **T17** (D2) With 20 READY tasks, 5 INBOX tasks and a plan of 3, `get_do_view` returns only ids from the plan, including when asked for the other remaining items.
- **T18** (D1) Mode table: unlocked plan gives Plan. Locked with wrap-up not done gives Do (Working while any planned task is open, All done when none is). Wrap-up done gives Plan.
- **T19** (D4) Starting a second session while one is active fails with SESSION_ALREADY_ACTIVE, and the schema rejects a direct insert as well.
- **T20** (D6) Start, run 20 min, pause 10 min, resume, run 5 min: elapsed is 25 min.
- **T21** (D7) Extend succeeds once on a FOCUS session; the second call fails with EXTENSION_USED. Extending a BATCH session fails with INVALID_TRANSITION.
- **T22** (D3, D9) Completing a task with an active session ends the session, sets DONE and `completed_at`, adds the logged minutes, and the next item follows the order ONE, THREE, BATCH, EXTRA.
- **T23** (D9) Completing a 60-minute task with no session and `manual_mins = 45` creates a MANUAL session of 45 minutes.
- **T24** (D8) Stop with a breadcrumb stores it on the task; the task stays planned and READY.
- **T25** (D12) A running session whose `last_seen_at` is 5 minutes before now is frozen at `last_seen_at` on startup, and the 5-minute gap is not counted.
- **T26** (D10) `pull_extra` fails while any planned task today is open. When all are DONE it assigns EXTRA with the next `slot_pos`.

**Wrap-up**

- **T27** (W3, W4) Unfinished task with 40 of 60 minutes logged today, outcome CARRY: rollover unchanged, task READY and unplanned. Skipping the breadcrumb succeeds.
- **T28** (W4, W5) Unfinished task with 5 of 60 minutes logged: rollover +1. After three such days the wrap-up response flags it as needing resolution.
- **T29** (W5) SHRINK requires a new first step and an estimate no larger than before, resets the counter to 0 and keeps the task READY. Skipping leaves it stuck.
- **T30** (W2) WAITING without `waiting_on` or a review date fails; with both it succeeds and rollover is unchanged.
- **T31** (W2, W6) `complete_wrapup` with an unfinished task that lacks an outcome fails with MISSING_OUTCOME. After *carry all* it succeeds, and a second call changes nothing.
- **T32** (B2) The app is closed for two days with planned tasks outstanding. On startup `reconcile` carries each exactly once; running it again changes nothing.
- **T33** (B1) With rollover hour 4, 03:59 local belongs to the previous day and 04:00 to the new one. Use fixed offsets in tests, plus one case across a DST change.
- **T34** (V1) A WAITING task with `review_on = today` appears in the due list, and no state changes until the user acts.

**Calibration**

- **T35** (K1, K2) Under 5 samples, M = 1.25. Five samples of ratio 1.5 give M = 1.5. Ratios of 10.0 and 0.1 are clamped to 3.0 and 0.5 before the median is taken.
- **T36** (K1, K4) A SPLIT-closed task with logged time is included. A micro task and a task with no time data are excluded, and the no-data count is reported.
- **T37** (K3) 60 min × 1.38 = 82.8, which rounds to 85. 15 min × 0.8 = 12, which rounds to 10. The result is never below 5.

**Data safety**

- **T38** Fault injection: force a failure midway through `lock_plan`, `complete_wrapup`, `split_task` and `swap_task`, and assert the database is unchanged.
- **T39** `export_json` contains every table. `backup_db` produces a database that opens with identical row counts.
- **T40** A fresh database reaches the latest `user_version`; opening it again runs no migration.
- **T41** `undo_last` reverses the most recent complete, drop, assign, unassign or move-to-Someday using `before_json`, and refuses with NOTHING_TO_UNDO if the event is older than 10 minutes, a newer event touched the same task, or the event was a split, swap, lock or wrap-up.
- **T42** Property test: after any random sequence of valid commands, no day has more than 1 ONE, 3 THREE or 5 BATCH; at most one session is active; every planned task is READY or DONE; and no task is both planned and WAITING or SOMEDAY.

**Added in v2.1: planning, sessions, lifecycle and performance**

- **T43** (P9) With 6 deep READY tasks of varied value and estimate, 4 micro tasks, budget 240 and M = 1.0: ONE is the first deep task in R2 order that is not stuck and is 120 minutes or less; THREE are up to three further deep tasks that fit; BATCH holds micro tasks only when at least 45 minutes of budget remain; planned minutes never exceed the budget; `suggest_plan` changes nothing in the database. `apply_plan` then assigns exactly the proposal in one transaction and never displaces an existing assignment.
- **T44** (P1, section 4) A plan for the day after tomorrow fails with INVALID_DAY. On a fresh database the first assign for tomorrow succeeds, because the `days` row is created lazily.
- **T45** (D4, D9) Starting a FOCUS session on a BATCH-slot task fails with INVALID_TRANSITION, and so does starting a BATCH session when no BATCH task is planned. Completing a task while a session is active for a different task fails with INVALID_TRANSITION. Completing a micro task with no session succeeds.
- **T46** (W7) `reopen_day` after wrap-up clears `wrapup_done_at` for today and mode returns to Do. For any other day it fails with INVALID_DAY.
- **T47** (Task lifecycle) Table-driven: every status change not in the lifecycle table fails with INVALID_TRANSITION, and every listed change succeeds from a valid fixture. Moving a planned task to WAITING clears its slot fields, with no PLAN_LOCKED gate even on a locked day.
- **T48** (C6, S4) `reactivate_task` on a SOMEDAY task with no first step or estimate fails with MISSING_REQUIRED_FIELD unless the fields are supplied. `restore_task` moves a NOT_DOING task to INBOX and refuses a SPLIT-closed task.
- **T49** (D13) Blocked from Do: the current task goes to WAITING and leaves the plan, its `rollover_count` is unchanged, and the next item in order becomes current.
- **T50** (section 3) With 10,000 tasks and 100,000 events, `get_plan_view`, `get_do_view` and `assign_slot` each finish in under 50 ms (release build; use a looser ceiling in debug).
- **T51** (S1) A corrupted database file makes startup refuse to run and name the newest backup. The daily backup keeps only the 14 newest files.

## 9. Build slices

Each slice ends with a working app. Run the CI gates and a manual smoke test before starting the next slice.

**Slice 0: Scaffold.** Workspace, `flow-core` with Clock, AppError, the migration runner and the schema from section 4, `ts-rs` export, a Tauri window that calls `get_mode` and shows the result, and one script that runs all CI gates. *Tests:* T40. *Done when:* the gates pass and the window shows Plan.

**Slice 1: Walking skeleton, usable end to end.** In-app capture, clarify (C1 to C3), ordering (R2, R3), budgets using the default M, planning (P1 to P5, P7, P9), Do mode (D1 to D6, D9 with auto-advance but without the manual-time prompt, D10), logical day (B1). Plain, ugly UI is fine. *Tests:* T01 to T03, T07 to T11, T13, T15 to T20, T22, T26, T33, T43 to T45. *Done when:* you can capture three tasks, clarify them, plan ONE, THREE and BATCH, lock, work through them in Do mode, and land on the All done screen.

**Slice 2: Capture everywhere and a robust timer.** Hidden capture window, global shortcut, tray item, Wayland fallback, the full keyboard matrix, extension and stop-for-now (D7, D8), heartbeat and crash recovery (D12), the single-instance lock, and close-to-tray with a Quit item. *Tests:* T21, T24, T25. *Done when:* the capture bar opens over another app instantly (target under 50 ms) and the item lands in the Inbox, and killing the app mid-session then relaunching offers Resume or Stop with the right elapsed time.

**Slice 3: Wrap-up, rollover and mid-day swap.** The wizard (W1 to W7), reconcile (B2), the stuck gate (P3), post-lock rules and swap (P8), blocked-from-Do (D13), and breadcrumbs shown in Do. *Tests:* T12, T14, T27 to T32, T46, T49. *Done when:* a full day cycle works: plan, work part of it, wrap up, and the next morning Do shows the breadcrumb.

**Slice 4: Ready hygiene.** WAITING, SOMEDAY and DROPPED triage, review dates and the Due banner (V1), Ready cap and overflow (R1), split (C5), boulder nudge (C4), pinning, weekly review (V2), restore from the Dropped drawer, and the task lifecycle matrix. *Tests:* T04 to T06, T34, T47, T48.

**Slice 5: Calibration and stats.** M computed per K1 to K5 and used everywhere calibrated estimates appear, MANUAL sessions and the time prompt on Complete (D9), and a Stats screen (M, sample count, focus minutes for the last 14 days, most-rolled-over tasks). *Tests:* T23, T35 to T37.

**Slice 6: Safety net and settings.** A Settings screen covering every field in `settings`, `export_json`, `backup_db`, an automatic daily backup in the app data folder (keep the last 14), `undo_last`, the integrity check and the rest of data safety (S1 to S4), plus the fault-injection, property and performance tests. *Tests:* T38 to T42, T50, T51.

**Slice 7: Polish and packaging.** Hold-to-leave friction when navigating from Do to Plan (hold Shift+Esc for 1.5 s with a progress bar), break suggestion (D11), end-of-block sound and OS notification (notification plugin), optional launch at login (autostart plugin), reduced-motion support, app icon, `tauri build` installers for macOS, Windows and Linux, and a README covering install, data location and shortcuts.

## 10. Defaults and scope

**Defaults** (editable in Settings unless marked fixed)

| Setting | Default |
|---|---|
| Day rollover hour | 0 |
| Daily focus budget | 240 min |
| Ready cap | 10 |
| Focus block | 90 min |
| Batch block | 45 min |
| Default multiplier (cold start) | 1.25 |
| Someday review delay | 14 days |
| Stuck threshold (fixed) | 3 rollovers |
| Rollover progress threshold (fixed) | 25% of estimate |
| Weekly review interval (fixed) | 7 days |

**Out of scope for v1:** sync or cloud, accounts, mobile, recurring tasks, calendar or email integration, projects and tags, reminders and any notification other than the end-of-block alert, AI features, per-category calibration, EWMA capacity modeling, wake-time and circadian scheduling, stored WSJF scores, and multiple users.

**A note on the science:** the 90-minute focus block, the peak and trough shape of the day, and the 45-minute batch are sensible defaults, not facts the app enforces. They are settings, and no rule above blocks the user because of them. The mechanisms with the strongest research support (a concrete first step, an if-then cue, writing down where to resume) are the ones this spec makes prominent.
