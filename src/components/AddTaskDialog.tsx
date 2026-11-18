import { useState, type FormEvent } from "react";
import type { Band } from "../generated/Band";
import type { Goal } from "../generated/Goal";
import type { HorizonKind } from "../generated/HorizonKind";
import type { QuickAddInput } from "../generated/QuickAddInput";
import type { Slot } from "../generated/Slot";
import type { Task } from "../generated/Task";
import { slotLabel } from "../lib/format";
import { useFlow } from "../store";
import { AutoGrowTextarea, Field, Modal } from "./ui";

type Timeframe = "DAILY" | "INBOX" | HorizonKind;

const bandOptions: Array<{ value: Slot; label: string }> = [
  { value: "ONE", label: "Main" },
  { value: "THREE", label: "Next" },
  { value: "BATCH", label: "Small" },
  { value: "EXTRA", label: "More" },
];

const goalBands: Record<Slot, Band> = {
  ONE: "FOCUS",
  THREE: "SECONDARY",
  BATCH: "COMMITMENT",
  EXTRA: "MORE",
};

const goalBandLabels: Record<Band, string> = {
  FOCUS: "Main",
  SECONDARY: "Next",
  COMMITMENT: "Small",
  MORE: "More",
};

const timeframes: Array<{
  value: Timeframe;
  label: string;
  target: string;
  success: string;
}> = [
  { value: "DAILY", label: "Daily", target: "today's list", success: "today" },
  { value: "WEEK", label: "Weekly", target: "this week's plan", success: "this week" },
  { value: "MONTH", label: "Monthly", target: "this month's plan", success: "this month" },
  { value: "QUARTER", label: "Quarterly", target: "this quarter's plan", success: "this quarter" },
  { value: "YEAR", label: "Yearly", target: "this year's plan", success: "this year" },
  { value: "LONG_TERM", label: "Long term", target: "the long-term plan", success: "long term" },
  { value: "INBOX", label: "Inbox", target: "your inbox", success: "your inbox" },
];

const estimates = [15, 30, 60, 90, 120];

