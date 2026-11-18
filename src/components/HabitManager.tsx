import { useState } from "react";
import { PencilSimple, Plus, Repeat, Trash } from "@phosphor-icons/react";
import type { Cadence } from "../generated/Cadence";
import type { Habit } from "../generated/Habit";
import type { HabitInput } from "../generated/HabitInput";
import type { TimeOfDay } from "../generated/TimeOfDay";
import { logicalTodayISO } from "../lib/format";
import { useFlow } from "../store";
import { Field, Modal } from "./ui";
import { ScheduleBuilder } from "./ScheduleBuilder";

const blankSchedule = (rolloverHour = 0): Cadence => ({
  kind: "DAILY",
  anchor_on: logicalTodayISO(rolloverHour),
  interval_n: 1,
  weekdays_mask: 127,
  month_day: null,
  month: null,
  nth_weekday: null,
  weekday: null,
  dates: [],
  quota_count: null,
  quota_period: null,
});

const blankHabit = (rolloverHour = 0): HabitInput => ({
  title: "",
  why: null,
  cue: null,
  area_id: null,
  schedule: blankSchedule(rolloverHour),
  target_count: 1,
  unit_label: null,
  time_of_day: "ANYTIME" as TimeOfDay,
  sort_order: 0,
  starts_on: null,
  ends_on: null,
});

export function HabitManager({ onClose }: { onClose: () => void }) {
  const habits = useFlow((state) => state.habits);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const refreshHabits = useFlow((state) => state.refreshHabits);
  const [editing, setEditing] = useState<Habit | "new" | null>(null);
  const settings = useFlow((state) => state.settings);
  const [form, setForm] = useState<HabitInput>(() =>
    blankHabit(settings?.day_rollover_hour ?? 0),
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const beginNew = () => {
    setForm(blankHabit(settings?.day_rollover_hour ?? 0));
    setEditing("new");
    setError(null);
  };

  const beginEdit = (habit: Habit) => {
    setForm({
      title: habit.title,
      why: habit.why,
      cue: habit.cue,
      area_id: habit.area_id,
      schedule: habit.schedule,
      target_count: habit.target_count,
      unit_label: habit.unit_label,
      time_of_day: habit.time_of_day,
      sort_order: habit.sort_order,
      starts_on: habit.starts_on,
      ends_on: habit.ends_on,
    });
    setEditing(habit);
    setError(null);
  };

  const save = async () => {
    if (!form.title.trim()) {
      setError("Give the habit a name.");
      return;
    }
    setSaving(true);
    setError(null);
    const input: HabitInput = {
      ...form,
      title: form.title.trim(),
      schedule: {
        ...form.schedule,
        anchor_on:
          form.schedule.anchor_on || logicalTodayISO(settings?.day_rollover_hour ?? 0),
      },
    };
    const result =
      editing === "new"
        ? await act<Habit>("create_habit", { input })
        : editing
          ? await act<Habit>("update_habit", { id: editing.id, input })
          : null;
    setSaving(false);
    if (result) {
      notify(editing === "new" ? "Habit added." : "Habit updated.", "success");
      await refreshHabits();
      setEditing(null);
    }
  };

  const archive = async (habit: Habit) => {
    const result = await act<unknown>("archive_habit", { id: habit.id });
    if (result) {
      notify("Habit archived.", "success");
      await refreshHabits();
      if (editing !== "new" && editing?.id === habit.id) {
        setEditing(null);
      }
    }
  };

  return (
    <Modal
      kicker="Rituals"
      title="Your habits"
      wide
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">
            {editing === "new" ? "New habit" : editing ? "Editing habit" : `${habits.length} habits`}
          </span>
          {editing ? (
            <>
              <button type="button" className="btn btn-ghost" onClick={() => setEditing(null)}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={saving}
                onClick={() => void save()}
              >
                {saving ? "Saving." : "Save habit"}
              </button>
            </>
          ) : (
            <button type="button" className="btn btn-primary" onClick={beginNew}>
              <Plus size={14} /> New habit
            </button>
          )}
        </>
      }
    >
      {editing ? (
        <div className="habit-editor">
          {error ? <div className="form-error">{error}</div> : null}
          <div className="form-grid">
            <Field label="Habit">
              <input
                className="input"
                value={form.title}
                onChange={(event) => setForm({ ...form, title: event.target.value })}
                placeholder="Morning walk"
              />
            </Field>
            <Field label="Why" hint="Optional">
              <input
                className="input"
                value={form.why ?? ""}
                onChange={(event) =>
                  setForm({ ...form, why: event.target.value || null })
                }
                placeholder="To clear my head"
              />
            </Field>
            <Field label="Cue" hint="Optional">
              <input
                className="input"
                value={form.cue ?? ""}
                onChange={(event) =>
                  setForm({ ...form, cue: event.target.value || null })
                }
                placeholder="After coffee"
              />
            </Field>
            <Field label="Time of day">
              <select
                className="input"
                value={form.time_of_day}
                onChange={(event) =>
                  setForm({ ...form, time_of_day: event.target.value as TimeOfDay })
                }
              >
                <option value="ANYTIME">Anytime</option>
                <option value="MORNING">Morning</option>
                <option value="AFTERNOON">Afternoon</option>
                <option value="EVENING">Evening</option>
              </select>
            </Field>
            <Field label="Target count">
              <input
                className="input"
                type="number"
                min={1}
                max={100}
                value={form.target_count}
                onChange={(event) =>
                  setForm({ ...form, target_count: Number(event.target.value) || 1 })
                }
              />
            </Field>
            <Field label="Unit" hint="Optional">
              <input
                className="input"
                value={form.unit_label ?? ""}
                onChange={(event) =>
                  setForm({ ...form, unit_label: event.target.value || null })
                }
                placeholder="pages, glasses, minutes"
              />
            </Field>
          </div>
          <div className="schedule-section">
            <div className="kicker">Schedule</div>
            <ScheduleBuilder
              value={form.schedule}
              onChange={(schedule) => setForm({ ...form, schedule })}
            />
          </div>
        </div>
      ) : (
        <div className="habit-manager-list">
          {habits.length === 0 ? (
            <div className="empty-inline">
              <Repeat size={18} />
              <div>
                <strong>No habits yet.</strong>
                <p>Add the repeated things that make the day feel like yours.</p>
              </div>
            </div>
          ) : (
            habits.map((habit) => (
              <div className="habit-manager-row" key={habit.id}>
                <div className="habit-manager-main">
                  <div className="habit-manager-title">
                    {habit.title}
                    {!habit.active ? <span className="tiny-label">Archived</span> : null}
                  </div>
                  <div className="task-meta">
                    {habit.time_of_day.toLowerCase()} · {habit.schedule.kind.toLowerCase().replaceAll("_", " ")}
                  </div>
                </div>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label={`Edit ${habit.title}`}
                  onClick={() => beginEdit(habit)}
                >
                  <PencilSimple size={15} />
                </button>
                <button
                  type="button"
                  className="icon-btn danger"
                  aria-label={`Archive ${habit.title}`}
                  onClick={() => void archive(habit)}
                >
                  <Trash size={15} />
                </button>
              </div>
            ))
          )}
        </div>
      )}
    </Modal>
  );
}
