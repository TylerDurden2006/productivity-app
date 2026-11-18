import { useState, type FormEvent } from "react";
import { ArrowRight, Plus, Trash } from "@phosphor-icons/react";
import type { OutcomeKind } from "../generated/OutcomeKind";
import type { SplitChild } from "../generated/SplitChild";
import type { Task } from "../generated/Task";
import { useFlow } from "../store";
import { addDays, logicalTodayISO } from "../lib/format";
import { Field, Modal } from "./ui";

const estimates = [15, 30, 60, 90, 120];

const outcomeCopy: Record<
  OutcomeKind,
  { kicker: string; title: string; note: string; action: string }
> = {
  CARRY: {
    kicker: "Unfinished",
    title: "Carry it forward",
    note: "It leaves today's plan and returns to Ready. The rollover count goes up.",
    action: "Carry forward",
  },
  WAITING: {
    kicker: "Unfinished",
    title: "Wait on someone",
    note: "Park it with a person or thing and a date to look again.",
    action: "Put on waiting",
  },
  SOMEDAY: {
    kicker: "Unfinished",
    title: "Move to someday",
    note: "Take it out of the active list until the review date.",
    action: "Move to someday",
  },
  DROP: {
    kicker: "Unfinished",
    title: "Let it go",
    note: "Dropping is a real decision. The task stays in history, not in your way.",
    action: "Drop it",
  },
  SPLIT: {
    kicker: "Too big",
    title: "Break it into pieces",
    note: "The parent closes. Each child becomes a Ready task with its own first step.",
    action: "Split task",
  },
  SHRINK: {
    kicker: "Keeps rolling",
    title: "Make it smaller",
    note: "Give it a new first step and an estimate no larger than it had.",
    action: "Shrink it",
  },
};

