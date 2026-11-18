import { useEffect, useMemo, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  CalendarBlank,
  Check,
  CheckCircle,
  Circle,
  DotsSixVertical,
  LinkSimple,
  Plus,
  Tray,
} from "@phosphor-icons/react";
import type { Band } from "../generated/Band";
import type { GoalView } from "../generated/GoalView";
import type { HorizonKind } from "../generated/HorizonKind";
import type { Task } from "../generated/Task";
import type { TriageTarget } from "../generated/TriageTarget";
import { ClarifyDialog } from "../components/ClarifyDialog";
import { GoalDialog } from "../components/GoalDialog";
import { GoalTasksDialog } from "../components/GoalTasksDialog";

import { TriageDialog } from "../components/TriageDialog";
import { formatDayMonth, formatLongDay, formatMins } from "../lib/format";
import { useFlow } from "../store";

function shiftPeriod(kind: HorizonKind, start: string | null, delta: number): string | null {
  if (!start || kind === "LONG_TERM") {
    return null;
  }
  const date = new Date(`${start}T00:00:00Z`);
  if (kind === "WEEK") {
    date.setUTCDate(date.getUTCDate() + 7 * delta);
  } else if (kind === "MONTH") {
    date.setUTCMonth(date.getUTCMonth() + delta);
  } else if (kind === "QUARTER") {
    date.setUTCMonth(date.getUTCMonth() + 3 * delta);
  } else if (kind === "YEAR") {
    date.setUTCFullYear(date.getUTCFullYear() + delta);
  }
  return date.toISOString().slice(0, 10);
}

const tabs: Array<{ kind: HorizonKind; label: string }> = [
  { kind: "WEEK", label: "Week" },
  { kind: "MONTH", label: "Month" },
  { kind: "QUARTER", label: "Quarter" },
  { kind: "YEAR", label: "Year" },
  { kind: "LONG_TERM", label: "Long term" },
];