export function AddTaskDialog({
  onClose,
  onAdded,
}: {
  onClose: () => void;
  onAdded?: (slot: Slot) => void;
}) {
  const plan = useFlow((state) => state.plan);
  const daily = useFlow((state) => state.daily);
  const nav = useFlow((state) => state.nav);
  const horizonKind = useFlow((state) => state.horizonKind);
  const horizonPeriod = useFlow((state) => state.horizonPeriod);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const refreshHorizon = useFlow((state) => state.refreshHorizon);
  const navigate = useFlow((state) => state.navigate);
  const [mode, setMode] = useState<"new" | "existing">("new");
  const [existingId, setExistingId] = useState("");
  const [title, setTitle] = useState("");
  const [band, setBand] = useState<Slot>("THREE");
  const [estimate, setEstimate] = useState(30);
  const [timeframe, setTimeframe] = useState<Timeframe>(() =>
    nav === "plan" ? horizonKind : "DAILY",
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const ready = [
    ...(plan?.deep_candidates ?? []),
    ...(plan?.quick_candidates ?? []),
  ];
  const today = daily?.day;
  const selected =
    timeframes.find((item) => item.value === timeframe) ?? timeframes[0];
  const target = mode === "existing" ? "today's list" : selected.target;

  const submit = async (event?: FormEvent) => {
    event?.preventDefault();
    if (saving) {
      return;
    }
    if ((mode === "existing" || timeframe === "DAILY") && !today) {
      setError("Today is not ready yet.");
      return;
    }

    if (mode === "existing") {
      if (!existingId) {
        setError("Choose a task first.");
        return;
      }
      setSaving(true);
      setError(null);
      const result = await act<Task>("assign_task_quick", {
        day: today,
        id: existingId,
        slot: band,
      });
      setSaving(false);
      if (result) {
        const actual = result.slot ?? band;
        onAdded?.(actual);
        notify(
          actual === band ? "Added to the day." : `Added to ${slotLabel(actual)}.`,
          "success",
          actual === band ? undefined : `${slotLabel(band)} was full.`,
        );
        onClose();
      }
      return;
    }

    if (!title.trim()) {
      return;
    }

    setSaving(true);
    setError(null);
    const cleanTitle = title.trim().replace(/\s+/g, " ");

    if (timeframe === "INBOX") {
      const result = await act<Task>("capture_task", { title: cleanTitle });
      setSaving(false);
      if (result) {
        notify("Inbox", "success", "Captured. Make sense of it when you plan.");
        onClose();
      }
      return;
    }

    if (timeframe === "DAILY") {
      if (!today) {
        setError("Today is not ready yet.");
        return;
      }
      const input: QuickAddInput = {
        title: cleanTitle,
        estimate_mins: estimate,
        slot: band,
        day: today,
        value: 3,
      };
      const result = await act<Task>("add_task", { input });
      setSaving(false);
      if (result) {
        const actual = result.slot ?? band;
        onAdded?.(actual);
        notify(
          actual === band ? "Task added to today." : `Task added to ${slotLabel(actual)}.`,
          "success",
          actual === band ? undefined : `${slotLabel(band)} was full.`,
        );
        onClose();
      }
      return;
    }

    const created = await act<Goal>("create_goal_quick", {
      input: {
        horizon_kind: timeframe,
        period_start:
          nav === "plan" && timeframe === horizonKind ? horizonPeriod : null,
        area_id: null,
        parent_id: null,
        band: goalBands[band],
        title: cleanTitle,
        description: null,
        success_definition: null,
        target_on: null,
        estimate_mins: estimate,
      },
    });
    setSaving(false);
    if (created) {
      await refreshHorizon(timeframe, null);
      const requestedBand = goalBands[band];
      notify(
        `Task added to ${selected.success}.`,
        "success",
        created.band === requestedBand
          ? undefined
          : `${goalBandLabels[requestedBand]} was full, so it went to ${goalBandLabels[created.band]}.`,
      );
      onClose();
      navigate("plan");
    }
  };

  return (
    <Modal
      kicker={timeframe === "DAILY" ? "Today" : selected.label}
      title="Add a task"
      large
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">It lands in {target}.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="submit"
            form="add-task-form"
            className="btn btn-primary"
            disabled={saving}
          >
            {saving ? "Adding." : "Add task"}
          </button>
        </>
      }
    >
      {error ? <div className="form-error">{error}</div> : null}
      <div className="segmented">
        <button
          type="button"
          data-active={mode === "new"}
          onClick={() => {
            setMode("new");
            setError(null);
          }}
        >
          New task
        </button>
        <button
          type="button"
          data-active={mode === "existing"}
          onClick={() => {
            setMode("existing");
            setError(null);
          }}
        >
          From Ready
        </button>
      </div>
      <form
        id="add-task-form"
        onSubmit={(event) => {
          event.preventDefault();
          void submit(event);
        }}
      >
        <div className="form-grid add-task-grid">
          {mode === "existing" ? (
            <>
              <div className="span-2">
                <Field label="Ready task">
                  <select
                    className="input"
                    value={existingId}
                    onChange={(event) => setExistingId(event.target.value)}
                  >
                    <option value="">Choose a task</option>
                    {ready.map((task) => (
                      <option key={task.id} value={task.id}>
                        {task.title}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
              <div className="span-2">
                <Field label="Place in">
                  <select
                    className="input"
                    value={band}
                    onChange={(event) => setBand(event.target.value as Slot)}
                  >
                    {bandOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
            </>
          ) : (
            <>
              <div className="span-2">
                <Field label="Task">
                  <AutoGrowTextarea
                    className="task-entry"
                    rows={2}
                    value={title}
                    onChange={(event) => setTitle(event.target.value)}
                    onKeyDown={(event) => {
                      if (
                        event.key === "Enter" &&
                        !event.shiftKey &&
                        !event.repeat
                      ) {
                        event.preventDefault();
                        void submit();
                      }
                    }}
                    placeholder="What needs doing?"
                    autoFocus
                  />
                </Field>
              </div>
              {timeframe !== "INBOX" ? (
                <>
                  <Field label="Estimate">
                    <select
                      className="input"
                      value={estimate}
                      onChange={(event) => setEstimate(Number(event.target.value))}
                    >
                      {estimates.map((value) => (
                        <option key={value} value={value}>
                          {value} min
                        </option>
                      ))}
                    </select>
                  </Field>
                  <Field label="Place in">
                    <select
                      className="input"
                      value={band}
                      onChange={(event) => setBand(event.target.value as Slot)}
                    >
                      {bandOptions.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </Field>
                </>
              ) : null}
              <div className="span-2">
                <Field label="Timeframe">
                  <select
                    className="input"
                    value={timeframe}
                    onChange={(event) => setTimeframe(event.target.value as Timeframe)}
                  >
                    {timeframes.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>
            </>
          )}
        </div>
      </form>
    </Modal>
  );
}
