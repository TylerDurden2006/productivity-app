# Flow v3

Status: implemented in code. Manual QA and `npm run ci` passed on 2026-10-23.

This document describes the v3 implementation. It covers what is in the repository now, including the parts that are still narrow. `docs/flow-spec.md` remains the v2 build specification.

## 1. What v3 adds

- A Daily page with due habits first, then 1/3/5/More task bands.
- A Plan page for goals across week, month, quarter, year, and long term, with previous and next period controls and an Inbox backlog.
- Habits as a separate system with a cadence engine and habit entries.
- A Try shelf for ideas, scheduling, skip, unschedule, and reflection.
- Goal to task linking in the Plan page.
- Flow Home for moving the database and backups to another folder.
- A light studio visual direction and a seven-item left rail.
- Single-instance protection.

## 2. Navigation

The rail contains, in order:

1. Daily
2. Plan
3. Do
4. Try
5. Review
6. Patterns
7. Settings

`Ctrl+1` to `Ctrl+7` open Daily, Plan, Do, Try, Review, Patterns, and Settings. `Ctrl+K` opens capture from any screen.

## 3. Daily

The Daily page is the task page.

### 3.1 Habits first

Due habits appear above the task bands. Each row shows the habit title, its cadence label, and either a quota count or a streak.

- Check off writes a `DONE` habit entry for that date.
- Skip writes a `SKIPPED` entry.
- Reopen deletes the entry for that date.
- Manage habits opens the habit editor and archive list.

The metrics row shows habits done, open tasks, tasks done today, and planned minutes against the budget.

### 3.2 Task bands

Four bands follow the habits:

| Band | Label in the UI | Internal slot | Capacity |
| --- | --- | --- | --- |
| 1 | Focus | `ONE` | 1 |
| 3 | Priorities | `THREE` | 3 |
| 5 | Commitments | `BATCH` | 5 |
| More | Overflow | `EXTRA` | Unlimited |

The internal slot names are kept for storage and generated types. The active UI uses the new labels.

- The 5 band accepts any task duration, including through a swap.
- More is collapsed by default and can hold any number of tasks.
- Tasks can be checked off from any band.
- The clock button on a task opens Do with that task selected.
- The completed list shows tasks completed today.

Add task supports two paths:

- Create a new task with a title, first step, and estimate, then place it in any band, including More.
- Pull an existing Ready task into any band.

Both paths put the task on today. The Plan page has no task assignment control.

A task planned for More is outside the focus budget. A task in bands 1, 3, or 5 counts its calibrated estimate.

## 4. Plan

The Plan page is the goal page. It uses the same 1/3/5/More shape at longer scales.

### 4.1 Horizons

Tabs: Week, Month, Quarter, Year, Long term.

- Week bounds follow `settings.week_starts_on`.
- Month, quarter, and year use calendar bounds.
- Long term has no start or end date.
- The page opens the current period for the selected horizon. Previous and next controls move the Week, Month, Quarter, and Year periods. Long term has no period navigation.

Each horizon has these goal bands:

| Band | Label | Capacity |
| --- | --- | --- |
| Focus | 1 | 1 |
| Secondary | 3 | 3 |
| Commitment | 5 | 5 |
| More | More | Unlimited |

Band capacity is enforced by the core. A full band returns `SLOT_FULL`.

### 4.2 Goals

A goal has a title, band, optional description, optional success definition, and optional target date. The create dialog exposes those fields. The UI does not expose area or parent goal selection.

Goals can be:

- Created in any band.
- Completed and reopened with the check control.
- Linked to one or more tasks.

Completed goals move to the Completed this period section below the bands. Goal editing exists in the core but has no action on the Plan page. There is no goal drop command; only the stored status value exists.

### 4.3 Goal links

Each goal row has a link button that opens `GoalTasksDialog`. The dialog shows:

- Linked tasks with an unlink action.
- Available tasks with a title filter.
- A Done button.

The available list is built from Ready tasks and tasks already on the current plan. Links are stored in `task_goals`. The goal row shows linked counts, done counts, and up to eight linked titles.

### 4.4 Week strip

For the Week horizon, the page shows seven day cards with the open task count and the done task count. The view model also carries planned minutes and lock state, but the cards do not render them. The strip is read only. Task assignment still follows the core today or tomorrow rule, and the Plan page has no task assignment control. Ready tasks are added from Daily, which targets today.

### 4.5 Backlog

A collapsible Backlog section lists Inbox items. Each row offers Clarify and Someday. The heading shows the Ready count against the cap. Ready tasks are added from Daily with Add task.

