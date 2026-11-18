# Flow v4

Status: implemented in the active desktop UI. The latest `npm run ci` run passed with 188 tests, clippy, formatting, and the production build. Manual QA ran on the current dev build. This documentation pass is dated 2026-10-24.

This document describes the v4 implementation. It covers what is in the repository now, including the parts that are still narrow. `docs/flow-spec.md` remains the v2 build specification, and `docs/flow-v3.md` remains the prior version record. The current source version is 1.0.3.

## 1. What v4 is

v4 is a smaller rail and a more automatic planner:

- Six screens: Daily, Plan, Do, Try, Dashboard, Settings.
- Automatic carry for daily tasks and Plan items.
- Plan items as first-class horizon tasks with checkboxes.
- Drag reorder on Daily and Plan.
- A task action dialog for pin, split, shrink, wait, and drop.
- A Dashboard with six ranges and charts.
- Review and Patterns removed, along with the weekly review setting, the batch block setting, the wrap-up wizard, and the Markdown notes UI.

## 2. Navigation and shortcuts

The rail contains, in order:

1. Daily
2. Plan
3. Do
4. Try
5. Dashboard
6. Settings

`Ctrl+1` to `Ctrl+6` open those screens. `Ctrl+K` or `Cmd+K` opens capture from any screen. `Ctrl+Z` or `Cmd+Z` runs the last reversible task change.

Do has its own shortcuts:

| Key | Action |
| --- | --- |
| `Space` | Start or pause the timer |
| `D` | Complete the current task |
| `E` | Extend a focus session by 15 minutes, once |
| `S` | Stop for now and leave a breadcrumb |
| `N` | Show the next planned task when no timer is running |
| `B` | Move the current task to Waiting |
| Hold `Shift+Esc` for 1.5 s | Leave Do for Plan |

Dialogs close on `Esc`. Global shortcuts are not installed. Flow listens only while its window has focus.

## 3. Daily

Daily is the day page. It uses a single-column layout.

### 3.1 Metrics and habits

The metrics row has three cards:

- Habits: done of total.
- Open today: open tasks across all bands.
- Done today: completed tasks.

The old Planned card is gone.

Due habits appear above the task bands. Each row shows the habit title, its cadence label, and either a quota count or a streak.

- Check off writes a `DONE` habit entry for that date.
- Skip writes a `SKIPPED` entry.
- Reopen deletes the entry for that date.
- Manage habits opens the habit editor and archive list.

Habits are separate from tasks. Checking a habit never creates a task and never adds task time.

The cadence engine supports daily, specific weekdays, every N days, every N weeks, monthly on a day, monthly on the nth weekday, yearly, custom dates, quota by week or month, and one-off. Archive sets `active = 0` and keeps history. The API also exposes habit history and stats, but no screen renders them.

### 3.2 Task bands

Four bands follow the habits:

| Band | Label in the UI | Internal slot | Capacity |
| --- | --- | --- | --- |
| 1 | Main | `ONE` | 1 |
| 3 | Next | `THREE` | 3 |
| 5 | Small | `BATCH` | 5 |
| More | Overflow | `EXTRA` | Unlimited |

The internal slot names are kept for storage and generated types. The Small band uses the `BATCH` slot, but it is not the old batch block.

- Small accepts any task duration, including through a drag or a swap.
- More is collapsed by default and can hold any number of tasks.
- Tasks can be checked off from any band.
- The clock button on a task opens Do with that task selected.
- The completed list shows tasks completed today, newest first.

Add task supports two paths:

- Create a new task with a title, first step, and estimate, then place it in any band, including More.
- Pull an existing Ready task into any band.

Both paths put the task on today. The Plan page has no daily task assignment control.

A task planned for More is outside the focus budget. A task in Main, Next, or Small counts its calibrated estimate.

### 3.3 Task actions

The dots button on a Daily task opens `TaskActionsDialog`. It offers:

- Pin or unpin. The pin flag is used when the core ranks Ready candidates. It does not move a task that is already planned.
- Split into smaller tasks. The parent closes with `SPLIT`, and each child becomes a Ready task with its own first step and estimate.
- Shrink the first step. The estimate must be no larger than the current estimate, and the rollover count resets to zero.
- Move to Waiting. This asks for the person or thing and a review date.
- Drop the task. This closes it with `NOT_DOING` and keeps it in history.

The split and shrink dialogs share an outcome form that can also switch to carry, wait, someday, or drop. Plan items do not have this action dialog.

### 3.4 Drag and reorder

Tasks have a drag handle. The underlying command is `reorder_task` with the day, task id, target slot, and target index.

- Drag works within a band and across Main, Next, and Small.
- Drag works onto a More row. The empty More list is not a drop target.
- A move into a full band is refused with `SLOT_FULL`.
- The command reindexes every slot for the day in one transaction.

### 3.5 Completed and foot

A Completed today section appears when at least one task is done. The foot line shows the Inbox count when there are items, or a short planning line when it is empty, plus a link to Plan.

### 3.6 Long text

Add task and Plan item dialogs use a large modal at up to 800px. The task title and first step are full-width auto-growing textareas. They grow with the content, cap at 220px, and scroll inside after that. Line breaks collapse to spaces when the title is saved.

Task and Plan item titles wrap in full in Daily, every Plan horizon, the Plan backlog, completed rows, and linked-task rows. Long unbroken tokens break with `overflow-wrap: anywhere`. Rows grow in height, and the dialogs keep their footer visible by scrolling the body.

## 4. Plan

Plan is the horizon page. It uses a single-column layout.

### 4.1 Horizons

Tabs: Week, Month, Quarter, Year, Long term.

- Week bounds follow `settings.week_starts_on`.
- Month, quarter, and year use calendar bounds.
- Long term has no start or end date.
- The page opens the current period for the selected horizon.
- Previous and next controls move the Week, Month, Quarter, and Year periods.
- Long term has no period navigation.

Each horizon has the same four bands:

| Band | Label | Internal band | Capacity |
| --- | --- | --- | --- |
| 1 | Main | `FOCUS` | 1 |
| 3 | Next | `SECONDARY` | 3 |
| 5 | Small | `COMMITMENT` | 5 |
| More | Overflow | `MORE` | Unlimited |

Band capacity is enforced by the core. A full band returns `SLOT_FULL`.

### 4.2 Plan items

Plan items are stored in the `goals` table. The code and IPC still call them goals. The UI presents them as tasks.

A Plan item has a title, band, optional description, optional success definition, and optional target date. The create dialog exposes those fields. It does not expose area or parent item selection.

Plan items can be:

- Created in any band.
- Completed and reopened with the check control.
- Linked to one or more daily tasks.
- Reordered by drag.

Completed items move to the Completed this period section below the bands. Reopening an item can fail if its original band is full.

Plan items do not appear on Daily or Do. Completing a Plan item does not complete its linked daily tasks, and completing linked daily tasks does not complete the Plan item.

Plan item editing exists in the core through `update_goal`, but the page has no edit action. There is no drop command; only the stored status value exists.

The create dialog uses the same Main, Next, Small, and More labels as the page headers.

### 4.3 Drag and reorder

Plan items have a drag handle. The underlying command is `reorder_goal` with the horizon kind, period start, item id, target band, and target index.

- Drag works within a band and across all bands, including More.
- A move into a full band is refused with `SLOT_FULL`.
- The command reindexes the target period in one transaction.

### 4.4 Linking

Each Plan item row has a link button that opens `GoalTasksDialog`. The dialog shows:

- Linked tasks with an unlink action.
- Available tasks with a title filter.
- A Done button.

The available list is built from Ready tasks and tasks already on the current plan. Links are stored in `task_goals`. A Plan item can link to more than one task, and a task can link to more than one Plan item. The row shows linked counts, done counts, and up to eight linked titles.

