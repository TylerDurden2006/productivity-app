# Decisions

This file records the choices that shape Flow where the spec left room to interpret, or where the product scope changed during the build.

## Product scope

The desktop shell stays deliberately small:

- No global capture shortcut. `Ctrl+K` or `Cmd+K` works only while Flow has focus.
- No separate always-on-top capture window.
- No tray icon, close-to-tray behavior, or autostart.
- Closing the window quits Flow. Minimizing keeps it running so a focus session can continue.
- Single-instance protection is installed through `tauri-plugin-single-instance`. A second launch focuses the running window instead of opening a second process. This replaces the earlier choice to let a second process fail on the database lock.

These choices are surfaced in Settings under "Quiet by default". The settings table omits `capture_shortcut` because no global shortcut is registered. Settings shows "Tray or global shortcut: Not installed" instead. The in-app shortcut is `Ctrl+K` or `Cmd+K`.

## Navigation and removed surfaces

The left rail has six screens, in order:

1. Daily
2. Plan
3. Do
4. Try
5. Dashboard
6. Settings

`Ctrl+1` to `Ctrl+6` open those screens. `Ctrl+K` opens capture from any screen.

The Review and Patterns screens are gone from the source. The weekly review setting is gone from Settings. The wrap-up wizard and its manual resolution step are gone. Task-level resolution remains in the Daily task action dialog.

Two pieces of the old flow still have code but no dedicated screen:

- No active screen locks the plan, so the legacy DO branch is not normally reachable. The Dashboard no longer shows recent win notes, and no Review flow uses them.
- The wrap-up resolver, its completion and reopen commands, and the manual carry command remain in the core. No screen runs the wrap-up flow.

## Removed or legacy

The Markdown notes UI was removed, including its toolbar, preview, autosave, and the Autosave notes setting. `src/components/MarkdownNotes.tsx` is deleted, and `react-markdown`, `remark-gfm`, `rehype-sanitize`, and the dnd-kit packages are no longer dependencies. Daily and Plan are single-column pages. The `horizon_notes` table, notes commands, notes unit tests, and `notes_autosave` setting remain in the core and database for compatibility, but no active screen uses them.

## Daily bands and budget

The visible task hierarchy is Main, Next, Small, and More. The schema and generated types still use the older slot names:

- Main is `ONE`.
- Next is `THREE`.
- Small is `BATCH`.
- More is `EXTRA`.

The names remain in the database and generated types. A rename would touch stored rows, migrations, and generated types. The active UI shows the new labels.

The Small band is not the old micro-task batch:

- Direct assignment to Small accepts any task duration. Dragging a task into Small follows the same rule. `BATCH_REQUIRES_MICRO` is no longer part of the assignment, reorder, or swap path.
- Planned minutes are the sum of calibrated estimates for open tasks in Main, Next, and Small. More is outside the focus budget.
- `batch_block_mins` remains in the settings table and in `update_settings`, but Settings does not show it. Normal planning and normal focus do not read it. Only the unused BATCH session path uses it for timing.
- Do starts a focus session for one task at a time. The core still contains the older BATCH session code and its 15-minute guards, but no active screen starts a BATCH session.
- More is assignable during planning. `pull_extra` remains for pulling one more task in from Ready when the planned work is done.

## Drag and reorder

Daily and Plan both use drag handles and native drag and drop.

- Daily uses `reorder_task` with a day, task id, target slot, and target index. The command refuses a move into a full band and reindexes every slot for that day.
- Plan uses `reorder_goal` with a horizon kind, period start, item id, target band, and target index. It applies the same capacity rule and reindexes the target period.
- Daily drag works within and across the Main, Next, and Small lists, and onto a More row. The empty More list is not a drop target.
- Plan drag works within and across the band lists, including More.
- A move into a full band returns `SLOT_FULL`. The UI reports it as a toast.

## Plan horizons

The Plan page keeps a separate record from the daily task page. It has five horizons:

- Week.
- Month.
- Quarter.
- Year.
- Long term.

Plan items are stored in the `goals` table. The code and IPC still call them goals. The UI creates them with "Add task", shows them in the same Main, Next, Small, and More bands, and completes them with a checkbox. Plan items do not appear on Daily or Do.

Band capacity is hard: 1, 3, 5, and unlimited. Long term items have no period start or end.

Week boundaries use `settings.week_starts_on`, which Settings exposes as a weekday choice. Month, quarter, and year bounds are calendar based.