## 5. Do

Do shows one task at a time.

- The current task shows its title, first step, done-when, cue, and breadcrumb when present.
- The timer shows the task estimate before a session starts.
- Starting work creates a focus session. A focus session uses the focus block length from Settings.
- Space starts or pauses. Pause and resume keep elapsed time.
- `D` completes the task. A deep task with no session opens the manual time dialog. A 15-minute task can complete without it.
- `E` adds 15 minutes once per focus session.
- At the end of the block, the expiry dialog offers Done, one extension when available, or Stop.
- A focus session that reaches 60 minutes before an extension triggers a break prompt with a pause option and a skip option.
- `S` stops the session and can save a breadcrumb.
- `B` moves the current task to Waiting.
- `N` shows the next planned task when no timer is running.
- Holding `Shift+Esc` for 1.5 seconds leaves Do for Plan.

The older BATCH session path still exists in the core. The current UI starts FOCUS sessions only.

The Do view returns planned tasks for today. It does not require the day to be locked, and the Do screen can be opened from the rail at any time. The mode chip still comes from the core mode, which depends on the plan lock and the wrap-up state.

## 6. Habits

Habits are separate from tasks.

### 6.1 Records

- `habits` holds the title, why, cue, area, cadence, target count, unit label, time of day, sort order, active flag, and start or end bounds.
- `habit_entries` holds one row per habit and date. The unique key is `(habit_id, on_date)`.
- Entry status is `DONE` or `SKIPPED`.

Checking a habit never creates a task and never adds task time.

### 6.2 Cadence kinds

The cadence engine supports:

- Daily.
- Specific weekdays.
- Every N days.
- Every N weeks.
- Monthly on a day.
- Monthly on the nth weekday, including the last one.
- Yearly on a month and day.
- Custom dates.
- Quota, counted by week or month.
- One-off.

Every cadence has an anchor date. The cadence is stored as JSON in `habits.schedule_json`, with `anchor_on` also stored as a column. The engine validates intervals, weekdays, month days, months, weekday patterns, custom dates, and quota settings.

### 6.3 Due behavior and streaks

- A non-quota habit is due when the cadence matches the date and the date is inside the habit's start and end bounds.
- A quota habit is due until enough completed days exist in the current week or month.
- A fixed cadence streak counts consecutive scheduled days with `DONE` entries.
- A quota streak counts consecutive complete periods.

The Daily page orders due habits by time of day, then sort order, then title.

### 6.4 Archive and history

Archive sets `active = 0`. The habit stays in the manager list as archived and stops appearing on Daily. The Daily page can reopen today's entry. The API also exposes habit history and habit stats, but no screen renders them.

### 6.5 Recurring tasks

Recurring tasks are still a separate system. They use the older `freq`, `interval_n`, and `weekdays_mask` fields, with one open occurrence at a time. Their due strip and editor remain in the unused Plan screen, so the active screens do not surface them. The cadence engine is not wired into the recurring-task editor.

## 7. Try

The Try shelf is a list of things to try, not a task list.

### 7.1 Fields

An item has a title, notes, category, effort, cost, social mode, energy, status, scheduled date, scheduled day part, optional linked task id, rating, reflection, and tried timestamp.

The create dialog exposes title, category, notes, effort, cost, social mode, and energy. The link field is not exposed.

### 7.2 Status and actions

- Every new item starts as `IDEA`.
- Schedule sets a date and a day part (`AnyTime`, `Morning`, `Afternoon`, or `Evening`) and moves the item to `SCHEDULED`.
- Tried it opens a rating from 1 to 5 and an optional reflection, then sets `TRIED` and records the timestamp.
- The screen has a Skip action, which sets `SKIPPED`, and a Skipped filter.
- The screen can reschedule an item. Scheduled items also have an Unschedule action.
- Scheduling does not create a task or set `linked_task_id`.

### 7.3 Filters and order

Filters are All, Idea, Scheduled, Tried, and Skipped. Items sort by status (Scheduled, Idea, Tried, Skipped), then newest first.

## 8. Settings and Flow Home

### 8.1 Settings fields

- Day rollover hour.
- Daily focus budget.
- Ready cap.
- Focus block.
- Batch block.
- Default multiplier.
- Week starts on.

The Batch block field remains, but normal planning and normal focus do not read it. Planned minutes now count each task in bands 1, 3, and 5 individually.

Settings also holds the weekly review marker, export JSON, database backup, the database and backup paths, the newest backup path, and the Flow Home card.

### 8.2 Flow Home

Flow Home moves the database and backups.

