import { useState, type FormEvent } from "react";
import { ArrowRight } from "@phosphor-icons/react";
import { useFlow } from "../store";
import { Field, Modal } from "./ui";

export function StopSessionDialog({
  onClose,
  onStopped,
}: {
  onClose: () => void;
  onStopped?: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [breadcrumb, setBreadcrumb] = useState("");
  const [saving, setSaving] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const result = await act<unknown>("stop_session", {
      breadcrumb: breadcrumb.trim() || null,
    });
    setSaving(false);
    if (result) {
      notify("Stopped for now.", "success", "The time is saved.");
      onStopped?.();
      onClose();
    }
  };

  return (
    <Modal
      kicker="Stop the clock"
      title="Leave a trail"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Keep going
          </button>
          <button
            type="submit"
            form="stop-session-form"
            className="btn btn-primary"
            data-testid="stop-session-submit"
            disabled={saving}
          >
            Stop for now <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="stop-session-form" onSubmit={submit}>
        <p className="modal-note">
          The elapsed time stays with the task. The note is only needed when you
          have been going for more than ten minutes.
        </p>
        <Field label="Where did you get to?" hint="Optional">
          <textarea
            className="text-area"
            data-testid="stop-session-breadcrumb"
            value={breadcrumb}
            onChange={(event) => setBreadcrumb(event.target.value)}
            placeholder="Drafted the intro, still need the two examples."
          />
        </Field>
      </form>
    </Modal>
  );
}

export function CompleteTaskDialog({
  taskId,
  taskTitle,
  onClose,
}: {
  taskId: string;
  taskTitle: string;
  onClose: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [manualMins, setManualMins] = useState<number | null>(30);
  const [saving, setSaving] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSaving(true);
    const result = await act<unknown>("complete_task", {
      id: taskId,
      manualMins,
    });
    setSaving(false);
    if (result) {
      notify("Done.", "success", "Logged without a timer.");
      onClose();
    }
  };

  return (
    <Modal
      kicker="No timer running"
      title="Mark it done"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="submit"
            form="complete-task-form"
            className="btn btn-primary"
            data-testid="complete-task-submit"
            disabled={saving}
          >
            Done <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="complete-task-form" onSubmit={submit}>
        <p className="modal-note">{taskTitle}</p>
        <Field
          label="How long did it take?"
          hint="Used to calibrate future estimates"
        >
          <div className="seg">
            {[15, 30, 60, 90, 120].map((option) => (
              <button
                key={option}
                type="button"
                className="seg-btn"
                data-testid={`complete-task-mins-${option}`}
                data-active={manualMins === option}
                onClick={() => setManualMins(option)}
              >
                {option}m
              </button>
            ))}
            <button
              type="button"
              className="seg-btn"
              data-testid="complete-task-mins-skip"
              data-active={manualMins === null}
              onClick={() => setManualMins(null)}
            >
              skip
            </button>
          </div>
        </Field>
      </form>
    </Modal>
  );
}

