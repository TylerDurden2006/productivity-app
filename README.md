# Flow

Flow is a local-first planner with six screens. Daily holds due habits and the day's tasks in Main, Next, Small, and More bands. Plan holds tasks for the week, month, quarter, year, and long term. Do runs one focus session at a time. Try keeps a shelf of things worth trying. Dashboard shows what was planned, done, left, and carried. Settings holds the numbers and data controls. Tasks, Plan items, habits, Try items, sessions, and settings live in a SQLite database on your machine. There is no account, network call, or telemetry.

## Requirements

- Rust 1.85 or newer. On Windows, use the MSVC toolchain.
- Node.js 20.19 or newer on the 20.x line, or Node.js 22.12 or newer.
- Tauri system prerequisites for your operating system. On Windows that means WebView2 and Visual Studio Build Tools with the C++ workload. On macOS install Xcode Command Line Tools. On Linux install the WebKitGTK and build packages listed in the Tauri v2 guide.

## Run

```powershell
npm install
npm run tauri dev
```

Vite runs on port 1420. Tauri opens the desktop window when the Rust build finishes.

## Build

```powershell
npm run tauri build
```

Installers are produced on the host operating system, under `target/release/bundle`. A Windows build makes Windows installers; build on macOS or Linux to make those packages. The current version is `1.0.3`.

## Install on Windows

Build the NSIS installer, then run it:

```powershell
npm run tauri build
target\release\bundle\nsis\Flow_1.0.3_x64-setup.exe
```

It installs for the current Windows user, without an administrator prompt, to `%LOCALAPPDATA%\Flow`. It creates a top-level Start Menu shortcut at `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Flow.lnk`, an uninstaller, and an Apps and Features entry. `Get-StartApps` reports Flow with AppID `com.flow.focus`. Windows Search indexes the Start Menu shortcut, so pressing `Win` and typing `Flow` opens it.

There is no desktop shortcut by default. Flow keeps its database in `%APPDATA%\com.flow.focus`; uninstalling does not remove that data unless the uninstaller checkbox is selected. The installer is unsigned, so Windows may show an unknown publisher or SmartScreen warning.

## Checks

```powershell
npm run ci
```