Links are informational. They do not move tasks, complete tasks, or carry tasks.

### 4.5 Completed, week strip, and backlog

- Completed this period lists done items for the selected horizon and period.
- The week strip appears only on Week. It shows seven day cards with open and done task counts. It does not render planned minutes or lock state.
- The Backlog section is collapsible. It lists Inbox items with Clarify and Someday actions. The heading shows the Ready count against the cap. Ready tasks are added from Daily with Add task.

## 5. Automatic carry

Automatic carry is on by default through `auto_carry_enabled`. It replaces the manual resolution step for overdue work.

### 5.1 When it runs

Carry runs inside `reconcile`. Reconcile runs:

- At startup, after the database opens and passes its integrity check.
- Every five minutes while the app is open.
- When the window becomes visible again.
- When the UI detects that the logical day changed.

The same reconcile pass freezes stale sessions and materializes recurring task occurrences.

### 5.2 Daily carry

Sources are `READY` tasks with no recurrence, a planned date before today, and a slot. Targets are the tasks already planned for today.

For each slot in `ONE`, `THREE`, `BATCH`, `EXTRA`, the order is:

1. Overflow from the previous slot.
2. Carried work for this slot.
3. Today's planned tasks in this slot, including any completed ones.

The first 1, 3, or 5 tasks stay, depending on the slot. The rest overflow to the next slot. More is unlimited.

The visible result:

- Carried work keeps its band when there is room.
- A task displaced from the previous band comes first.
- Then carried work.
- Then the tasks already planned for today, including completed ones.
- If a band is full, its last task moves to the front of the next band.

Completed tasks keep their planned date and slot, so they are part of the target order and can be renumbered. Daily hides them from the bands.

The pass clears today's slots and rewrites every position in one transaction. Only the carried source tasks are reported in the reconcile result. Today's renumbered tasks are not.

### 5.3 Plan carry

Plan carry runs for Week, Month, Quarter, and Year. Long term is excluded.

- Sources are `OPEN` items of the same horizon with a period start earlier than the current period start.
- Targets are `OPEN` items of that horizon in the current period.
- The same order and cascade rules apply per band.
- Carried items keep their band when there is room and cascade through 1, 3, 5, and unlimited.

### 5.4 Counters and events

- Daily tasks store `first_planned_for` on first assignment and increment `rollover_count` on each carry. Daily carry writes a `ROLLED_OVER` event with the old date.
- Plan items store `first_period_start` at creation and increment `carry_count` on each carry. Plan carry does not write an event.
- The `Task` and `Goal` IPC types do not expose `first_planned_for` or `carry_count`. The Dashboard reads the columns directly.

### 5.5 Exclusions and settings

- Habits are separate records and never carry.
- Recurring task occurrences are excluded from daily carry. The recurrence system still materializes them during reconcile.
- Inbox items, unplanned Ready tasks, Waiting, Someday, Done, and Dropped items do not carry.
- Done and dropped Plan items do not carry.
- Turning automatic carry off stops both daily and Plan carry. Reconcile still freezes stale sessions and materializes recurrences.

## 6. Do

Do shows one task at a time.

- The current task shows its title, first step, done-when, cue, and breadcrumb when present.
- The timer shows the task estimate before a session starts.
- Starting work creates a focus session. A focus session uses `focus_block_mins`.
- Space starts or pauses. Pause and resume keep elapsed time.
- `D` completes the task. A deep task with no session opens the manual time dialog. A 15-minute task can complete without it.
- `E` adds 15 minutes once per focus session.
- At the end of the block, the expiry dialog offers Done, one extension when available, or Stop.
- A focus session that reaches 60 minutes before an extension triggers a break prompt with a pause option and a skip option.
- `S` stops the session and can save a breadcrumb.
- `B` moves the current task to Waiting.
- `N` shows the next planned task when no timer is running.
- Holding `Shift+Esc` for 1.5 seconds leaves Do for Plan.

