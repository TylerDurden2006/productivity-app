# Progress

Last updated: 2026-10-24

Flow v4 is implemented in the active desktop UI. The rail has Daily, Plan, Do, Try, Dashboard, and Settings. Review and Patterns are gone. The core, migrations, and tests for automatic carry, the Dashboard, Plan items, and the revised Daily bands are in place. The Markdown notes UI and its packages have been removed.

## Current status

| Area | Status | Notes |
| --- | --- | --- |
| Navigation and shell | Done | Six screens in a light studio style. Single-instance protection focuses the running window. |
| Daily | Done | Due habits first, then Main, Next, Small, and More. Tasks and habits can be checked off. Drag reorder works within and across the main lists and onto a More row. Add task supports new tasks and Ready tasks, and targets today. The page is single-column. |
| Plan | Done | Week, Month, Quarter, Year, and Long term. Plan items use the same Main, Next, Small, and More bands, can be completed and reopened, and can be dragged between bands. Previous and next controls move periods. Items can be linked to daily tasks. Completed this period, the week summary, and the Inbox backlog are present. The page is single-column. |
| Automatic carry | Done | Daily and Plan carry preserve the band, place carried work ahead of the period's tasks, and cascade displaced work through 1, 3, 5, and unlimited. First planned date or period, rollover or carry counts, and daily events are recorded. |
| Live dates | Done | The rail date and dialog defaults use the logical day. A 30-second check reconciles and refreshes the open screen when the logical day changes. |
| Long text | Done | Add task and Plan item dialogs are wider, with full-width auto-growing text fields. Task and Plan titles wrap in full, including long unbroken links. |
| Do | Done | One task at a time with a focus session per task, timer, manual time on completion, stop with breadcrumb, and blocked flow. In the normal PLAN mode, the empty state points back to Daily. The old Ready pull remains behind the legacy DO mode. |
| Dashboard | Done | Day, Week, Month, Quarter, Year, and All ranges. Planned, done, left, carried, overdue, daily and horizon totals, completion, section completion, habit consistency, focus time, most carried, and Try counts. |
| Habits | Done | Separate habit records, cadence engine, habit entries, quotas, streaks, skip, reopen, and archive. |
| Try shelf | Done | Ideas, scheduling with date and day part, unschedule, skip, tried rating and reflection, and status filters including Skipped. |
| Settings | Done | Planning numbers, week start, automatic carry, dashboard default range, block-end notification, nightly planning reminder with time, reduced motion, Advanced Ready cap and multiplier, Flow Home, export, and backup. |
| Flow Home move | Done | Folder picker, validation, copy, restart, cancel, and open folder are implemented. The move was verified by hand. Restore remains manual. |
| Removed surfaces | Done | Review, Patterns, the weekly review setting, the wrap-up wizard, the batch block setting, and the Markdown notes UI are out of the UI. |
| Legacy code | Partial | The wrap-up resolver, BATCH session path, plan lock and suggest commands, several unused components, and the notes table and commands remain in the repository. |

## Removed or legacy

The Markdown notes UI is gone, including its toolbar, preview, autosave, and the Autosave notes setting. `src/components/MarkdownNotes.tsx` was deleted, and `react-markdown`, `remark-gfm`, `rehype-sanitize`, and the dnd-kit packages were uninstalled. Daily and Plan are single-column again. The `horizon_notes` table, notes commands, notes unit tests, and `notes_autosave` setting remain unused for compatibility.

## Test results

The latest project gate passed:

```text
npm run ci    pass (cargo fmt, cargo clippy, 188 tests, npm run build)
```

The run covers the core unit tests and the acceptance suites for carry, Daily, habits, horizons, Try, sessions, planning, lifecycle, tasks, safety, performance, property, calibration, and wrap-up. Dashboard and cadence unit tests run inside the core library. The core library run also exports the TypeScript bindings in `src/generated`.

