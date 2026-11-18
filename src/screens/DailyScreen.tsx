import { useMemo, useState } from "react";
import {
  ArrowRight,
  Check,
  CheckCircle,
  Circle,
  Clock,
  GearSix,
  DotsSixVertical,
  DotsThree,
  Plus,
  Repeat,
  SkipForward,
} from "@phosphor-icons/react";
import type { HabitToday } from "../generated/HabitToday";
import type { OutcomeKind } from "../generated/OutcomeKind";
import type { Slot } from "../generated/Slot";
import type { Task } from "../generated/Task";
import type { TriageTarget } from "../generated/TriageTarget";
import { AddTaskDialog } from "../components/AddTaskDialog";
import { HabitManager } from "../components/HabitManager";
import { OutcomeDialog } from "../components/OutcomeDialog";
import { TaskActionsDialog } from "../components/TaskActionsDialog";
import { TriageDialog } from "../components/TriageDialog";

import { formatLongDay } from "../lib/format";
import { useFlow } from "../store";

function isDone(task: Task) {
  return task.status === "DONE";
}

export function DailyScreen() {
  const daily = useFlow((state) => state.daily);
  const plan = useFlow((state) => state.plan);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const navigate = useFlow((state) => state.navigate);
  const setFocusTaskId = useFlow((state) => state.setFocusTaskId);
  const [addOpen, setAddOpen] = useState(false);
  const [habitsOpen, setHabitsOpen] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [actionTask, setActionTask] = useState<Task | null>(null);
  const [outcome, setOutcome] = useState<{ task: Task; initial: OutcomeKind } | null>(null);
  const [triage, setTriage] = useState<{ task: Task; initial: TriageTarget } | null>(null);

  const groups = useMemo(() => {
    const one = daily?.plan.one ? [daily.plan.one] : [];
    return {
      one: one.filter((task) => !isDone(task)),
      three: daily?.plan.three.filter((task) => !isDone(task)) ?? [],
      five: daily?.plan.batch.filter((task) => !isDone(task)) ?? [],
      more: daily?.plan.extra.filter((task) => !isDone(task)) ?? [],
    };
  }, [daily]);
  const completed = daily?.completed ?? [];
  const openCount =
    groups.one.length + groups.three.length + groups.five.length + groups.more.length;

  if (!daily) {
    return (
      <div className="page daily-page">
        <div className="skeleton skeleton-title" />
        <div className="skeleton skeleton-line" style={{ width: "62%" }} />
        <div className="skeleton skeleton-block" />
        <div className="skeleton skeleton-block" />
      </div>
    );
  }

  const startFocus = (task: Task) => {
    setFocusTaskId(task.id);
    navigate("do");
  };

  const moveTask = async (taskId: string, targetSlot: Slot, targetIndex: number) => {
    const result = await act<unknown>("reorder_task", {
      day: daily.day,
      id: taskId,
      targetSlot,
      targetIndex,
    });
    if (result) {
      notify("Order updated.", "success");
    }
  };

  const completeTask = async (task: Task) => {
    const result = await act<Task>("complete_task", {
      id: task.id,
      manualMins: null,
    });
    if (result) {
      notify("Done.", "success", task.title);
    }
  };

  const toggleHabit = async (entry: HabitToday) => {
    const done = entry.entry?.status === "DONE";
    const result = done
      ? await act<unknown>("reopen_habit", { id: entry.habit.id, day: daily.day })
      : await act<unknown>("complete_habit", {
          id: entry.habit.id,
          day: daily.day,
          count: null,
          note: null,
        });
    if (result) {
      notify(done ? "Habit reopened." : "Habit checked off.", "success");
    }
  };

  const pinTask = async (task: Task) => {
    const result = await act<unknown>("set_pinned", {
      id: task.id,
      pinned: !task.pinned,
    });
    if (result) {
      notify(task.pinned ? "Task unpinned." : "Task pinned.", "success");
      setActionTask(null);
    }
  };

  const skipHabit = async (entry: HabitToday) => {
    const result = await act<unknown>("skip_habit", {
      id: entry.habit.id,
      day: daily.day,
      note: null,
    });
    if (result) {
      notify("Habit skipped for today.", "info");
    }
  };

  return (
    <div
      className="page daily-page"
      data-testid="daily-page"
      onDragOver={(event) => {
        if (!event.dataTransfer.types.includes("text/plain")) {
          return;
        }
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }}
      onDrop={(event) => {
        if (event.dataTransfer.types.includes("text/plain")) {
          event.preventDefault();
        }
      }}
    >
      <div className="daily-head">
        <div>
          <div className="kicker">Today</div>
          <h1 className="page-title">
            {formatLongDay(daily.day)} <span className="title-mark">.</span>
          </h1>
          <p className="page-sub">
            Habits first, then the Main task, three Next tasks, five Small tasks,
            and the rest if there is room.
          </p>
        </div>
        <div className="daily-actions">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => setAddOpen(true)}
            data-testid="daily-add-task"
          >
            <Plus size={15} /> Add task
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => {
              navigate("do");
            }}
          >
            Focus mode <ArrowRight size={14} />
          </button>
        </div>
      </div>

      <div className="daily-metrics">
        <div className="daily-metric">
          <span>Habits</span>
          <strong>
            {daily.habit_done} <em>of {daily.habit_total}</em>
          </strong>
        </div>
        <div className="daily-metric">
          <span>Open today</span>
          <strong>{openCount}</strong>
        </div>
        <div className="daily-metric">
          <span>Done today</span>
          <strong>{completed.length}</strong>
        </div>

      </div>

      <section className="rituals-block">
        <div className="section-head">
          <div>
            <div className="kicker">Rituals</div>
            <h2 className="section-title">Today's habits</h2>
          </div>
          <button
            type="button"
            className="quiet-link"
            onClick={() => {
              setHabitsOpen(true);
            }}
          >
            <GearSix size={14} /> Manage habits
          </button>
        </div>
        {daily.habits.length === 0 ? (
          <button
            type="button"
            className="empty-row"
            onClick={() => setHabitsOpen(true)}
          >
            <Repeat size={16} />
            No habits scheduled today. Add a small ritual.
            <Plus size={14} />
          </button>
        ) : (
          <div className="habit-checklist">
            {daily.habits.map((entry) => {
              const done = entry.entry?.status === "DONE";
              const skipped = entry.entry?.status === "SKIPPED";
              return (
                <div
                  className="habit-row"
                  data-done={done}
                  data-skipped={skipped}
                  key={entry.habit.id}
                >
                  <button
                    type="button"
                    className="check-button"
                    aria-label={done ? `Reopen ${entry.habit.title}` : `Complete ${entry.habit.title}`}
                    onClick={() => void toggleHabit(entry)}
                  >
                    {done ? <CheckCircle size={21} weight="fill" /> : <Circle size={21} />}
                  </button>
                  <div className="habit-main">
                    <span className="habit-title">{entry.habit.title}</span>
                    <span className="habit-meta">
                      {entry.cadence_label}
                      {entry.quota_target > 1
                        ? ` · ${entry.quota_done}/${entry.quota_target}`
                        : entry.streak > 0
                          ? ` · ${entry.streak} day streak`
                          : ""}
                    </span>
                  </div>
                  {!done && !skipped ? (
                    <button
                      type="button"
                      className="icon-btn subtle"
                      aria-label={`Skip ${entry.habit.title} today`}
                      onClick={() => void skipHabit(entry)}
                    >
                      <SkipForward size={14} />
                    </button>
                  ) : null}
                  {skipped ? <span className="tiny-label">Skipped</span> : null}
                </div>
              );
            })}
          </div>
        )}
      </section>

      <section className="bands" data-testid="daily-bands">
        <Band
          index="1"
          label="Main"
          note="The one that makes the day count."
          tasks={groups.one}
          empty="Choose the one thing."
          slot="ONE"
          onComplete={completeTask}
          onFocus={startFocus}
          onMove={moveTask}
          onActions={setActionTask}
        />
        <Band
          index="3"
          label="Next"
          note="The work that moves the week."
          tasks={groups.three}
          empty="No next tasks yet."
          slot="THREE"
          onComplete={completeTask}
          onFocus={startFocus}
          onMove={moveTask}
          onActions={setActionTask}
        />
        <Band
          index="5"
          label="Small"
          note="Smaller, bounded, still worth doing."
          tasks={groups.five}
          empty="Nothing in this band."
          slot="BATCH"
          onComplete={completeTask}
          onFocus={startFocus}
          onMove={moveTask}
          onActions={setActionTask}
        />
        <div
          className="band band-more"
          onDragOver={(event) => {
            if (!event.dataTransfer.types.includes("text/plain")) {
              return;
            }
            event.preventDefault();
            event.dataTransfer.dropEffect = "move";
            if (!moreOpen) {
              setMoreOpen(true);
            }
          }}
          onDrop={(event) => {
            if (!event.dataTransfer.types.includes("text/plain")) {
              return;
            }
            event.preventDefault();
            const id = event.dataTransfer.getData("text/plain");
            if (id) {
              void moveTask(id, "EXTRA", groups.more.length);
            }
          }}
        >
          <button
            type="button"
            className="band-head band-toggle"
            onClick={() => setMoreOpen((current) => !current)}
          >
            <span className="band-number">More</span>
            <span className="band-copy">
              <strong>Overflow</strong>
              <span>Lower-priority tasks, if there is room.</span>
            </span>
            <span className="band-count">{groups.more.length}</span>
            <ArrowRight size={16} className={moreOpen ? "rotated" : ""} />
          </button>
          {moreOpen ? (
            <div className="band-list">
              {groups.more.length === 0 ? (
                <div className="band-empty">Nothing waiting in the overflow.</div>
              ) : (
                groups.more.map((task, index) => (
                  <TaskRow
                    key={task.id}
                    task={task}
                    slot="EXTRA"
                    index={index}
                    onMove={moveTask}
                    onComplete={completeTask}
                    onFocus={startFocus}
                    onActions={setActionTask}
                  />
                ))
              )}
            </div>
          ) : null}
        </div>
      </section>

      {completed.length > 0 ? (
        <section className="completed-block">
          <div className="section-head">
            <div>
              <div className="kicker">Done</div>
              <h2 className="section-title">Completed today</h2>
            </div>
            <span className="tiny-label">{completed.length}</span>
          </div>
          <div className="completed-list">
            {completed.map((task) => (
              <div className="completed-row" key={task.id}>
                <Check size={14} />
                <span>{task.title}</span>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <div className="daily-foot">
        <span>
          {plan?.inbox.length
            ? `${plan.inbox.length} in the inbox. Clear one from Plan.`
            : "The plan can change. The list should not lie."}
        </span>
        <button type="button" className="quiet-link" onClick={() => navigate("plan")}>
          Open planning <ArrowRight size={14} />
        </button>
      </div>

      {addOpen ? (
        <AddTaskDialog
          onClose={() => setAddOpen(false)}
          onAdded={(slot) => {
            if (slot === "EXTRA") {
              setMoreOpen(true);
            }
          }}
        />
      ) : null}
      {habitsOpen ? <HabitManager onClose={() => setHabitsOpen(false)} /> : null}
      {actionTask ? (
        <TaskActionsDialog
          task={actionTask}
          onClose={() => setActionTask(null)}
          onOutcome={(initial) => {
            setOutcome({ task: actionTask, initial });
            setActionTask(null);
          }}
          onTriage={(initial) => {
            setTriage({ task: actionTask, initial });
            setActionTask(null);
          }}
          onPin={() => void pinTask(actionTask)}
        />
      ) : null}
      {outcome ? (
        <OutcomeDialog
          task={outcome.task}
          initial={outcome.initial}
          onClose={() => setOutcome(null)}
        />
      ) : null}
      {triage ? (
        <TriageDialog
          task={triage.task}
          initial={triage.initial}
          onClose={() => setTriage(null)}
        />
      ) : null}
    </div>
  );
}

function Band({
  index,
  label,
  note,
  tasks,
  empty,
  slot,
  onComplete,
  onFocus,
  onMove,
  onActions,
}: {
  index: string;
  label: string;
  note: string;
  tasks: Task[];
  empty: string;
  slot: Slot;
  onComplete: (task: Task) => Promise<void>;
  onFocus: (task: Task) => void;
  onMove: (taskId: string, slot: Slot, index: number) => Promise<void>;
  onActions: (task: Task) => void;
}) {
  return (
    <section className="band">
      <div className="band-head">
        <span className="band-number">{index}</span>
        <span className="band-copy">
          <strong>{label}</strong>
          <span>{note}</span>
        </span>
        <span className="band-count">{tasks.length}</span>
      </div>
      <div
        className="band-list"
        onDragOver={(event) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
        }}
        onDrop={(event) => {
          event.preventDefault();
          const id = event.dataTransfer.getData("text/plain");
          if (id) {
            void onMove(id, slot, tasks.length);
          }
        }}
      >
        {tasks.length === 0 ? (
          <div className="band-empty">{empty}</div>
        ) : (
          tasks.map((task, index) => (
            <TaskRow
              key={task.id}
              task={task}
              slot={slot}
              index={index}
              onMove={onMove}
              onComplete={onComplete}
              onFocus={onFocus}
              onActions={onActions}
            />
          ))
        )}
      </div>
    </section>
  );
}

function TaskRow({
  task,
  slot,
  index,
  onMove,
  onComplete,
  onFocus,
  onActions,
}: {
  task: Task;
  slot: Slot;
  index: number;
  onMove: (taskId: string, slot: Slot, index: number) => Promise<void>;
  onComplete: (task: Task) => Promise<void>;
  onFocus: (task: Task) => void;
  onActions: (task: Task) => void;
}) {
  return (
    <div
      className="task-row"
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }}
      onDrop={(event) => {
        event.preventDefault();
        event.stopPropagation();
        const id = event.dataTransfer.getData("text/plain");
        if (id && id !== task.id) {
          void onMove(id, slot, index);
        }
      }}
    >
      <span
        className="drag-handle"
        draggable
        title="Drag to reprioritize"
        onDragStart={(event) => {
          event.dataTransfer.setData("text/plain", task.id);
          event.dataTransfer.effectAllowed = "move";
        }}
      >
        <DotsSixVertical size={15} />
      </span>
      <button
        type="button"
        className="check-button"
        aria-label={`Complete ${task.title}`}
        onClick={() => void onComplete(task)}
      >
        <Circle size={21} />
      </button>
      <div className="task-main">
        <span className="task-title">{task.title}</span>
        <span className="task-meta">
          {task.estimate_mins ? `${task.estimate_mins} min` : "No estimate"}
          {task.first_step ? ` - ${task.first_step}` : ""}
        </span>
      </div>
      <button
        type="button"
        className="icon-btn subtle"
        aria-label={`Focus on ${task.title}`}
        onClick={() => onFocus(task)}
      >
        <Clock size={15} />
      </button>
      <button
        type="button"
        className="icon-btn subtle"
        aria-label={`Actions for ${task.title}`}
        onClick={() => onActions(task)}
      >
        <DotsThree size={16} />
      </button>
    </div>
  );
}