When the desk is empty, Do points back to Daily and offers a pull from Ready. No active screen locks the plan, so the legacy DO branch is not normally reachable.

The older BATCH session path still exists in the core. The current UI starts FOCUS sessions only.

The mode chip still comes from the core mode, which depends on the plan lock and the wrap-up state. No active screen locks the plan or completes a wrap-up, so the chip usually reads Planning. Do can be opened from the rail at any time.

## 7. Dashboard

The Dashboard has six ranges: Day, Week, Month, Quarter, Year, and All. Each range is anchored to the current period. There is no previous or next control.

### 7.1 Summary

The summary cards show:

- Planned: daily tasks in the range plus Plan items of the matching horizon that are not dropped.
- Done: the same combination.
- Left: planned minus done.
- Carried: open items with carry history.

The small text under Planned shows the daily and horizon split. The small text under Left shows the overdue count.

Definitions:

- Day counts daily tasks only. It does not add Plan items.
- Week, Month, Quarter, and Year add Plan items of that same horizon. They do not add lower or higher horizons.
- All counts every Plan item that is not dropped.
- Overdue counts open daily tasks whose first planned date is before the range start, plus open Plan items whose first period start is before the range start. For All, the threshold is today.
- Carried counts open daily tasks with `rollover_count > 0` plus open Plan items with `carry_count > 0`. This count is all-time, not filtered by range.

### 7.2 Charts and lists

- Completion donut: done versus left.
- Daily rhythm bars: planned versus done for each day in the series. These bars use daily tasks only.
- Completion by section: tasks by slot plus Plan items by band. For Day it uses tasks only. For Week, Month, Quarter, and Year it uses Plan items of that horizon. For All it uses every Plan item that is not dropped.
- Habit consistency: due and done habit entries for each day in the series, per habit.
- Focus time: ended sessions for each day, rounded up to whole minutes.
- Most carried: up to five open items, drawn from tasks and Plan items, with carry history. This list is all-time, not filtered by range.
- Try: scheduled and tried counts in the range.

### 7.3 The 60-day cap

The daily series is capped at 60 days. When the selected range is longer than 60 days, the series uses the last 60 days of the range.

- For Quarter and Year, the range end can be in the future, so the series is the last 60 days of the period, not the last 60 days up to today.
- The summary still counts the full range.
- The daily bars, habit consistency, and focus total use the capped series.
- The donut, section completion, carried, overdue, and Try counts use the full range, except where noted above.

### 7.4 Default range

`dashboard_default_range` is saved and validated in Settings. The store reads it at startup, so Dashboard opens on the chosen range.

## 8. Try

The Try shelf is a list of things to try, not a task list.

An item has a title, notes, category, effort, cost, social mode, energy, status, scheduled date, scheduled day part, optional linked task id, rating, reflection, and tried timestamp.

The create dialog exposes title, category, notes, effort, cost, social mode, and energy. The link field is not exposed.

- Every new item starts as `IDEA`.
- Schedule sets a date and a day part (`ANYTIME`, `MORNING`, `AFTERNOON`, or `EVENING`) and moves the item to `SCHEDULED`.
- Tried it opens a rating from 1 to 5 and an optional reflection, then sets `TRIED` and records the timestamp.
- The screen has a Skip action, which sets `SKIPPED`, and a Skipped filter.
- The screen can reschedule an item. Scheduled items also have an Unschedule action.
- Scheduling does not create a task or set `linked_task_id`.

Filters are All, Idea, Scheduled, Tried, and Skipped. Items sort by status (Scheduled, Idea, Tried, Skipped), then newest first.

## 9. Settings

### 9.1 Fields

- Day rollover hour.
- Daily focus budget.
- Focus block.
- Week starts on.
- Automatic carry.
- Dashboard opens on.
- Block-end notification.
- Nightly planning reminder and reminder time.
- Reduce motion.
- Advanced: Ready cap and default multiplier.

