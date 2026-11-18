import { useState, type FormEvent } from "react";
import { ArrowRight } from "@phosphor-icons/react";
import { useFlow } from "../store";
import { Modal } from "./ui";

export function CaptureDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [title, setTitle] = useState("");
  const [saving, setSaving] = useState(false);

  if (!open) {
    return null;
  }

  const close = () => {
    setTitle("");
    setSaving(false);
    onClose();
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const trimmed = title.trim();
    if (!trimmed || saving) {
      return;
    }
    setSaving(true);
    const task = await act<{ id: string }>("capture_task", { title: trimmed });
    setSaving(false);
    if (task) {
      notify("Inbox", "success", "Captured. Make sense of it when you plan.");
      close();
    }
  };

  return (
    <Modal
      kicker="Quick capture"
      title="What just came up?"
      onClose={close}
      footer={
        <>
          <span className="inline-note left">
            It lands in your inbox, not on today's plan.
          </span>
          <button
            type="submit"
            form="capture-form"
            className="btn btn-primary"
            data-testid="capture-submit"
            disabled={!title.trim() || saving}
          >
            Capture <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <form id="capture-form" onSubmit={submit}>
        <input
          className="quick-input"
          data-testid="capture-title"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          placeholder="The thing you do not want to forget"
          maxLength={240}
          autoComplete="off"
        />
        <div className="quick-hint">
          <span>Title only. Clarify it later.</span>
          <span>
            <span className="kbd">Enter</span> to capture
          </span>
        </div>
      </form>
    </Modal>
  );
}
