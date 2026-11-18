import {
  ArrowRight,
  Coffee,
  PushPin,
  Scissors,
  ArrowsInLineVertical,
} from "@phosphor-icons/react";
import type { OutcomeKind } from "../generated/OutcomeKind";
import type { Task } from "../generated/Task";
import type { TriageTarget } from "../generated/TriageTarget";
import { Modal } from "./ui";

export function TaskActionsDialog({
  task,
  onClose,
  onOutcome,
  onTriage,
  onPin,
}: {
  task: Task;
  onClose: () => void;
  onOutcome: (outcome: OutcomeKind) => void;
  onTriage: (target: TriageTarget) => void;
  onPin: () => void;
}) {
  return (
    <Modal
      kicker="Task actions"
      title={task.title}
      onClose={onClose}
      footer={
        <>
          <span className="inline-note left">Change the task without changing the day.</span>
          <button type="button" className="btn btn-ghost" onClick={onClose}>
            Close
          </button>
        </>
      }
    >
      <div className="task-action-list" data-testid="task-actions-dialog">
        <button type="button" onClick={onPin}>
          <PushPin size={16} />
          <span>
            <strong>{task.pinned ? "Unpin task" : "Pin task"}</strong>
            <small>Keep it near the top of its next plan.</small>
          </span>
          <ArrowRight size={14} />
        </button>
        <button type="button" onClick={() => onOutcome("SPLIT")}>
          <Scissors size={16} />
          <span>
            <strong>Split into smaller tasks</strong>
            <small>Give a too-large task a narrower shape.</small>
          </span>
          <ArrowRight size={14} />
        </button>
        <button type="button" onClick={() => onOutcome("SHRINK")}>
          <ArrowsInLineVertical size={16} />
          <span>
            <strong>Shrink the first step</strong>
            <small>Lower the estimate and reset the carry count.</small>
          </span>
          <ArrowRight size={14} />
        </button>
        <button type="button" onClick={() => onTriage("WAITING")}>
          <Coffee size={16} />
          <span>
            <strong>Move to Waiting</strong>
            <small>Park it until a person or thing moves.</small>
          </span>
          <ArrowRight size={14} />
        </button>
        <button type="button" onClick={() => onTriage("DROPPED")}>
          <Scissors size={16} />
          <span>
            <strong>Drop the task</strong>
            <small>Let it leave the plan and keep the history.</small>
          </span>
          <ArrowRight size={14} />
        </button>
      </div>
    </Modal>
  );
}