Plan items can be created, completed, reopened, and linked to daily tasks. A link is stored in `task_goals`, and one item can link to more than one task. The Plan page shows linked counts, up to eight linked titles, and a link dialog that can add or remove links. The picker lists Ready tasks and tasks already on the current plan. Links are informational: completing a Plan item does not complete its linked tasks, and completing linked tasks does not complete the Plan item.

The page opens the current period by default. Previous and next controls move the Week, Month, Quarter, and Year periods. Long term has no period navigation. Completed items for the selected period appear in the Completed this period section. Reopening an item can fail if its original band is full.

Plan item editing exists in the core through `update_goal`, but the Plan page has no edit action. There is no goal drop command; only the stored status value exists.

The Plan page has a collapsible Backlog for Inbox items. Each item offers Clarify and Someday. The heading shows the Ready count against the cap. Ready tasks are added from Daily with Add task.

The week strip appears only on the Week horizon. It shows seven day cards with open and done task counts. The view model also carries planned minutes and lock state, but the card does not render them. The strip is a summary only.

Task assignment still follows the core rule that allows today or tomorrow only, and the current Plan page has no task assignment control.

## Automatic carry

Automatic carry replaces the manual resolution step for overdue work. It is on by default through `auto_carry_enabled`.

The carry pass runs inside `reconcile`, which runs at startup, every five minutes while the app is open, when the window becomes visible, and when the UI detects a logical-day change. The same pass freezes stale sessions and materializes recurring task occurrences.

Daily carry:

- Sources are `READY` tasks with no recurrence, a planned date before today, and a slot.
- Targets are the tasks already planned for today.
- For each slot in `ONE`, `THREE`, `BATCH`, `EXTRA`, the order is: overflow from the previous slot, then carried work for this slot, then today's planned tasks in that slot, including any completed ones.
- The first 1, 3, or 5 tasks stay, depending on the slot. The rest overflow to the next slot. More is unlimited.
- Carried work therefore keeps its band when there is room. A task displaced from the previous band comes first, then carried work, then the tasks already planned for today, including completed ones.
- The pass clears today's slots and rewrites every position in one transaction.

Plan carry:

- Sources are `OPEN` items of the same horizon with a period start earlier than the current period start.
- Targets are `OPEN` items of that horizon in the current period.
- The same order and cascade rules apply per band.
- Week, Month, Quarter, and Year carry. Long term does not.

Counters and history:

- Daily tasks store `first_planned_for` on first assignment and increment `rollover_count` on each carry. Daily carry writes a `ROLLED_OVER` event with the old date.
- Plan items store `first_period_start` at creation and increment `carry_count` on each carry. Plan carry does not write an event.
- The `Task` and `Goal` IPC types do not expose `first_planned_for` or `carry_count`. The Dashboard reads the columns directly.

Exclusions:

- Habits are separate records and never carry.
- Recurring task occurrences are excluded from daily carry. The recurrence system still materializes them during reconcile.
- Inbox items, unplanned Ready tasks, Waiting, Someday, Done, and Dropped items do not carry.
- Done and dropped Plan items do not carry.
- Turning automatic carry off stops both daily and Plan carry. Reconcile still freezes stale sessions and materializes recurrences.

## Live dates

The core is the source of truth for the logical day. `Clock::logical_today` subtracts `day_rollover_hour`, and every backend view uses `db::today`.

The UI must not freeze the date at launch. `src/lib/format.ts` exposes `logicalTodayISO`. `App` compares that value every 30 seconds and when the window becomes visible. When it changes, the rail date updates, `reconcile` runs, and the open screen refreshes. The task action, triage, and habit dialogs use the same helper, so their default dates follow the rollover hour. Changing the rollover hour in Settings takes effect on the next check.

## Long text

Add task and Plan item dialogs use a large modal at up to 800px. The task title and first step are full-width `AutoGrowTextarea` fields. The field grows with its content, caps at 220px, and scrolls inside after that. Titles collapse line breaks to spaces on save.

Task and Plan item titles do not use ellipsis in the active lists. Daily rows, Plan horizon rows, the Plan backlog, completed rows, and linked-task rows grow vertically and wrap long unbroken tokens with `overflow-wrap: anywhere`. The body scrolls only when the whole dialog would exceed the viewport.

## Habits

Habits are separate from tasks. They live in `habits` and `habit_entries`, not in `tasks`.