The current version is 1.0.3. The installers in `target/release/bundle` are 1.0.3 MSI and NSIS packages. Flow is installed for the current Windows user at `%LOCALAPPDATA%\Flow`, with the Start Menu shortcut `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Flow.lnk`, an Apps and Features entry, and no desktop shortcut. `Get-StartApps` reports Flow with AppID `com.flow.focus`. The installed copy reports version 1.0.3. No macOS or Linux installer was built.

## Manual QA

Manual QA ran on the current dev build.

- Dashboard loaded its charts and summary.
- Settings saved.
- The task action dialog opened.
- Dragging a task reordered the Next tasks.
- Dragging a Plan item moved it into Next.
- No console errors appeared.
- The logical-day helper returned the previous date before rollover and the new date at rollover, including a month boundary. The installed 1.0.3 build launched with the current rail date.
- A 195-character task and first step filled the wide Add task dialog without clipping. The same long title plus an unbroken URL wrapped in Daily and Plan, and neither view gained a horizontal scrollbar.

Still to check by hand: carry across a real date boundary, notification delivery, the restore path, a clean-machine installer, and builds on macOS and Linux.

## Known gaps

1. **No in-app restore or reset.** A database restore is still a manual file replacement while Flow is closed. The Reset button on Settings restores unsaved form fields only.
2. **Estimates are limited to preset buckets.** The core validator and the UI accept 15, 30, 60, 90, or 120 minutes. Custom estimates are not exposed.
3. **Recurring tasks still use the older recurrence fields.** The rules read `freq`, `interval_n`, and `weekdays_mask`. No active screen exposes the recurrence editor, and the old recurrence editor component was removed. Habits use the newer cadence engine.
4. **Try scheduling does not create or link a task.** `linked_task_id` and `set_try_item_task` exist, but no screen sets the link.
5. **Habit history and stats have no screen.** `habit_history` and `habit_stats` exist in the API. Daily shows streak and quota progress only.
6. **The old wrap-up core remains for compatibility.** No screen runs the wrap-up flow. No active screen locks the plan, so the legacy DO branch is not normally reachable.
7. **Task assignment for future days is not exposed.** Daily Add task targets today. Plan items are horizon records, not daily tasks, and the core still limits assignment to today or tomorrow.
8. **The batch block column remains in the database.** `batch_block_mins` has no active UI and no role in normal planning or focus. Only the unused BATCH session path uses it for timing.
9. **The Dashboard range caps the daily series at 60 days.** The charts, habit consistency, and focus totals use the capped series. The summary still counts the full range.
10. **No macOS or Linux installer was built.** The bundle contains Windows MSI and NSIS packages only.
11. **Dashboard carried totals are all-time.** The Carried summary and Most carried list are not filtered by the selected range.
12. **Plan items cannot be edited or dropped in the UI.** `update_goal` exists, but there is no edit action and no drop command.
13. **The week strip shows counts only.** It does not render planned minutes or lock state. The Dashboard has no previous or next period control.
14. **Some legacy core commands remain unused.** The plan lock, suggest, swap, and restore commands still exist without an active screen.
15. **The installed Windows copy is current at 1.0.3.**

## Next steps

1. Decide whether to keep the all-time carried totals or filter them by the Dashboard range.
2. Add an in-app restore and reset flow.
3. Add custom task estimates, or keep the preset buckets and say so in the UI.
4. Wire the cadence engine into recurring tasks, or retire the old recurrence fields and editor.
5. Add a Try to task link action.
6. Add a habit history and stats screen.
7. Add future-day assignment if the product needs it on Daily or Plan.
8. Remove or archive the unused legacy core commands.
9. Fix the Dashboard carried range filter and the 60-day labeling, or show the cap in the UI.
10. Run a packaged installer test on a clean Windows machine, then build macOS and Linux packages on their hosts.
11. Test carry across a real date boundary and check notification delivery.

## Documentation

`README.md`, `DECISIONS.md`, and `PROGRESS.md` now describe v4. `docs/flow-v4.md` records the v4 screens, carry rules, Dashboard definitions, settings, data changes, tests, and limits. `docs/flow-v3.md` remains as the prior version record. `docs/flow-spec.md` remains the v2 build specification.
