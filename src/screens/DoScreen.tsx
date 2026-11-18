import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowRight,
  Coffee,
  LockSimple,
  Pause,
  Play,
  Plus,
  SkipForward,
  Stop,
} from "@phosphor-icons/react";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type { Task } from "../generated/Task";
import { CompleteTaskDialog, StopSessionDialog } from "../components/SessionDialogs";
import { TriageDialog } from "../components/TriageDialog";
import { Modal } from "../components/ui";
import { formatMins, formatTimer, slotLabel } from "../lib/format";
import { useFlow } from "../store";

export function DoScreen() {
  const doView = useFlow((state) => state.doView);
  const mode = useFlow((state) => state.mode);
  const plan = useFlow((state) => state.plan);
  const act = useFlow((state) => state.act);
  const notify = useFlow((state) => state.notify);
  const navigate = useFlow((state) => state.navigate);
  const setCaptureOpen = useFlow((state) => state.setCaptureOpen);
  const focusTaskId = useFlow((state) => state.focusTaskId);
  const override = useFlow((state) => state.override);
  const settings = useFlow((state) => state.settings);
  const [now, setNow] = useState(Date.now());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [completeOpen, setCompleteOpen] = useState(false);
  const [stopOpen, setStopOpen] = useState(false);
  const [blockedTask, setBlockedTask] = useState<Task | null>(null);
  const [expiryOpen, setExpiryOpen] = useState(false);
  const [expiryKey, setExpiryKey] = useState<string | null>(null);
  const [breakOpen, setBreakOpen] = useState(false);
  const [breakKey, setBreakKey] = useState<string | null>(null);
  const [holdOpen, setHoldOpen] = useState(false);
  const holdTimer = useRef<number | null>(null);

  const session = doView?.session ?? null;
  const all = useMemo(() => {
    if (!doView) {
      return [];
    }
    return doView.current ? [doView.current, ...doView.others] : doView.others;
  }, [doView]);

  const selected =
    (session ? doView?.current : null) ??
    all.find((entry) => entry.task.id === selectedId) ??
    doView?.current ??
    null;

  useEffect(() => {
    if (!session?.running) {
      return;
    }
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [session?.running]);

  useEffect(() => {
    if (focusTaskId) {
      setSelectedId(focusTaskId);
    }
  }, [focusTaskId]);

  useEffect(() => {
    if (selectedId && !all.some((entry) => entry.task.id === selectedId)) {
      setSelectedId(null);
    }
  }, [all, selectedId]);

  const plannedSecs = session
    ? session.planned_mins * 60
    : (selected?.task.estimate_mins ?? 0) * 60;
  const runningSecs =
    session?.running && session.running_since
      ? Math.max(0, (now - new Date(session.running_since).getTime()) / 1000)
      : 0;
  const elapsedSecs = (session?.elapsed_secs ?? 0) + runningSecs;
  const remainingSecs = plannedSecs - elapsedSecs;
  const over = session !== null && plannedSecs > 0 && remainingSecs < 0;
  const progress =
    plannedSecs > 0 ? Math.min(100, (elapsedSecs / plannedSecs) * 100) : 0;
  const nextReady =
    plan?.deep_candidates[0] ?? plan?.quick_candidates[0] ?? null;

  const pullOneMore = async () => {
    if (!nextReady) {
      return;
    }
    const result = await act<Task>("pull_extra", { id: nextReady.id });
    if (result) {
      notify("One more.", "success", "It joined today as Extra.");
    }
  };

  useEffect(() => {
    if (!session) {
      setExpiryKey(null);
      setExpiryOpen(false);
      return;
    }
    const key = `${session.id}:${session.extended}`;
    if (plannedSecs > 0 && elapsedSecs >= plannedSecs && expiryKey !== key) {
      setExpiryKey(key);
      setExpiryOpen(true);
      setBreakOpen(false);
      chime();
      if (settings?.notify_block_end !== false) void sendBlockNotice(
        session.kind === "FOCUS" ? "Focus block finished" : "Batch block finished",
        "Finish, take one extension, or stop and leave a breadcrumb.",
      );
    }
  }, [elapsedSecs, expiryKey, plannedSecs, session]);

  useEffect(() => {
    if (!session) {
      setBreakOpen(false);
      return;
    }
    if (
      session.kind !== "FOCUS" ||
      session.extended ||
      elapsedSecs < 3600 ||
      breakKey === session.id
    ) {
      return;
    }
    setBreakKey(session.id);
    setBreakOpen(true);
    chime();
    if (settings?.notify_block_end !== false) void sendBlockNotice(
      "An hour at the desk",
      "Stand up, drink water, and come back in fifteen minutes.",
    );
  }, [breakKey, elapsedSecs, session]);

  const start = async () => {
    if (!selected) {
      return;
    }
    const result = await act<unknown>("start_session", {
      kind: "FOCUS",
      taskId: selected.task.id,
    });
    if (result) {
      setNow(Date.now());
      void ensureNotificationPermission();
    }
  };

  const pauseForBreak = async () => {
    setBreakOpen(false);
    if (session?.running) {
      const result = await act<unknown>("pause_session", {});
      if (result) {
        setNow(Date.now());
      }
    }
  };

  const togglePause = async () => {
    if (!session) {
      await start();
      return;
    }
    const result = session.running
      ? await act<unknown>("pause_session", {})
      : await act<unknown>("resume_session", {});
    if (result) {
      setNow(Date.now());
    }
  };

  const openComplete = () => {
    if (!selected) {
      return;
    }
    if (session || selected.has_session_today) {
      void complete();
      return;
    }
    if (selected.task.estimate_mins === 15) {
      void complete();
      return;
    }
    setCompleteOpen(true);
  };

  const complete = async () => {
    if (!selected) {
      return;
    }
    setExpiryOpen(false);
    const result = await act<Task>("complete_task", {
      id: selected.task.id,
      manualMins: null,
    });
    if (result) {
      notify("Done.", "success");
      setSelectedId(null);
    }
  };

  const extend = async () => {
    const result = await act<unknown>("extend_session", {});
    if (result) {
      notify("Fifteen minutes added.", "success", "Once per focus session.");
      setExpiryOpen(false);
      setNow(Date.now());
    }
  };

  const stop = async () => {
    setExpiryOpen(false);
    setStopOpen(true);
  };

  const cycleNext = () => {
    if (!all.length) {
      return;
    }
    if (session) {
      notify("Stop the timer before switching.", "warning");
      return;
    }
    const index = all.findIndex((entry) => entry.task.id === selected?.task.id);
    const next = all[(index + 1) % all.length];
    setSelectedId(next.task.id);
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        override ||
        completeOpen ||
        stopOpen ||
        blockedTask ||
        expiryOpen ||
        breakOpen ||
        target?.closest("input, textarea, select, button, [contenteditable='true']")
      ) {
        return;
      }
      const key = event.key.toLowerCase();
      if (event.code === "Space") {
        event.preventDefault();
        void togglePause();
      } else if (key === "d") {
        event.preventDefault();
        openComplete();
      } else if (key === "e") {
        event.preventDefault();
        if (session?.kind === "FOCUS" && !session.extended) {
          void extend();
        } else {
          notify("No extension here.", "info", "Batch sessions and used extensions cannot grow.");
        }
      } else if (key === "s") {
        event.preventDefault();
        if (session) {
          void stop();
        }
      } else if (key === "n") {
        event.preventDefault();
        cycleNext();
      } else if (key === "b") {
        event.preventDefault();
        if (selected) {
          setBlockedTask(selected.task);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  useEffect(() => {
    const busy =
      override ||
      completeOpen ||
      stopOpen ||
      blockedTask !== null ||
      expiryOpen ||
      breakOpen;
    const clearHold = () => {
      if (holdTimer.current !== null) {
        window.clearTimeout(holdTimer.current);
        holdTimer.current = null;
      }
      setHoldOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "Escape" ||
        !event.shiftKey ||
        event.repeat ||
        busy ||
        holdTimer.current !== null
      ) {
        return;
      }
      const target = event.target as HTMLElement | null;
      if (
        target?.closest("input, textarea, select, button, [contenteditable='true']")
      ) {
        return;
      }
      event.preventDefault();
      setHoldOpen(true);
      holdTimer.current = window.setTimeout(() => {
        holdTimer.current = null;
        setHoldOpen(false);
        navigate("plan");
      }, 1500);
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Escape" || event.key === "Shift") {
        clearHold();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", clearHold);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", clearHold);
      clearHold();
    };
  }, [override, completeOpen, stopOpen, blockedTask, expiryOpen, breakOpen, navigate]);

  const sessionReady = Boolean(session);
  const timerSeconds = plannedSecs > 0 ? Math.abs(remainingSecs) : null;

  return (
    <div className="do-page" data-testid="do-page">
      <header className="do-topbar">
        <div className="do-brand">
          <span className="do-brand-dot" />
          Flow
        </div>
        <span className="do-topbar-label">Do</span>
        <div className="do-topbar-spacer" />
        <div className="do-topbar-stats">
          <span>
            <strong data-testid="do-remaining-count">
              {doView?.remaining_count ?? 0}
            </strong>{" "}
            planned left
          </span>
          <span>
            <strong data-testid="do-done-count">{doView?.done_count ?? 0}</strong>{" "}
            done today
          </span>
        </div>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          data-testid="do-capture"
          onClick={() => setCaptureOpen(true)}
        >
          <Plus size={13} /> Add task
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          data-testid="do-open-plan"
          onClick={() => navigate("plan")}
        >
          Plan
        </button>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          data-testid="do-close-day"
          onClick={() => navigate("daily")}
        >
          Back to Daily <ArrowRight size={13} />
        </button>
      </header>

      <div className="do-stage">
        {!selected ? (
          <div className="do-empty" data-testid="do-empty">
            <div className="do-mode" style={{ marginBottom: 25 }}>
              Nothing on the desk
            </div>
            {mode === "PLAN" ? (
              <>
                <h1>There is no plan for today yet.</h1>
                <p>
                  Add one task that matters, and the useful middle, on Daily.
                  This screen will hold only the first thing.
                </p>
                <div className="do-empty-actions">
                  <button
                    type="button"
                    className="btn btn-primary btn-lg"
                    onClick={() => navigate("daily")}
                  >
                    Open Daily
                  </button>
                </div>
              </>
            ) : (
              <>
                <h1>
                  {(doView?.done_count ?? 0) > 0
                    ? "Everything on the page is done."
                    : "The plan is empty. The day is still yours."}
                </h1>
                <p>
                  {(doView?.done_count ?? 0) > 0
                    ? "The desk is clear. Pull in one more from Ready, or close the day while it is still fresh."
                    : "There are no tasks on this page yet. Add one on Daily when you are ready."}
                </p>
                {nextReady ? (
                  <div className="do-more">
                    <div>
                      <span>One more from Ready</span>
                      <strong>{nextReady.title}</strong>
                    </div>
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm"
                      data-testid="do-pull-extra"
                      onClick={() => void pullOneMore()}
                    >
                      Pull it in
                    </button>
                  </div>
                ) : null}
                <div className="do-empty-actions">
                  <button
                    type="button"
                    className="btn btn-primary btn-lg"
                    data-testid="do-empty-open-daily"
                    onClick={() => navigate("daily")}
                  >
                    Open Daily
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary btn-lg"
                    data-testid="do-empty-open-plan"
                    onClick={() => navigate("plan")}
                  >
                    Open the plan
                  </button>
                </div>
              </>
            )}
          </div>
        ) : (
          <>
            <div className="do-mode" data-testid="do-slot">
              {slotLabel(selected.slot)}
              <span className="do-value" aria-label={`Value ${selected.task.value} of 5`}>
                {[1, 2, 3, 4, 5].map((value) => (
                  <span
                    key={value}
                    className="do-value-bar"
                    data-on={value <= selected.task.value}
                  />
                ))}
              </span>
            </div>

            <div className="do-inner">
              {selected.task.breadcrumb ? (
                <div className="do-resume" data-testid="do-breadcrumb">
                  Resume here: {selected.task.breadcrumb}
                </div>
              ) : null}
              <h1 className="do-title" data-testid="do-task-title">
                {selected.task.title}
              </h1>

              <div className="do-first-step" data-testid="do-first-step">
                <span className="do-step-label">First step</span>
                {selected.task.first_step ?? "No first step yet. Edit this task before starting."}
              </div>

              {selected.task.done_when ? (
                <div className="do-step">
                  <span className="do-step-label">Done when</span>
                  {selected.task.done_when}
                </div>
              ) : null}

              {selected.task.cue ? (
                <div className="do-cue" data-testid="do-cue">
                  "{selected.task.cue}"
                </div>
              ) : null}

              <div className="timer-block">
                <div
                  className="timer"
                  data-running={session?.running ?? false}
                  data-testid="do-timer"
                >
                  {timerSeconds === null ? "-:-" : formatTimer(timerSeconds)}
                </div>
                <div className="timer-sub" data-testid="do-timer-sub">
                  {session
                    ? over
                      ? "over the block"
                      : remainingSecs > 0
                        ? "remaining"
                        : "ready to finish"
                    : `${formatMins(selected.task.estimate_mins)} planned`}
                  {session?.extended ? " · extended" : ""}
                  {session && !session.running ? " · paused" : ""}
                </div>
                <div className="timer-track">
                  <div
                    className="timer-fill"
                    data-over={over}
                    data-testid="do-timer-progress"
                    style={{ width: `${over ? 100 : progress}%` }}
                  />
                </div>
              </div>

              <div className="do-controls">
                <button
                  type="button"
                  className="btn btn-primary btn-lg"
                  data-testid="do-start-pause"
                  onClick={() => void togglePause()}
                >
                  {session ? (
                    session.running ? (
                      <>
                        <Pause size={15} weight="fill" /> Pause
                      </>
                    ) : (
                      <>
                        <Play size={15} weight="fill" /> Resume
                      </>
                    )
                  ) : (
                    <>
                      <Play size={15} weight="fill" /> Start focus
                    </>
                  )}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-lg"
                  data-testid="do-done"
                  onClick={openComplete}
                >
                  Done
                </button>
                {session?.kind === "FOCUS" && !session.extended ? (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    data-testid="do-extend"
                    onClick={() => void extend()}
                  >
                    <Plus size={13} /> 15 min
                  </button>
                ) : null}
                {session ? (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    data-testid="do-stop"
                    onClick={() => void stop()}
                  >
                    <Stop size={14} /> Stop for now
                  </button>
                ) : null}
                <div className="do-divider" />
                <button
                  type="button"
                  className="btn btn-ghost"
                  data-testid="do-blocked"
                  onClick={() => setBlockedTask(selected.task)}
                >
                  <Coffee size={14} /> Blocked
                </button>
                {all.length > 1 && !session ? (
                  <button
                    type="button"
                    className="btn btn-ghost"
                    data-testid="do-next"
                    onClick={cycleNext}
                  >
                    <SkipForward size={14} /> Next
                  </button>
                ) : null}
              </div>

              <div className="do-keys">
                <span>
                  <span className="kbd">Space</span> start / pause
                </span>
                <span>
                  <span className="kbd">D</span> done
                </span>
                <span>
                  <span className="kbd">E</span> +15
                </span>
                <span>
                  <span className="kbd">S</span> stop
                </span>
                <span>
                  <span className="kbd">N</span> something else
                </span>
                <span>
                  <span className="kbd">B</span> blocked
                </span>
              </div>

              <div className="do-footer">
                <span>
                  <LockSimple size={10} style={{ verticalAlign: "-1px" }} /> One
                  thing at a time
                </span>
                <button
                  type="button"
                  data-testid="do-edit-plan"
                  onClick={() => navigate("plan")}
                >
                  {sessionReady ? "Plan is here" : "Edit plan"}
                </button>
              </div>
            </div>
          </>
        )}
      </div>

      {holdOpen ? (
        <div className="hold-leave" data-testid="do-hold-leave">
          <div className="hold-leave-card">
            <div className="kicker">Leaving Do</div>
            <div className="hold-leave-title">Hold Shift + Esc to open the plan.</div>
            <div className="hold-leave-track">
              <span className="hold-leave-fill" />
            </div>
            <p className="inline-note">Release either key to stay here.</p>
          </div>
        </div>
      ) : null}
      {completeOpen && selected ? (
        <CompleteTaskDialog
          taskId={selected.task.id}
          taskTitle={selected.task.title}
          onClose={() => setCompleteOpen(false)}
        />
      ) : null}
      {stopOpen ? (
        <StopSessionDialog
          onClose={() => setStopOpen(false)}
          onStopped={() => setNow(Date.now())}
        />
      ) : null}
      {blockedTask ? (
        <TriageDialog
          task={blockedTask}
          initial="WAITING"
          onResolved={() => {
            if (session) {
              void act<unknown>("stop_session", { breadcrumb: null });
            }
          }}
          onClose={() => {
            setBlockedTask(null);
            setSelectedId(null);
            setNow(Date.now());
          }}
        />
      ) : null}
      {breakOpen && session ? (
        <Modal
          kicker="An hour in"
          title="Step away for fifteen."
          onClose={() => setBreakOpen(false)}
          footer={
            <>
              <button
                type="button"
                className="btn btn-ghost"
                data-testid="do-break-skip"
                onClick={() => setBreakOpen(false)}
              >
                Skip
              </button>
              <button
                type="button"
                className="btn btn-primary"
                data-testid="do-break-pause"
                onClick={() => void pauseForBreak()}
              >
                Pause for a break
              </button>
            </>
          }
        >
          <p className="modal-note">
            You have been in this focus block for an hour. A short break now is
            cheaper than a slow hour later.
          </p>
        </Modal>
      ) : null}
      {expiryOpen && session ? (
        <BlockEnded
          focus={session.kind === "FOCUS"}
          extended={session.extended}
          onClose={() => setExpiryOpen(false)}
          onDone={complete}
          onExtend={extend}
          onStop={stop}
        />
      ) : null}
    </div>
  );
}

function BlockEnded({
  focus,
  extended,
  onClose,
  onDone,
  onExtend,
  onStop,
}: {
  focus: boolean;
  extended: boolean;
  onClose: () => void;
  onDone: () => Promise<void>;
  onExtend: () => Promise<void>;
  onStop: () => Promise<void>;
}) {
  const canExtend = focus && !extended;
  return (
    <Modal
      kicker="The block ended"
      title={canExtend ? "That is the block." : "The clock is over."}
      onClose={onClose}
      footer={
        <>
          <button
            type="button"
            className="btn btn-ghost"
            data-testid="do-expiry-stop"
            onClick={onStop}
          >
            Stop for now
          </button>
          {canExtend ? (
            <button
              type="button"
              className="btn btn-secondary"
              data-testid="do-expiry-extend"
              onClick={() => void onExtend()}
            >
              +15 minutes
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-primary"
            data-testid="do-expiry-done"
            onClick={() => void onDone()}
          >
            Done <ArrowRight size={14} />
          </button>
        </>
      }
    >
      <p className="modal-note">
        {canExtend
          ? "You planned this much. Finish, take one extension, or stop and leave a breadcrumb."
          : "There is no more time to add. Finish what is in front of you, or stop and leave a trail for next time."}
      </p>
      <p className="inline-note">
        The clock keeps running until you choose. This nudge is not a lock.
      </p>
    </Modal>
  );
}

function chime() {
  try {
    const context = new AudioContext();
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = 660;
    gain.gain.setValueAtTime(0.0001, context.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.12, context.currentTime + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, context.currentTime + 0.55);
    oscillator.connect(gain);
    gain.connect(context.destination);
    oscillator.start();
    oscillator.stop(context.currentTime + 0.6);
    window.setTimeout(() => void context.close(), 900);
  } catch {
    return;
  }
}

async function ensureNotificationPermission(): Promise<boolean> {
  try {
    if (await isPermissionGranted()) {
      return true;
    }
    return (await requestPermission()) === "granted";
  } catch {
    return false;
  }
}

async function sendBlockNotice(title: string, body: string) {
  if (!(await ensureNotificationPermission())) {
    return;
  }
  try {
    sendNotification({ title, body });
  } catch {
    return;
  }
}



