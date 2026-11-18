import { useState, type FormEvent } from "react";
import { ArrowRight, Lightning } from "@phosphor-icons/react";
import type { Task } from "../generated/Task";
import { useFlow } from "../store";
import { Field, Modal } from "./ui";

const estimates = [15, 30, 60, 90, 120];

export function ClarifyDialog({
  task,
  readyCount,
  readyCap,
  onClose,
  mode = "clarify",
}: {
  task: Task;
  readyCount: number;
  readyCap: number;
  onClose: () => void;
  mode?: "clarify" | "reactivate";
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [title, setTitle] = useState(task.title);
  const [firstStep, setFirstStep] = useState(task.first_step ?? "");
  const [doneWhen, setDoneWhen] = useState(task.done_when ?? "");
  const [estimate, setEstimate] = useState<number | null>(
    task.estimate_mins,
  );
  const [value, setValue] = useState(task.value || 3);
  const [deadline, setDeadline] = useState(task.deadline_on ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!title.trim()) {
      setError("Give it a title.");
      return;
    }
    if (firstStep.trim().length < 3) {
      setError("Name the first physical action, in a few words.");
      return;
    }
    if (estimate === null) {
      setError("Pick an estimate. A rough one is fine.");
      return;
    }
    setError(null);
    setSaving(true);
    const result = await act<Task>(
      mode === "reactivate" ? "reactivate_task" : "clarify_task",
      {
        id: task.id,
        input: {
          title: title.trim(),
          first_step: firstStep.trim(),
          done_when: doneWhen.trim() || null,
          estimate_mins: estimate,
          value,
          deadline_on: deadline || null,
        },
      },
    );
    setSaving(false);
    if (result) {
      notify(
        result.status === "SOMEDAY"
          ? "Moved to Someday."
          : mode === "reactivate"
            ? "Reactivated."
            : "Clarified.",
        "success",
        result.status === "SOMEDAY"
          ? "Ready was full, so it is parked with a review date."
          : "It is Ready and can be planned.",
      );
      onClose();
    }
  };

  return (
    <Modal
      kicker={mode === "reactivate" ? "Back to Ready" : "Inbox to Ready"}
      title={mode === "reactivate" ? "Make it startable again" : "Make it startable"}
      onClose={onClose}
      wide
      footer={
        <>
          <span className="inline-note left">
            {readyCount} / {readyCap} deep tasks ready
          </span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Not now
          </button>
          <button
            type="submit"
            form="clarify-form"
            className="btn btn-primary"
            data-testid="clarify-submit"
            disabled={saving}
          >
            {mode === "reactivate" ? "Reactivate" : "Move to Ready"}{" "}
            <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="clarify-form" onSubmit={submit}>
        {error ? <div className="form-error">{error}</div> : null}
        <div className="form-grid">
          <div className="form-grid cols-1" style={{ gridColumn: "1 / -1" }}>
            {mode === "clarify" ? (
              <Field label="Title">
                <input
                  className="input"
                  data-testid="clarify-title"
                  value={title}
                  onChange={(event) => setTitle(event.target.value)}
                  maxLength={240}
                />
              </Field>
            ) : null}
            <Field
              label="First step"
              hint="Visible, physical, under two minutes"
            >
              <input
                className="input"
                data-testid="clarify-first-step"
                value={firstStep}
                onChange={(event) => setFirstStep(event.target.value)}
                placeholder="Open the doc and write the outline heading"
                maxLength={240}
                autoFocus
              />
            </Field>
          </div>
          <Field label="Done when" hint="Optional">
            <input
              className="input"
              data-testid="clarify-done-when"
              value={doneWhen}
              onChange={(event) => setDoneWhen(event.target.value)}
              maxLength={240}
            />
          </Field>
          <Field label="Deadline" hint="Optional">
            <input
              className="input"
              type="date"
              data-testid="clarify-deadline"
              value={deadline}
              onChange={(event) => setDeadline(event.target.value)}
            />
          </Field>
        </div>

        <div className="section-rule">
          <Lightning size={11} /> Estimate
        </div>
        <div className="seg">
          {estimates.map((option) => (
              <button
                key={option}
                type="button"
                className="seg-btn"
                data-testid={`clarify-estimate-${option}`}
                data-active={estimate === option}
                onClick={() => {
                setEstimate(option);
                setError(null);
              }}
            >
              {option}m
            </button>
          ))}
        </div>

        <div className="section-rule">Value</div>
        <div className="seg">
          {[1, 2, 3, 4, 5].map((option) => (
            <button
              key={option}
              type="button"
              className="seg-btn"
              data-testid={`clarify-value-${option}`}
              data-active={value === option}
              onClick={() => setValue(option)}
            >
              {option}
            </button>
          ))}
        </div>
      </form>
    </Modal>
  );
}