The Batch block field, the weekly review control, and the Autosave notes control are gone from the UI.

### 9.2 Behavior

- Automatic carry controls both daily and Plan carry.
- Dashboard opens on is saved, validated, and read when the app boots.
- Block-end notification controls the native block-end and break notices. In-app dialogs still appear if permission is denied.
- The nightly planning reminder checks once a minute while Flow is open. It compares the local time to the reminder time and sends one notice per day. If Flow is closed at that time, nothing fires.
- Reduce motion sets a data attribute that shortens animations and transitions.

### 9.3 Flow Home, export, and backup

Flow Home moves the database and backups.

1. Change folder opens a directory picker.
2. The chosen folder must be writable, must differ from the current folder, and must not already contain `flow.db`.
3. An active session blocks the move.
4. Choosing a folder writes a pending move to `flow-config.json` in the app config folder.
5. Restart and move restarts the app.
6. On the next start, Flow copies the database and backups to the new folder, validates the copy, records the old folder, and uses the new folder.
7. Open folder opens the current data folder in the file manager.

The old folder is kept as a previous copy and shown in Settings. Flow does not delete it. The confirmation has a Not now control that calls `cancel_data_move` and clears the pending move.

Export JSON and Backup database are one-way copies. There is no JSON import. Restore is a manual file replacement while Flow is closed. There is no in-app restore or reset. The Reset button on Settings restores unsaved form fields only.

### 9.4 Single instance

`tauri-plugin-single-instance` is installed. A second launch focuses the running window. Two processes do not open the same database.

## 10. Data and migrations

| Migration | Adds |
| --- | --- |
| `001_init.sql` | Settings, tasks, sessions, days, events. |
| `002_planning.sql` | `week_starts_on`, areas, goals, task_goals, horizon_notes, goal_checkins. |
| `003_habits.sql` | habits and habit_entries. |
| `004_try.sql` | try_items. |
| `005_flexible.sql` | `schedule_json` and `anchor_on` on recurrences. |
| `006_carry_notes.sql` | `first_planned_for` on tasks; `first_period_start` and `carry_count` on goals; an open period index; `body_markdown` on horizon_notes; carry, dashboard, notes, notification, reminder, and motion settings. |

Key storage rules:

- Task slots keep the internal values `ONE`, `THREE`, `BATCH`, and `EXTRA`.
- Plan bands use `FOCUS`, `SECONDARY`, `COMMITMENT`, and `MORE`.
- Goal links live in `task_goals`, keyed by task and goal.
- Habit entries are unique per habit and date.
- Cadence is stored as JSON, with the anchor date also in a column.
- Carry counters live on tasks and goals. Daily carry also writes `ROLLED_OVER` events.

## 11. Removed or legacy

Removed from the active app:

- The Review and Patterns screens. Their source files are gone.
- The weekly review setting and control.
- The wrap-up wizard and its manual resolution step.
- The Batch block field in Settings.
- The old Plan screen. Its file is gone.
- The Markdown notes UI on Daily and Plan, including its toolbar, preview, autosave, manual Save, and per-period storage. `src/components/MarkdownNotes.tsx` is deleted.
- The Autosave notes control in Settings.
- The `react-markdown`, `remark-gfm`, `rehype-sanitize`, and dnd-kit packages.
- The two-column workspace on Daily and Plan. Both pages are single-column again.

Legacy code that remains:

- The `horizon_notes` table, the notes commands in `flow-core`, the notes unit tests, and the `notes_autosave` setting remain. No active screen uses them.
- `last_weekly_review_at` is still in settings, and `weekly_review_due` is still computed in the Plan view. No screen uses them.
- The `wrapup` module and the `complete_wrapup`, `reopen_wrapup`, and `carry_all` commands remain. No screen runs the wrap-up flow.
- The BATCH session path remains in the core, and `batch_block_mins` remains in the settings table. No active screen starts a BATCH session or edits the setting. Only that unused path uses it for timing.
- Plan lock, suggest plan, apply plan, unassign, swap, and restore commands remain in the core. No active screen calls them.
- The old Plan screen and its unused dialogs were removed.
- The mode chip still depends on plan lock and wrap-up state, which no active screen changes.

