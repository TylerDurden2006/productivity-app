import { useEffect, useState, type FormEvent } from "react";
import { ArrowRight, Tray } from "@phosphor-icons/react";
import { useFlow } from "../store";
import { Modal } from "./ui";

export function OverrideDialog() {
  const override = useFlow((state) => state.override);
  const resolveOverride = useFlow((state) => state.resolveOverride);
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const overrideKey = override
    ? `${override.action}:${override.error.code}`
    : null;

  useEffect(() => {
    setReason("");
    setError(null);
  }, [overrideKey]);

  if (!override) {
    return null;
  }

  const isReadyFull = override.error.code === "READY_FULL";
  const title = isReadyFull
    ? "Ready is at the cap"
    : override.error.code === "BUDGET_EXCEEDED"
      ? "Past today's focus budget"
        : override.error.code === "STUCK_TASK"
          ? "This task keeps rolling over"
          : override.error.code === "BOULDER"
            ? "That task is bigger than two hours"
            : "The plan is locked";

  const cancel = () => {
    void resolveOverride(null);
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = reason.trim();
    if (trimmed.length < 3) {
      setError("Write at least a few words so future-you knows why.");
      return;
    }
    void resolveOverride({ type: "reason", reason: trimmed });
  };

  return (
    <Modal
      kicker="Soft gate"
      title={title}
      onClose={cancel}
      footer={
        <>
          <button type="button" className="btn btn-ghost" onClick={cancel}>
            Cancel
          </button>
          {isReadyFull ? (
            <button
              type="button"
              className="btn btn-primary"
              data-testid="override-someday"
              onClick={() => void resolveOverride({ type: "someday" })}
            >
              Send to Someday <Tray size={14} />
            </button>
          ) : (
            <button
              type="submit"
              form="override-form"
              className="btn btn-primary"
              data-testid="override-submit"
            >
              Continue with reason <ArrowRight size={14} />
            </button>
          )}
        </>
      }
    >
      {isReadyFull ? (
        <>
          <p className="modal-note">{override.error.message}</p>
          <p className="modal-note">
            The simplest way through is to let this task wait in Someday and
            review it in two weeks. You can also cancel, then finish or drop
            something already in Ready.
          </p>
        </>
      ) : (
        <form id="override-form" onSubmit={submit}>
          <p className="modal-note">{override.error.message}</p>
          {override.error.hint ? (
            <p className="inline-note" style={{ marginBottom: 12 }}>
              {override.error.hint}
            </p>
          ) : null}
          <label className="field">
            <span className="field-label">
              <span>Reason</span>
              <span className="field-hint">Written to the task history</span>
            </span>
            <textarea
              className="text-area"
              data-testid="override-reason"
              value={reason}
              onChange={(event) => {
                setReason(event.target.value);
                setError(null);
              }}
              placeholder="The afternoon call ran long, and this still matters today."
              autoFocus
            />
          </label>
          {error ? <div className="form-error">{error}</div> : null}
        </form>
      )}
    </Modal>
  );
}

