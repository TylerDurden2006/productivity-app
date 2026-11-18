import { useMemo, useState } from "react";
import { CheckSquare, Plus } from "@phosphor-icons/react";
import type { GoalView } from "../generated/GoalView";
import type { Task } from "../generated/Task";
import { formatMins } from "../lib/format";
import { useFlow } from "../store";
import { Field, Modal } from "./ui";

export function GoalTasksDialog({
  goal,
  onClose,
}: {
  goal: GoalView;
  onClose: () => void;
}) {
  const plan = useFlow((state) => state.plan);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const [filter, setFilter] = useState("");
  const [pendingId, setPendingId] = useState<string | null>(null);

  const tasksById = useMemo(() => {
    const map = new Map<string, Task>();
    const add = (task: Task | null | undefined) => {
      if (task && !map.has(task.id)) {
        map.set(task.id, task);
      }
    };
    plan?.deep_candidates.forEach(add);
    plan?.quick_candidates.forEach(add);
    add(plan?.plan.one);
    plan?.plan.three.forEach(add);
    plan?.plan.batch.forEach(add);
    plan?.plan.extra.forEach(add);
    return map;
  }, [plan]);

  const linkedIds = goal.linked_task_ids;
  const linkedSet = useMemo(() => new Set(linkedIds), [linkedIds]);

  const linkedRows = linkedIds.map((id, index) => {
    const task = tasksById.get(id);
    return {
      id,
      title: task?.title ?? goal.linked_titles[index] ?? "Linked task",
      estimate_mins: task?.estimate_mins ?? null,
    };
  });

  const availableRows = useMemo(() => {
    const query = filter.trim().toLowerCase();
    return Array.from(tasksById.values()).filter(
      (task) =>
        !linkedSet.has(task.id) &&
        (query === "" || task.title.toLowerCase().includes(query)),
    );
  }, [tasksById, linkedSet, filter]);

  const toggleLink = async (taskId: string, linked: boolean) => {
    setPendingId(taskId);
    const result = await act<unknown>(
      linked ? "unlink_task_goal" : "link_task_goal",
      { taskId, goalId: goal.goal.id },
    );
    setPendingId(null);
    if (result) {
      notify(linked ? "Task unlinked." : "Task linked.", "success");
    }
  };

  return (
    <Modal
      kicker="Linked tasks"
      title={goal.goal.title}
      onClose={onClose}
      wide
      footer={
        <>
          <span className="inline-note left">
            {linkedRows.length} linked · {availableRows.length} available
          </span>
          <button
            type="button"
            className="btn btn-primary"
            data-testid="goal-tasks-close"
            onClick={onClose}
          >
            Done
          </button>
        </>
      }
    >
      <div data-testid="goal-tasks-dialog">
        <div className="section-rule">Linked</div>
        <div className="bank-list">
          {linkedRows.length === 0 ? (
            <div className="empty-row">No tasks linked yet.</div>
          ) : (
            linkedRows.map((row) => (
              <div
                key={row.id}
                className="task-row"
                data-testid={`goal-task-linked-${row.id}`}
              >
                <div className="task-main">
                  <div className="task-title">
                    <CheckSquare
                      size={15}
                      weight="fill"
                      style={{ flex: "none", color: "var(--forest)" }}
                    />
                    <span className="task-title-text">{row.title}</span>
                  </div>
                </div>
                <div className="task-actions">
                  <span className="estimate">
                    {formatMins(row.estimate_mins)}
                  </span>
                  <button
                    type="button"
                    className="quiet-link"
                    disabled={pendingId === row.id}
                    data-testid={`goal-task-unlink-${row.id}`}
                    onClick={() => void toggleLink(row.id, true)}
                  >
                    Unlink
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="section-rule">Available</div>
        <div className="form-grid">
          <Field label="Find a task" hint="Optional">
            <input
              className="input"
              value={filter}
              placeholder="Filter by title"
              data-testid="goal-tasks-filter"
              onChange={(event) => setFilter(event.target.value)}
            />
          </Field>
        </div>
        <div className="bank-list">
          {availableRows.length === 0 ? (
            <div className="empty-row">
              {filter.trim()
                ? "No tasks match that filter."
                : "Every available task is already linked."}
            </div>
          ) : (
            availableRows.map((task) => (
              <button
                key={task.id}
                type="button"
                className="task-row button-row"
                disabled={pendingId === task.id}
                data-testid={`goal-task-available-${task.id}`}
                onClick={() => void toggleLink(task.id, false)}
              >
                <div className="task-main">
                  <div className="task-title">
                    <span className="task-title-text">{task.title}</span>
                  </div>
                </div>
                <div className="task-actions">
                  <span className="estimate">
                    {formatMins(task.estimate_mins)}
                  </span>
                  <Plus size={14} />
                </div>
              </button>
            ))
          )}
        </div>
      </div>
    </Modal>
  );
}