- Every habit has a cadence: daily, specific weekdays, every N days, every N weeks, monthly on a day, monthly on the nth weekday, yearly, custom dates, quota, or one-off.
- Checking a habit writes a `habit_entries` row for the date. A completed row has status `DONE`. It does not create a task and does not add task time.
- Skip writes a `SKIPPED` entry. Reopen deletes the entry.
- A quota habit is due until enough completed days exist in the current week or month. Its streak counts complete quota periods.
- Other habits build a streak across scheduled days.
- Habits are archived with `active = 0`. History stays in place.
- The Daily page orders due habits by time of day, then sort order, then title.

The cadence engine is used only by habits. Recurring tasks still use the older `freq`, `interval_n`, and `weekdays_mask` fields. Migration 005 adds `schedule_json` and `anchor_on` to `recurrences`, but the recurrence rules do not read them. The recurrence editor component was removed. This is a known gap.

## Try shelf

The Try shelf is a separate list, not a task list.

- An item starts as `IDEA`.
- Scheduling sets a date and a day part, and moves the item to `SCHEDULED`.
- Marking it tried sets `TRIED`, records an optional rating from 1 to 5, an optional reflection, and a timestamp.
- Skip sets `SKIPPED`. The Try screen has a Skip action and a Skipped filter.
- Scheduling does not create or link a task. `linked_task_id` and `set_try_item_task` exist in the API, but no screen sets the link.
- Reschedule and Unschedule are available from the card.

## Dashboard

The Dashboard has six ranges: Day, Week, Month, Quarter, Year, and All. Each range is anchored to the current period. There is no previous or next control.

- Planned combines daily tasks in the range with Plan items of the matching horizon that are not dropped. Day counts daily tasks only. All counts all Plan items that are not dropped.
- Done uses the same combination. Left is planned minus done.
- The daily bars use daily tasks only. Plan items appear in the summary and section totals, not in the daily bars.
- Section completion combines tasks by slot with Plan items by band. For Day it uses tasks only. For Week, Month, Quarter, and Year it uses Plan items of that horizon. For All it uses every Plan item that is not dropped.
- Habit consistency counts due and done habit entries for each day in the series.
- Focus time sums ended sessions for each day and rounds each day up to whole minutes.
- Try counts scheduled and tried items in the range.
- Carried counts open tasks and Plan items with carry history. Most carried lists up to five open items.
- The daily series is capped at 60 days. For Quarter and Year the range end can be in the future, so the series is the last 60 days of the period, not the last 60 days up to today. The summary still covers the full range.
- The Carried summary and Most carried list are all-time, not filtered by range. This is a known gap.
- `dashboard_default_range` is saved, validated, and applied when the app boots.

## Flow Home

Flow Home moves the database and its backups to a folder the user picks.

- The folder must be writable, must not be the current folder, and must not already contain a `flow.db`.
- An active session blocks the move.
- Choosing a folder writes a pending move to `flow-config.json` in the app config folder.
- The next launch copies the database and backups, validates the copy, records the old folder, and uses the new folder. The old folder is kept, not deleted.
- Not now calls `cancel_data_move` and clears the pending move. Nothing moves on the next restart.

## Data and backups

- JSON export is a one-way copy. There is no JSON import. This avoids a second write path that could bypass the task lifecycle rules.
- Flow writes one startup backup per logical day and keeps the 14 newest files. The backup runs before the session starts.
- The database uses WAL mode. Copying a live database requires the `-wal` and `-shm` files too, so the documented restore path starts with quitting the app.
- Restoring a database remains a manual file replacement while Flow is closed. Settings shows the exact path and the newest backup. There is no in-app restore or reset. The Reset button on Settings restores unsaved form fields only.
- A second app process is blocked by the single-instance plugin. It focuses the existing window.

## Notifications

End-of-block and break notices use `tauri-plugin-notification`. The UI asks for permission when the first session starts, then sends a native notification when a block expires or a break is due. This replaces an earlier browser `Notification` fallback, which WebView2 reports as denied in the desktop shell.

The `notify_block_end` setting controls both block-end and break notices. If permission is denied, the in-app expiry and break dialogs still appear. Notification delivery is a convenience, not a dependency for the timer rules.

The nightly planning reminder is separate. Flow checks once a minute while the app is open, compares the local time to `planning_reminder_time`, and sends one reminder per day. The per-day marker is stored in local storage. If Flow is closed at the reminder time, nothing fires.

