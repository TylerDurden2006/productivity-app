import { useState, type FormEvent } from "react";
import { ArrowRight } from "@phosphor-icons/react";
import type { Task } from "../generated/Task";
import type { TriageTarget } from "../generated/TriageTarget";
import { addDays, logicalTodayISO } from "../lib/format";
import { useFlow } from "../store";
import { Field, Modal } from "./ui";

const targets: { value: TriageTarget; label: string; note: string }[] = [
  { value: "WAITING", label: "Waiting", note: "Blocked on a person or thing." },
  { value: "SOMEDAY", label: "Someday", note: "Not now, but not never." },
  { value: "DROPPED", label: "Drop", note: "Let it go without pretending." },
];

export function TriageDialog({
  task,
  initial,
  onClose,
  onResolved,
}: {
  task: Task;
  initial: TriageTarget;
  onClose: () => void;
  onResolved?: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const settings = useFlow((state) => state.settings);
  const today = logicalTodayISO(settings?.day_rollover_hour ?? 0);
  const [target, setTarget] = useState<TriageTarget>(initial);
  const [waitingOn, setWaitingOn] = useState(task.waiting_on ?? "");
  const [reviewOn, setReviewOn] = useState(
    task.review_on ?? addDays(today, initial === "SOMEDAY" ? 14 : 7),
  );
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (target === "WAITING" && !waitingOn.trim()) {
      setError("Say who or what you are waiting on.");
      return;
    }
    if (target !== "DROPPED" && !reviewOn) {
      setError("Pick a date to look at it again.");
      return;
    }
    setError(null);
    setSaving(true);
    const result = await act<Task>("triage_task", {
      id: task.id,
      input: {
        to: target,
        waiting_on: target === "WAITING" ? waitingOn.trim() : null,
        review_on: target === "DROPPED" ? null : reviewOn,
      },
    });
    setSaving(false);
    if (result) {
      notify(
        target === "DROPPED" ? "Dropped." : "Moved out of the way.",
        "success",
      );
      onResolved?.();
      onClose();
    }
  };

  return (
    <Modal
      kicker="Triage"
      title={task.title}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="submit"
            form="triage-form"
            className="btn btn-primary"
            data-testid="triage-submit"
            disabled={saving}
          >
            Move it <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="triage-form" onSubmit={submit}>
        {error ? <div className="form-error">{error}</div> : null}
        <div className="seg">
          {targets.map((option) => (
            <button
              key={option.value}
              type="button"
              className="seg-btn"
              data-testid={`triage-target-${option.value}`}
              data-active={target === option.value}
              onClick={() => {
                setTarget(option.value);
                setError(null);
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
        <p className="inline-note" style={{ marginTop: 12 }}>
          {targets.find((option) => option.value === target)?.note}
        </p>

        {target === "WAITING" ? (
          <div style={{ marginTop: 14 }}>
            <Field label="Waiting on" hint="A person, a reply, a delivery">
              <input
                className="input"
                data-testid="triage-waiting-on"
                value={waitingOn}
                onChange={(event) => setWaitingOn(event.target.value)}
                maxLength={200}
                autoFocus
              />
            </Field>
          </div>
        ) : null}

        {target !== "DROPPED" ? (
          <div style={{ marginTop: 14 }}>
            <Field label="Look again on">
              <input
                className="input"
                type="date"
                data-testid="triage-review-on"
                value={reviewOn}
                onChange={(event) => setReviewOn(event.target.value)}
              />
            </Field>
          </div>
        ) : null}
      </form>
    </Modal>
  );
}