That runs the four project gates in order:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run build
```

The suite includes the original acceptance tests plus the v4 coverage for automatic carry, Daily, habits, horizons, reorder, Dashboard, and Try. The latest `npm run ci` run passed with 188 tests, clippy, formatting, and the production build.

## Interface

Flow has six screens in the left rail.

| Screen | What it does |
| --- | --- |
| Daily | Shows due habits first, then the Main, Next, Small, and More task bands. Tasks and habits can be checked off. Drag reorders tasks within and across the Main, Next, and Small lists, and onto a More row. The task action dialog offers pin, split, shrink, wait, and drop. Add task creates a task or pulls one from Ready, and targets today. The page is single-column. |
| Plan | Holds Week, Month, Quarter, Year, and Long term tabs. Each period uses the same Main, Next, Small, and More bands. Plan items are created as tasks and checked off in place. Previous and next controls move the Week, Month, Quarter, and Year periods. Items can be dragged between bands and linked to daily tasks. Completed this period, a week summary, and an Inbox backlog sit below the bands. The page is single-column. |
| Do | Shows one planned task with its first step, done-when, cue, breadcrumb, and timer. Each task gets its own focus session. When the plan is empty, Do points back to Daily. The old Ready pull remains behind the legacy DO mode. |
| Try | Keeps ideas for things to try. An item can be scheduled to a date and day part, unscheduled, skipped, or marked tried with a 1 to 5 rating and a reflection. Filters are All, Idea, Scheduled, Tried, and Skipped. |
| Dashboard | Shows Day, Week, Month, Quarter, Year, and All ranges. Cards cover planned, done, left, carried and overdue, daily and horizon totals, completion, section completion, habit consistency, focus time, most carried, and Try counts. |
| Settings | Edits planning numbers and week start, automatic carry, dashboard default range, block-end notifications, the nightly planning reminder and time, reduced motion, Advanced Ready cap and default multiplier, Flow Home, export, and backup. |

The daily bands are Main, Next, Small, and More. Capacity is 1, 3, 5, and unlimited. The internal slot names remain `ONE`, `THREE`, `BATCH`, and `EXTRA`. The Small band uses the `BATCH` slot, but it is not the old batch block. The Small band accepts any task duration. More is outside the focus budget. A move into a full band is refused.

Plan bands use `FOCUS`, `SECONDARY`, `COMMITMENT`, and `MORE`. Capacity is also 1, 3, 5, and unlimited. Plan items are stored in the `goals` table and presented as tasks in the UI.

Add task and Plan item dialogs open wide. The task title and first step use full-width fields that grow with the text. Task and Plan item titles wrap in full in Daily and every Plan horizon, including the backlog, completed, and linked-task lists. Long unbroken links break instead of overflowing.

## Automatic carry

Automatic carry runs when Flow reconciles: at startup, every five minutes while the app is open, when the window becomes visible, and when the logical day changes. It is on by default.

- Daily: open tasks planned before today move to today. They keep their band. Within the band, a task displaced from the previous band comes first, then carried work, then the tasks already planned for today. If the band is full, its last task moves to the front of the next band. The cascade is Main 1, Next 3, Small 5, More unlimited.
- Plan: open items from earlier periods of the same horizon move to the current period. The same order and cascade rules apply. Week, Month, Quarter, and Year carry. Long term does not.
- Counters: Flow stores the first planned date or period and a rollover or carry count. Daily carry writes a `ROLLED_OVER` event with the old date. Plan carry does not write an event.
- Exclusions: habits, recurring tasks, Inbox items, unplanned Ready tasks, Waiting, Someday, Done, and Dropped items do not carry.
- The setting can be turned off in Settings.

Flow uses the rollover hour to decide the logical day; it defaults to midnight. The rail date and the default dates in the task action, triage, and habit dialogs use that day. Flow checks at the exact rollover boundary, every 30 seconds as a fallback, and when the window becomes visible; when the logical day moves, Flow reconciles and refreshes the open screen. Changing the rollover hour in Settings takes effect on the next check.

## Removed or legacy

The Markdown notes UI is gone, including its toolbar, preview, autosave, and the Autosave notes setting. `src/components/MarkdownNotes.tsx` was deleted, and `react-markdown`, `remark-gfm`, `rehype-sanitize`, and the dnd-kit packages were uninstalled. Daily and Plan are single-column again. The `horizon_notes` table, notes commands, and `notes_autosave` setting remain in the core and database for compatibility, but no active screen uses them.

## Dashboard

Dashboard ranges are Day, Week, Month, Quarter, Year, and All. Each range is anchored to the current period; there is no previous or next control. The summary combines daily tasks in the range with Plan items of the matching horizon that are not dropped. Day counts daily tasks only. All counts all Plan items that are not dropped. The daily charts and the habit and focus series are capped at 60 days. The Carried summary and Most carried list are all-time. The saved dashboard default range is applied at startup. See `docs/flow-v4.md` for the exact definitions.

## Data

The Settings screen shows the exact paths for your install. Default locations:

- Windows: `%APPDATA%\com.flow.focus\flow.db`
- macOS: `~/Library/Application Support/com.flow.focus/flow.db`
- Linux: `${XDG_DATA_HOME:-~/.local/share}/com.flow.focus/flow.db`

Flow Home can move the database and its backups to another folder. Choose a folder, confirm the move, and restart. Flow copies the database and backups, validates the copy, and keeps the old folder as a previous copy. An active session blocks the move. A pending move is stored in `flow-config.json` in the app config folder. Not now calls `cancel_data_move` and clears the pending move, so nothing moves on the next restart.

Backups sit in a `backups` folder beside the database. Flow makes one backup per logical day at startup and keeps the 14 newest files.

Export JSON and Backup database are in Settings. Both are one-way copies. There is no JSON import. To restore a backup, quit Flow, replace `flow.db` with a backup file, then reopen the app. There is no in-app restore or reset. The Reset button on Settings restores unsaved form fields only.

The database uses WAL mode, so a clean copy also needs any `flow.db-wal` and `flow.db-shm` files while Flow is running. Quitting first avoids that problem.

Only one Flow process can use the database. A second launch focuses the running window.

## Shortcuts

Global shortcuts are not installed. Flow only listens while its window has focus.

| Context | Keys | Action |
| --- | --- | --- |
| Anywhere | `Ctrl+K` or `Cmd+K` | Open capture |
| Anywhere | `Ctrl+1` to `Ctrl+6` | Go to Daily, Plan, Do, Try, Dashboard, or Settings |
| Anywhere | `Ctrl+Z` or `Cmd+Z` | Undo the last reversible task change |
| Do | `Space` | Start or pause the timer |
| Do | `D` | Complete the current task |
| Do | `E` | Extend a focus session by 15 minutes, once |
| Do | `S` | Stop for now and leave a breadcrumb |
| Do | `N` | Show the next planned task when no timer is running |
| Do | `B` | Move the current task to Waiting |
| Do | hold `Shift+Esc` for 1.5 s | Leave Do for Plan |
| Dialogs | `Esc` | Close without saving |

All six screens have number shortcuts. Closing the window quits Flow. Minimizing keeps it running so a session can continue.

## Structure

```text
core/                 flow-core: schema, rules, commands, tests
src-tauri/            Tauri host, startup, backups, IPC bridge
src/                  React UI
src/generated/        ts-rs output, regenerated by core tests
docs/flow-spec.md     The v2 build specification
docs/flow-v3.md       The v3 implementation and its limits
docs/flow-v4.md       The v4 implementation and its limits
scripts/ci.mjs        Format, lint, test, and build gates
DECISIONS.md          Scope and implementation decisions
PROGRESS.md           Current status, verification, and known gaps
```

All business rules live in `flow-core`. The Tauri layer translates commands and the React layer renders state. The UI never calculates plan rules, budgets, calibration, carry order, or lifecycle transitions.

## Test Workflow

Run the core library tests after changing any Rust type that uses `ts-rs`:

```powershell
cargo test -p flow-core --lib
```

That regenerates the TypeScript bindings in `src/generated`. The full workspace test then checks the desktop crate.

## Scope

Flow does not include sync, accounts, mobile, calendar or email integration, projects or tags, AI features, or multiple users. v4 adds automatic carry, the Dashboard, task action dialogs, and drag reorder on Daily and Plan. The rail is now six screens. Review and Patterns are gone. Some core workflows remain without an active screen: lock and suggest plan, the wrap-up resolver, recurring task editing, and habit history. See `PROGRESS.md` and `docs/flow-v4.md` for the current limits.