1. Change folder opens a directory picker.
2. The chosen folder must be writable, must differ from the current folder, and must not already contain `flow.db`.
3. An active session blocks the move.
4. Choosing a folder writes a pending move to `flow-config.json` in the app config folder.
5. Restart and move restarts the app.
6. On the next start, Flow copies the database and backups to the new folder, validates the copy, records the old folder, and uses the new folder.
7. Open folder opens the current data folder in the file manager.

The old folder is kept as a previous copy and shown in Settings. Flow does not delete it.

The confirmation has a Not now control. It calls `cancel_data_move` and clears the pending move, so the move does not run on the next restart.

### 8.3 Restore

There is no in-app restore. To restore, quit Flow, replace `flow.db` with a backup file, and reopen the app. The Settings card states the path and the newest backup.

### 8.4 Single instance

`tauri-plugin-single-instance` is installed. A second launch focuses the running window. Two processes do not open the same database.

## 9. Data and migrations

| Migration | Adds |
| --- | --- |
| `001_init.sql` | Settings, tasks, sessions, days, events. |
| `002_planning.sql` | `week_starts_on`, areas, goals, task_goals, horizon_notes, goal_checkins. |
| `003_habits.sql` | habits and habit_entries. |
| `004_try.sql` | try_items. |
| `005_flexible.sql` | `schedule_json` and `anchor_on` on recurrences. |

Key storage rules:

- Goal links live in `task_goals`, keyed by task and goal.
- Habit entries are unique per habit and date.
- Cadence is stored as JSON, with the anchor date also in a column.
- Task slots keep the internal values `ONE`, `THREE`, `BATCH`, and `EXTRA`.
- Goal bands use `FOCUS`, `SECONDARY`, `COMMITMENT`, and `MORE`.

## 10. Rules to preserve

- Task band capacity is 1, 3, 5, and unlimited.
- Goal band capacity is 1, 3, 5, and unlimited.
- Planned minutes sum the calibrated estimates of tasks in bands 1, 3, and 5. More is not counted.
- The old rule that added `batch_block_mins` once is retired.
- Swapping a task into the 5 band accepts any task duration.
- Task assignment is limited to today or tomorrow in the core. Daily Add task targets today.
- Task estimates are limited to 15, 30, 60, 90, or 120 minutes in the core validator and the UI.
- Only one session can be active at a time.
- Mode is derived: a locked plan with no completed wrap-up gives Do, otherwise Plan.
- Habits use the cadence engine. Recurring tasks do not.
- A second app process does not open the database.

## 11. Tests

The v3 suites are:

- `core/tests/daily.rs`: due habits and bands, task check-off, habit check-off, budget exclusion for More.
- `core/tests/habits.rs`: daily, weekly, every N days, quota, entries, skip and reopen, ordering, streaks.
- `core/tests/horizons.rs`: period math, band capacity, reindexing, completion and reopen, task linking, areas.
- `core/tests/try_shelf.rs`: defaults, validation, scheduling, tried, skip, linked task checks.
- Cadence unit tests in `core/src/rules/cadence.rs`: anchored intervals, weekdays, month-end clamping, last weekday, custom dates, labels.

`npm run ci` passed on 2026-10-23: 173 tests, clippy, fmt, and the production build.

A full manual QA ran on a fresh database. Daily loaded 4 due habits and tasks. A habit check and a task check updated the Daily view. A goal check moved the goal to Completed. Goal-task linking worked. The Plan next and previous period controls worked. Settings showed the Flow Home card. Add task modal opened. No console errors appeared.

The data folder move was tested by moving a clean database to a temporary Flow Home and restarting. Data and backups were copied, and the old folder was kept. The database was reset afterward.

Windows MSI and NSIS installers were rebuilt after the v3 UI changes and the release binary was smoke tested against a fresh database.

## 12. Current limits

These are the parts of v3 that are incomplete in the active app:

- No Lock plan control. Lock, Suggest plan, apply plan, unassign, swap, pin, and restore remain in the unused `src/screens/PlanScreen.tsx`.
- No task assignment on the Plan page. Daily assigns to today only, and the core still limits assignment to today or tomorrow.
- No goal edit or drop action in the UI.
- Recurring tasks still use the older recurrence fields.
- Estimates are still preset buckets.
- Try scheduling does not create or link a task.
- No in-app restore.
- The Batch block setting is shown but no longer used by normal planning or focus. Normal focus uses one focus session per task.
- Habit history and stats have APIs but no screen.
- The week strip shows open and done counts, not planned minutes or lock state.
- There is no mobile app, sync, or cloud.