export function OutcomeDialog({
  task,
  initial,
  onClose,
}: {
  task: Task;
  initial: OutcomeKind;
  onClose: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const settings = useFlow((state) => state.settings);
  const today = logicalTodayISO(settings?.day_rollover_hour ?? 0);
  const [outcome, setOutcome] = useState<OutcomeKind>(initial);
  const [waitingOn, setWaitingOn] = useState(task.waiting_on ?? "");
  const [reviewOn, setReviewOn] = useState(
    task.review_on ?? addDays(today, initial === "SOMEDAY" ? 14 : 7),
  );
  const [breadcrumb, setBreadcrumb] = useState(task.breadcrumb ?? "");
  const [firstStep, setFirstStep] = useState(task.first_step ?? "");
  const [estimate, setEstimate] = useState<number | null>(
    task.estimate_mins !== null && task.estimate_mins <= 30
      ? task.estimate_mins
      : 30,
  );
  const [children, setChildren] = useState<SplitChild[]>([
    { title: `${task.title} — part one`, first_step: "", estimate_mins: 30 },
    { title: `${task.title} — part two`, first_step: "", estimate_mins: 30 },
  ]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const copy = outcomeCopy[outcome];

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    if (outcome === "WAITING" && !waitingOn.trim()) {
      setError("Name who or what you are waiting on.");
      return;
    }
    if (outcome === "SHRINK" && !firstStep.trim()) {
      setError("The smaller version still needs a first step.");
      return;
    }
    if (outcome === "SHRINK" && estimate === null) {
      setError("Pick an estimate.");
      return;
    }
    if (outcome === "SPLIT") {
      if (children.length < 2) {
        setError("Keep at least two pieces.");
        return;
      }
      if (children.some((child) => !child.title.trim() || !child.first_step.trim())) {
        setError("Every piece needs a title and a first step.");
        return;
      }
    }

    setSaving(true);
    const input = {
      outcome,
      waiting_on: outcome === "WAITING" ? waitingOn.trim() : null,
      review_on:
        outcome === "WAITING" || outcome === "SOMEDAY" ? reviewOn : null,
      breadcrumb: breadcrumb.trim() || null,
      children:
        outcome === "SPLIT"
          ? children.map((child) => ({
              title: child.title.trim(),
              first_step: child.first_step.trim(),
              estimate_mins: child.estimate_mins,
            }))
          : null,
      first_step: outcome === "SHRINK" ? firstStep.trim() : null,
      estimate_mins: outcome === "SHRINK" ? estimate : null,
    };
    const result = await act<unknown>("resolve_task", {
      id: task.id,
      input,
    });
    setSaving(false);
    if (result !== null) {
      notify("Task resolved.", "success");
      onClose();
    }
  };

  return (
    <Modal
      kicker={copy.kicker}
      title={copy.title}
      onClose={onClose}
      wide={outcome === "SPLIT"}
      footer={
        <>
          <span className="inline-note left">{task.title}</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="submit"
            form="outcome-form"
            className="btn btn-primary"
            data-testid="outcome-submit"
            disabled={saving}
          >
            {copy.action} <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="outcome-form" onSubmit={submit}>
        <p className="modal-note">{copy.note}</p>
        {error ? <div className="form-error">{error}</div> : null}

        {outcome === "CARRY" ? (
          <div className="banner" data-tone="forest">
            <div>
              <div className="banner-title">Rollover {task.rollover_count + 1}</div>
              <div className="banner-body">
                At three rollovers, Flow stops putting this on the plan until you
                shrink, split, wait, or drop it.
              </div>
            </div>
          </div>
        ) : null}

        {outcome === "WAITING" || outcome === "SOMEDAY" ? (
          <div className="form-grid">
            {outcome === "WAITING" ? (
              <Field label="Waiting on" hint="Person, reply, event">
                <input
                  className="input"
                  data-testid="outcome-waiting-on"
                  value={waitingOn}
                  onChange={(event) => setWaitingOn(event.target.value)}
                  placeholder="Sam's reply"
                  maxLength={240}
                />
              </Field>
            ) : null}
            <Field label="Look again on">
              <input
                className="input"
                type="date"
                data-testid="outcome-review-on"
                value={reviewOn}
                min={today}
                onChange={(event) => setReviewOn(event.target.value)}
              />
            </Field>
          </div>
        ) : null}

        <div style={{ marginTop: 14 }}>
          <Field
            label={outcome === "DROP" ? "What are you letting go of?" : "Breadcrumb"}
            hint="Optional"
          >
            <textarea
              className="text-area"
              data-testid="outcome-breadcrumb"
              value={breadcrumb}
              onChange={(event) => setBreadcrumb(event.target.value)}
              placeholder={
                outcome === "DROP"
                  ? "Not worth the season I am in."
                  : outcome === "CARRY"
                    ? "Where you got to, so tomorrow is not a cold start."
                    : "What made this hard?"
              }
              style={{ minHeight: 62 }}
            />
          </Field>
        </div>

        {outcome === "SHRINK" ? (
          <div style={{ marginTop: 14 }}>
            <Field label="New first step">
              <input
                className="input"
                data-testid="outcome-first-step"
                value={firstStep}
                onChange={(event) => setFirstStep(event.target.value)}
                maxLength={240}
              />
            </Field>
            <div className="section-rule">New estimate</div>
            <div className="seg">
              {estimates
                .filter(
                  (option) =>
                    task.estimate_mins === null || option <= task.estimate_mins,
                )
                .map((option) => (
                  <button
                    key={option}
                    type="button"
                    className="seg-btn"
                    data-testid={`outcome-estimate-${option}`}
                    data-active={estimate === option}
                    onClick={() => setEstimate(option)}
                  >
                    {option}m
                  </button>
                ))}
            </div>
          </div>
        ) : null}

        {outcome === "SPLIT" ? (
          <div style={{ marginTop: 14 }}>
            <div className="section-rule">Pieces</div>
            {children.map((child, index) => (
              <div
                key={index}
                className="form-grid"
                style={{ marginBottom: 13, alignItems: "end" }}
              >
                <Field label={`Piece ${index + 1} title`}>
                  <input
                    className="input"
                    data-testid={`outcome-piece-title-${index}`}
                    value={child.title}
                    onChange={(event) => {
                      const next = [...children];
                      next[index] = { ...child, title: event.target.value };
                      setChildren(next);
                    }}
                    maxLength={240}
                  />
                </Field>
                <Field label="First step">
                  <div style={{ display: "flex", gap: 4 }}>
                    <input
                      className="input"
                      data-testid={`outcome-piece-step-${index}`}
                      value={child.first_step}
                      onChange={(event) => {
                        const next = [...children];
                        next[index] = {
                          ...child,
                          first_step: event.target.value,
                        };
                        setChildren(next);
                      }}
                      maxLength={240}
                    />
                    <select
                      className="select"
                      value={child.estimate_mins}
                      onChange={(event) => {
                        const next = [...children];
                        next[index] = {
                          ...child,
                          estimate_mins: Number(event.target.value),
                        };
                        setChildren(next);
                      }}
                      aria-label={`Estimate for piece ${index + 1}`}
                    >
                      {estimates.map((option) => (
                        <option key={option} value={option}>
                          {option}m
                        </option>
                      ))}
                    </select>
                    {children.length > 2 ? (
                      <button
                        type="button"
                        className="icon-btn danger"
                        aria-label={`Remove piece ${index + 1}`}
                        onClick={() =>
                          setChildren(children.filter((_, i) => i !== index))
                        }
                      >
                        <Trash size={14} />
                      </button>
                    ) : null}
                  </div>
                </Field>
              </div>
            ))}
            {children.length < 5 ? (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() =>
                  setChildren([
                    ...children,
                    { title: "", first_step: "", estimate_mins: 30 },
                  ])
                }
              >
                <Plus size={13} /> Add a piece
              </button>
            ) : null}
          </div>
        ) : null}

        {outcome === "CARRY" ? (
          <div className="seg" style={{ marginTop: 16 }}>
            <span className="inline-note">
              Logged today:{" "}
              {task.planned_for ? "planned, not finished" : "not on today's plan"}
            </span>
          </div>
        ) : null}

        <div className="section-rule">Change the outcome</div>
        <div className="seg">
          {(["CARRY", "SHRINK", "SPLIT", "WAITING", "SOMEDAY", "DROP"] as OutcomeKind[]).map(
            (option) => (
              <button
                key={option}
                type="button"
                className="seg-btn"
                data-testid={`outcome-kind-${option}`}
                data-active={outcome === option}
                onClick={() => {
                  setOutcome(option);
                  if (option === "SOMEDAY") {
                    setReviewOn(task.review_on ?? addDays(today, 14));
                  }
                  if (option === "WAITING") {
                    setReviewOn(task.review_on ?? addDays(today, 7));
                  }
                }}
              >
                {option.toLowerCase()}
              </button>
            ),
          )}
        </div>
      </form>
    </Modal>
  );
}