## 12. Rules to preserve

- Daily band capacity is 1, 3, 5, and unlimited.
- Plan band capacity is 1, 3, 5, and unlimited.
- Planned minutes sum the calibrated estimates of open tasks in Main, Next, and Small. More is not counted.
- Small accepts any task duration.
- Task assignment is limited to today or tomorrow in the core. Daily Add task targets today.
- Task estimates are limited to 15, 30, 60, 90, or 120 minutes in the core validator and the UI.
- Only one session can be active at a time.
- Mode is derived: a locked plan with no completed wrap-up gives Do, otherwise Plan.
- Habits use the cadence engine. Recurring tasks use the older recurrence fields.
- Automatic carry keeps the band, places carried work ahead of the period's existing tasks, and cascades displaced work through 1, 3, 5, and unlimited.
- Recurring tasks, habits, Inbox items, unplanned Ready tasks, Waiting, Someday, Done, and Dropped items do not carry.
- A second app process does not open the database.

## 13. Tests and manual QA

The latest project gate passed:

```text
npm run ci    pass (cargo fmt, cargo clippy, 188 tests, npm run build)
```

The run covers core unit tests and the acceptance suites for carry, Daily, habits, horizons, Try, sessions, planning, lifecycle, tasks, safety, performance, property, calibration, and wrap-up. Dashboard and cadence unit tests run inside the core library. The core library run also exports the TypeScript bindings in `src/generated`.

Manual QA ran on the current dev build:

- Dashboard loaded its charts and summary.
- Settings saved.
- The task action dialog opened.
- Dragging a task reordered the Next tasks.
- Dragging a Plan item moved it into Next.
- No console errors appeared.

Still to check by hand: carry across a real date boundary, notification delivery, the restore path, a clean-machine installer, and builds on macOS and Linux.

The current version is 1.0.3. Windows builds write `Flow_1.0.3_x64-setup.exe` and `Flow_1.0.3_x64_en-US.msi` under `target\release\bundle`; the installer is unsigned. Flow is installed for the current Windows user at `%LOCALAPPDATA%\Flow`, with the Start Menu shortcut `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Flow.lnk`, an Apps and Features entry, and no desktop shortcut. `Get-StartApps` reports Flow with AppID `com.flow.focus`. The installed copy reports version 1.0.3. The app data stays in `%APPDATA%\com.flow.focus`. The bundle contains no macOS or Linux package.

## 14. Current limits

- No in-app restore or reset. The Settings Reset button restores form fields only.
- No custom task estimate UI. Estimates remain preset buckets.
- Recurring tasks still use the older recurrence fields. No active screen exposes the editor, and the cadence engine is not wired into recurring tasks.
- Try scheduling does not create or link a task.
- Habit history and stats have APIs but no dedicated screen.
- The wrap-up core remains for compatibility. No screen runs the wrap-up flow.
- Task assignment for future days is not exposed. Daily Add task targets today, and Plan items are horizon records.
- The batch block column remains in the database but has no active UI or role in normal planning or focus.
- The Dashboard daily series is capped at 60 days. Charts, habit consistency, and focus totals can cover less than the selected range.
- No macOS or Linux installer is built.
- No macOS or Linux installer was built.
- The Dashboard carried totals are all-time, not filtered by the selected range.
- The Carried summary and Most carried list are all-time, not range-filtered.
- Plan items cannot be edited or dropped in the UI.
- The week strip shows counts only. The Dashboard has no previous or next period control.
- Some legacy core commands remain unused. The old Plan screen and unused dialogs are gone.
- There is no mobile app, sync, or cloud.