## Boulder and Ready overflow

Two soft gates remain in the core:

- **Boulder.** A calibrated estimate over 120 minutes asks for a short reason. The override dialog accepts the reason. Splitting is a separate action in the Daily task action dialog.
- **Ready overflow.** A task that is not 15 minutes long stops at the Ready cap. The override dialog can send it to Someday. It does not offer moving the lowest-ordered Ready task to Someday. A 15-minute task bypasses the cap.

Both gates are reachable through capture, clarify, and the override dialog. The old Review wizard that also reached them is gone.

## Windows installation

Flow ships as a normal Windows desktop app rather than a portable executable.

- The NSIS installer uses the `currentUser` mode, so it does not require an administrator prompt.
- The installed executable is `%LOCALAPPDATA%\Flow\Flow.exe`.
- A top-level Start Menu shortcut is created at `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Flow.lnk`.
- `Get-StartApps` reports Flow with AppID `com.flow.focus`.
- Windows Search uses that Start Menu shortcut, so `Win` then typing `Flow` opens the app. No separate Windows Search registration is needed.
- Apps and Features shows Flow with publisher Flow and the uninstaller path.
- No desktop shortcut is created.
- The app data stays in `%APPDATA%\com.flow.focus`; the uninstaller keeps it unless the user selects the delete-data checkbox.
- The NSIS installer is unsigned. Windows may show a publisher or SmartScreen warning until a code-signing certificate is added.

## Styling stack

The spec lists Tailwind, and Tailwind v4 is installed through `@tailwindcss/vite`. Most of the visual system is hand-written CSS in `src/index.css`: paper surfaces, custom keyframes, and responsive rules. React components use semantic class names instead of long utility strings. There is no `tailwind.config` file; v4 is imported at the top of the stylesheet.

The v4 direction is a clean, light studio style: warm paper backgrounds, dark green ink, restrained gold and rust accents, Plus Jakarta Sans, a fixed left rail, and flat card surfaces with thin borders. The six nav items are Daily, Plan, Do, Try, Dashboard, and Settings.

Daily and Plan are single-column pages. The old notes panel and its two-column workspace are gone from the UI.

## Do friction

Leaving Do requires holding `Shift+Esc` for 1.5 seconds. The hold has a visible progress bar and a cancel-on-release behavior. The explicit "Plan" action remains for pointer and assistive-technology users.

The keyboard shortcuts stay active only on the Do screen. Space, D, E, S, N, and B ignore key events from inputs, text areas, selects, buttons, and editable content.

## Undo

Undo is a global `Ctrl+Z` or `Cmd+Z` shortcut. It skips inputs, dialogs, and contenteditable elements. It covers done, waiting, someday, dropped, planned, and unplanned events. The core refuses to undo recurrence changes and any change older than 10 minutes. The UI reports "Nothing to undo" rather than pretending the command succeeded.

## Security and CSP

The production CSP is:

```text
default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost
```

`style-src 'unsafe-inline'` is needed for React inline styles and the progress bar. Fonts and images are bundled locally. `connect-src` is limited to Tauri IPC. There are no remote scripts, remote fonts, analytics, or network requests.

## Testing and generated types

- Acceptance tests use an in-memory SQLite database and a `FixedClock`, so no test depends on the wall clock or the network.
- `ts-rs` exports bindings during `cargo test -p flow-core --lib`. Changing a Rust IPC type requires a full, unfiltered run of that command. A filtered test run can leave `src/generated` stale.
- `core/tests/carry.rs` covers daily carry, the cascade, reorder, and Plan carry. The other acceptance suites are `daily.rs`, `habits.rs`, `horizons.rs`, `try_shelf.rs`, `sessions.rs`, `planning.rs`, `lifecycle.rs`, `tasks.rs`, `safety.rs`, `performance.rs`, `property.rs`, `calibration.rs`, and `wrapup.rs`.
- Dashboard unit tests live in `core/src/rules/dashboard.rs`. Cadence coverage lives in `core/src/rules/cadence.rs`. The notes module and its unit tests remain in the core without an active UI.
- Browser QA uses the WebView2 CDP endpoint on port 9222. The app is launched with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
- Settings QA must read the input `value`. The visible test ID for the budget is `setting-budget`; it is not named after the database column.
- In the completion dialog, the skip control sets no manual time. Clicking skip alone does not close the dialog; press Done to submit.