export function HorizonsScreen() {
  const horizon = useFlow((state) => state.horizon);
  const plan = useFlow((state) => state.plan);
  const kind = useFlow((state) => state.horizonKind);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const refreshHorizon = useFlow((state) => state.refreshHorizon);
  const [goalBand, setGoalBand] = useState<Band | null>(null);
  const [linkGoalId, setLinkGoalId] = useState<string | null>(null);
  const [moreOpen, setMoreOpen] = useState(
    (horizon?.bands.more.length ?? 0) > 0,
  );
  const [backlogOpen, setBacklogOpen] = useState(false);
  const [clarifyTask, setClarifyTask] = useState<Task | null>(null);
  const [triageTask, setTriageTask] = useState<{
    task: Task;
    initial: TriageTarget;
  } | null>(null);

  useEffect(() => {
    if (!horizon) {
      void refreshHorizon();
    }
  }, [horizon, refreshHorizon]);

  const linkGoal = useMemo(() => {
    if (!linkGoalId || !horizon) {
      return null;
    }
    const all = [
      ...(horizon.bands.focus ? [horizon.bands.focus] : []),
      ...horizon.bands.secondary,
      ...horizon.bands.commitment,
      ...horizon.bands.more,
      ...horizon.completed,
    ];
    return all.find((goal) => goal.goal.id === linkGoalId) ?? null;
  }, [horizon, linkGoalId]);

  const moveGoal = async (goalId: string, band: Band, index: number) => {
    if (!horizon) {
      return;
    }
    const result = await act<unknown>("reorder_goal", {
      kind: horizon.kind,
      periodStart: horizon.period_start ?? "",
      id: goalId,
      targetBand: band,
      targetIndex: index,
    });
    if (result) {
      notify("Order updated.", "success");
    }
  };

  const switchKind = (next: HorizonKind) => {
    setMoreOpen(false);
    void refreshHorizon(next, null);
  };

  const bands = useMemo(
    () => [
      {
        band: "FOCUS" as Band,
        label: "1",
        title: "Main",
        goals: horizon?.bands.focus ? [horizon.bands.focus] : [],
      },
      {
        band: "SECONDARY" as Band,
        label: "3",
        title: "Next",
        goals: horizon?.bands.secondary ?? [],
      },
      {
        band: "COMMITMENT" as Band,
        label: "5",
        title: "Small",
        goals: horizon?.bands.commitment ?? [],
      },
    ],
    [horizon],
  );

  if (!horizon) {
    return (
      <div className="page">
        <div className="skeleton skeleton-title" />
        <div className="skeleton skeleton-line" style={{ width: "62%" }} />
        <div className="skeleton skeleton-block" />
      </div>
    );
  }

  const complete = async (goal: GoalView) => {
    const done = goal.goal.status === "DONE";
    const result = await act<unknown>(
      done ? "reopen_goal" : "complete_goal",
      { id: goal.goal.id },
    );
    if (result) {
      notify(done ? "Goal reopened." : "Goal complete.", "success");
    }
  };

  return (
    <div
      className="page horizons-page"
      data-testid="horizons-page"
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
      <div className="page-head horizon-head">
        <div>
          <div className="kicker">Planning</div>
          <h1 className="page-title">
            Plan the <em>whole.</em>
          </h1>
          <p className="page-sub">
            The same 1-3-5-More shape, at every scale. Add the work, then check it off.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => setGoalBand("FOCUS")}
        >
          <Plus size={15} /> Add task
        </button>
      </div>

      <div className="horizon-tabs" role="tablist" aria-label="Planning horizon">
        {tabs.map((tab) => (
          <button
            key={tab.kind}
            type="button"
            role="tab"
            aria-selected={kind === tab.kind}
            data-active={kind === tab.kind}
            onClick={() => switchKind(tab.kind)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="horizon-period">
        <div>
          <div className="kicker">{kind.replaceAll("_", " ")}</div>
          <h2 className="period-title">{horizon.label}</h2>
        </div>
        <div className="period-nav">
          {kind !== "LONG_TERM" ? (
            <button
              type="button"
              className="icon-btn"
              aria-label="Previous period"
              onClick={() =>
                void refreshHorizon(kind, shiftPeriod(kind, horizon.period_start, -1))
              }
            >
              <ArrowLeft size={15} />
            </button>
          ) : null}
          <div className="period-meta">
            <CalendarBlank size={15} />
            {horizon.period_start && horizon.period_end
              ? `${formatLongDay(horizon.period_start)} to ${formatDayMonth(horizon.period_end)}`
              : "No end date"}
          </div>
          {kind !== "LONG_TERM" ? (
            <button
              type="button"
              className="icon-btn"
              aria-label="Next period"
              onClick={() =>
                void refreshHorizon(kind, shiftPeriod(kind, horizon.period_start, 1))
              }
            >
              <ArrowRight size={15} />
            </button>
          ) : null}
        </div>
      </div>

      <div className="goal-bands">
        {bands.map((band) => (
          <section className="goal-band" key={band.band}>
            <div className="goal-band-head">
              <span className="band-number">{band.label}</span>
              <h3>{band.title}</h3>
              <span className="band-count">{band.goals.length}</span>
              <button
                type="button"
                className="icon-btn subtle"
                aria-label={`Add ${band.title}`}
                onClick={() => setGoalBand(band.band)}
              >
                <Plus size={14} />
              </button>
            </div>
            <div
              className="goal-list"
              onDragOver={(event) => {
                event.preventDefault();
                event.dataTransfer.dropEffect = "move";
              }}
              onDrop={(event) => {
                event.preventDefault();
                const id = event.dataTransfer.getData("text/plain");
                if (id) {
                  void moveGoal(id, band.band, band.goals.length);
                }
              }}
            >
              {band.goals.length === 0 ? (
                <button
                  type="button"
                  className="empty-row"
                  onClick={() => setGoalBand(band.band)}
                >
                  Add a task for this {kind === "LONG_TERM" ? "direction" : kind.toLowerCase()}.
                </button>
              ) : (
                band.goals.map((goal, index) => (
                  <GoalRow
                    key={goal.goal.id}
                    goal={goal}
                    band={band.band}
                    index={index}
                    onMove={moveGoal}
                    onComplete={complete}
                    onLink={(item) => setLinkGoalId(item.goal.id)}
                  />
                ))
              )}
            </div>
          </section>
        ))}

        <section className="goal-band more-band">
          <button
            type="button"
            className="goal-band-head goal-band-toggle"
            onClick={() => setMoreOpen((current) => !current)}
          >
            <span className="band-number">More</span>
            <h3>Overflow</h3>
            <span className="band-count">{horizon.bands.more.length}</span>
            <ArrowRight size={15} className={moreOpen ? "rotated" : ""} />
          </button>
          {moreOpen ? (
            <div
              className="goal-list"
              onDragOver={(event) => {
                event.preventDefault();
                event.dataTransfer.dropEffect = "move";
              }}
              onDrop={(event) => {
                event.preventDefault();
                const id = event.dataTransfer.getData("text/plain");
                if (id) {
                  void moveGoal(id, "MORE", horizon.bands.more.length);
                }
              }}
            >
              {horizon.bands.more.length === 0 ? (
                <div className="band-empty">No overflow tasks.</div>
              ) : (
                horizon.bands.more.map((goal, index) => (
                  <GoalRow
                    key={goal.goal.id}
                    goal={goal}
                    band="MORE"
                    index={index}
                    onMove={moveGoal}
                    onComplete={complete}
                    onLink={(item) => setLinkGoalId(item.goal.id)}
                  />
                ))
              )}
              <button
                type="button"
                className="quiet-link add-more"
                onClick={() => setGoalBand("MORE")}
              >
                <Plus size={13} /> Add to More
              </button>
            </div>
          ) : null}
        </section>
      </div>

      {horizon.completed.length > 0 ? (
        <section className="completed-block">
          <div className="section-head">
            <div>
              <div className="kicker">Done</div>
              <h2 className="section-title">Completed this period</h2>
            </div>
            <span className="tiny-label">{horizon.completed.length}</span>
          </div>
          <div className="completed-list">
            {horizon.completed.map((goal, index) => (
              <GoalRow
                key={goal.goal.id}
                goal={goal}
                band={goal.goal.band}
                index={index}
                onMove={moveGoal}
                onComplete={complete}
                onLink={(item) => setLinkGoalId(item.goal.id)}
              />
            ))}
          </div>
        </section>
      ) : null}

      {horizon.days.length > 0 ? (
        <section className="week-strip">
          <div className="section-head">
            <div>
              <div className="kicker">The week</div>
              <h2 className="section-title">Daily shape</h2>
            </div>
            <span className="tiny-label">Open tasks</span>
          </div>
          <div className="week-days">
            {horizon.days.map((day) => (
              <div className="week-day" key={day.day}>
                <span>{formatDayMonth(day.day)}</span>
                <strong>{day.open_count}</strong>
                <span className="tiny-label">{day.done_count} done</span>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      <section className="backlog-block">
        <button
          type="button"
          className="section-head backlog-toggle"
          onClick={() => setBacklogOpen((current) => !current)}
        >
          <div>
            <div className="kicker">Backlog</div>
            <h2 className="section-title">
              Inbox and Ready{plan?.inbox.length ? ` (${plan.inbox.length})` : ""}
            </h2>
          </div>
          <span className="period-meta">
            {plan?.ready_count ?? 0} of {plan?.ready_cap ?? 0} Ready
            <ArrowRight size={15} className={backlogOpen ? "rotated" : ""} />
          </span>
        </button>
        {backlogOpen ? (
          <div className="backlog-list">
            {plan?.inbox.length ? (
              plan.inbox.map((task) => (
                <div className="task-row" key={task.id}>
                  <div className="task-main">
                    <span className="task-title">{task.title}</span>
                    <span className="task-meta">Inbox</span>
                  </div>
                  <div className="task-actions">
                    <button
                      type="button"
                      className="quiet-link"
                      onClick={() => setClarifyTask(task)}
                    >
                      Clarify
                    </button>
                    <button
                      type="button"
                      className="quiet-link"
                      onClick={() => setTriageTask({ task, initial: "SOMEDAY" })}
                    >
                      Someday
                    </button>
                  </div>
                </div>
              ))
            ) : (
              <div className="empty-row">
                <Tray size={16} /> The inbox is clear.
              </div>
            )}
            <div className="backlog-foot">
              Ready tasks can be added from Daily with Add task.
            </div>
          </div>
        ) : null}
      </section>

      <div className="daily-foot">
        <span>Habits and today's tasks live on Daily.</span>
        <button
          type="button"
          className="quiet-link"
          onClick={() => useFlow.getState().navigate("daily")}
        >
          Open Daily <ArrowRight size={14} />
        </button>
      </div>

      {clarifyTask ? (
        <ClarifyDialog
          task={clarifyTask}
          readyCount={plan?.ready_count ?? 0}
          readyCap={plan?.ready_cap ?? 0}
          onClose={() => setClarifyTask(null)}
        />
      ) : null}
      {triageTask ? (
        <TriageDialog
          task={triageTask.task}
          initial={triageTask.initial}
          onClose={() => setTriageTask(null)}
        />
      ) : null}
      {goalBand ? (
        <GoalDialog
          kind={kind}
          periodStart={horizon.period_start}
          initialBand={goalBand}
          onClose={() => setGoalBand(null)}
        />
      ) : null}

      {linkGoal ? (
        <GoalTasksDialog goal={linkGoal} onClose={() => setLinkGoalId(null)} />
      ) : null}
    </div>
  );
}

function GoalRow({
  goal,
  band,
  index,
  onMove,
  onComplete,
  onLink,
}: {
  goal: GoalView;
  band: Band;
  index: number;
  onMove: (goalId: string, band: Band, index: number) => Promise<void>;
  onComplete: (goal: GoalView) => Promise<void>;
  onLink: (goal: GoalView) => void;
}) {
  const done = goal.goal.status === "DONE";
  return (
    <div
      className="goal-row"
      data-done={done}
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "move";
      }}
      onDrop={(event) => {
        event.preventDefault();
        const id = event.dataTransfer.getData("text/plain");
        if (id && id !== goal.goal.id) {
          void onMove(id, band, index);
        }
      }}
    >
      {!done ? (
        <span
          className="drag-handle"
          draggable
          title="Drag to reprioritize"
          onDragStart={(event) => {
            event.dataTransfer.setData("text/plain", goal.goal.id);
            event.dataTransfer.effectAllowed = "move";
          }}
        >
          <DotsSixVertical size={15} />
        </span>
      ) : null}
      <button
        type="button"
        className="check-button"
        aria-label={done ? `Reopen ${goal.goal.title}` : `Complete ${goal.goal.title}`}
        onClick={() => void onComplete(goal)}
      >
        {done ? <CheckCircle size={21} weight="fill" /> : <Circle size={21} />}
      </button>
      <div className="goal-main">
        <span className="goal-title">{goal.goal.title}</span>
        <span className="task-meta">
          {goal.linked_tasks
            ? `${goal.done_tasks}/${goal.linked_tasks} linked tasks`
            : "No linked tasks"}
          {goal.goal.estimate_mins ? ` - ${formatMins(goal.goal.estimate_mins)}` : ""}
          {goal.goal.target_on ? ` - due ${formatDayMonth(goal.goal.target_on)}` : ""}
        </span>
        {goal.linked_titles.length > 0 ? (
          <span className="goal-links">{goal.linked_titles.join(" / ")}</span>
        ) : null}
      </div>
      <button
        type="button"
        className="icon-btn subtle"
        aria-label={`Link tasks to ${goal.goal.title}`}
        title="Link tasks"
        data-testid={`goal-link-${goal.goal.id}`}
        onClick={() => onLink(goal)}
      >
        <LinkSimple size={14} />
      </button>
      {done ? <Check size={14} className="done-check" /> : null}
    </div>
  );
}