import { useState } from "react";
import type { Band } from "../generated/Band";
import type { Goal } from "../generated/Goal";
import type { GoalInput } from "../generated/GoalInput";
import type { HorizonKind } from "../generated/HorizonKind";
import { useFlow } from "../store";
import { AutoGrowTextarea, Field, Modal } from "./ui";

const bands: Array<{ value: Band; label: string }> = [
  { value: "FOCUS", label: "Main" },
  { value: "SECONDARY", label: "Next" },
  { value: "COMMITMENT", label: "Small" },
  { value: "MORE", label: "More" },
];

export function GoalDialog({
  kind,
  periodStart,
  initialBand = "FOCUS",
  onClose,
}: {
  kind: HorizonKind;
  periodStart: string | null;
  initialBand?: Band;
  onClose: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [success, setSuccess] = useState("");
  const [targetOn, setTargetOn] = useState("");
  const [band, setBand] = useState<Band>(initialBand);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    const cleanTitle = title.trim().replace(/\s+/g, " ");
    if (!cleanTitle) {
      setError("Give this a name.");
      return;
    }
    setSaving(true);
    setError(null);
    const input: GoalInput = {
      horizon_kind: kind,
      period_start: periodStart,
      area_id: null,
      parent_id: null,
      band,
      title: cleanTitle,
      description: description.trim() || null,
      success_definition: success.trim() || null,
      target_on: targetOn || null,
      estimate_mins: null,
    };
    const result = await act<Goal>("create_goal_quick", { input });
    setSaving(false);
    if (result) {
      const label = (value: Band) =>
        bands.find((item) => item.value === value)?.label ?? value;
      notify(
        "Added to the plan.",
        "success",
        result.band === band
          ? undefined
          : `${label(band)} was full, so it went to ${label(result.band)}.`,
      );
      onClose();
    }
  };

  return (
    <Modal
      kicker={kind.toLowerCase().replaceAll("_", " ")}
      title="Add a task"
      large
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">One page, one clear next step.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving}
            onClick={() => void save()}
          >
            {saving ? "Saving." : "Add"}
          </button>
        </>
      }
    >
      {error ? <div className="form-error">{error}</div> : null}
      <div className="form-grid add-task-grid">
        <div className="span-2">
          <Field label="Task">
            <AutoGrowTextarea
              className="task-entry"
              rows={2}
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="Ship the first version"
            />
          </Field>
        </div>
        <Field label="Band">
          <select
            className="input"
            value={band}
            onChange={(event) => setBand(event.target.value as Band)}
          >
            {bands.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Target date" hint="Optional">
          <input
            className="input"
            type="date"
            value={targetOn}
            onChange={(event) => setTargetOn(event.target.value)}
          />
        </Field>
        <div className="span-2">
          <Field label="What does done look like?" hint="Optional">
            <textarea
              className="text-area"
              value={success}
              onChange={(event) => setSuccess(event.target.value)}
              placeholder="The result, not the activity."
            />
          </Field>
        </div>
        <div className="span-2">
          <Field label="Why this matters" hint="Optional">
            <textarea
              className="text-area"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
