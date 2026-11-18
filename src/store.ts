import { create } from "zustand";
import {
  getStartup,
  normalizeError,
  runAction,
  type ActionPayload,
  type StartupView,
} from "./api";
import type { AppError } from "./generated/AppError";
import type { DailyView } from "./generated/DailyView";
import type { DashboardView } from "./generated/DashboardView";
import type { DoView } from "./generated/DoView";
import type { Habit } from "./generated/Habit";
import type { HorizonKind } from "./generated/HorizonKind";
import type { HorizonView } from "./generated/HorizonView";
import type { Mode } from "./generated/Mode";
import type { PlanView } from "./generated/PlanView";
import type { Settings } from "./generated/Settings";
import type { TryItem } from "./generated/TryItem";

export type Nav = "daily" | "plan" | "do" | "try" | "dashboard" | "settings";

export type ToastTone = "success" | "error" | "warning" | "info";

export type Toast = {
  id: number;
  title: string;
  detail?: string;
  tone: ToastTone;
};

type OverrideChoice = { type: "reason"; reason: string } | { type: "someday" };

type OverrideState = {
  action: string;
  payload: ActionPayload;
  error: AppError;
  resolve: (value: unknown | null) => void;
};

type FlowState = {
  booted: boolean;
  booting: boolean;
  startup: StartupView | null;
  bootError: string | null;
  nav: Nav;
  mode: Mode;
  planDay: string | null;
  plan: PlanView | null;
  daily: DailyView | null;
  doView: DoView | null;
  dashboard: DashboardView | null;
  dashboardRange: string;
  settings: Settings | null;
  horizonKind: HorizonKind;
  horizonPeriod: string | null;
  horizon: HorizonView | null;
  habits: Habit[];
  tryItems: TryItem[];
  busy: boolean;
  dataError: string | null;
  toasts: Toast[];
  override: OverrideState | null;
  captureOpen: boolean;
  focusTaskId: string | null;
  boot: () => Promise<void>;
  navigate: (nav: Nav, day?: string) => void;
  setCaptureOpen: (open: boolean) => void;
  setFocusTaskId: (id: string | null) => void;
  refreshPlan: (day?: string) => Promise<PlanView | null>;
  refreshDaily: () => Promise<DailyView | null>;
  refreshHorizon: (kind?: HorizonKind, periodStart?: string | null) => Promise<HorizonView | null>;
  refreshHabits: () => Promise<Habit[] | null>;
  refreshTry: () => Promise<TryItem[] | null>;
  refreshDo: () => Promise<DoView | null>;
  refreshDashboard: (range?: string) => Promise<DashboardView | null>;
  refreshSettings: () => Promise<Settings | null>;
  refreshMode: () => Promise<void>;
  refreshAfter: (action: string) => Promise<void>;
  act: <T>(action: string, payload?: ActionPayload) => Promise<T | null>;
  resolveOverride: (choice: OverrideChoice | null) => Promise<void>;
  notify: (title: string, tone?: ToastTone, detail?: string) => void;
  dismissToast: (id: number) => void;
};

let toastId = 0;

const refreshExempt = new Set([
  "export_json",
  "backup_db",
  "validate_database",
  "heartbeat",
  "suggest_plan",
  "get_default_target",
  "get_budget",
]);

export const useFlow = create<FlowState>((set, get) => ({
  booted: false,
  booting: false,
  startup: null,
  bootError: null,
  nav: "daily",
  mode: "PLAN",
  planDay: null,
  plan: null,
  daily: null,
  doView: null,
  dashboard: null,
  dashboardRange: "WEEK",
  settings: null,
  horizonKind: "WEEK",
  horizonPeriod: null,
  horizon: null,
  habits: [],
  tryItems: [],
  busy: false,
  dataError: null,
  toasts: [],
  override: null,
  captureOpen: false,
  focusTaskId: null,

  boot: async () => {
    if (get().booted || get().booting) {
      return;
    }
    set({ booting: true });
    try {
      const startup = await getStartup();
      set({ startup });
      if (!startup.ok) {
        set({ booted: true, booting: false });
        return;
      }
      await Promise.all([
        get().refreshPlan(),
        get().refreshDaily(),
        get().refreshHorizon(),
        get().refreshHabits(),
        get().refreshTry(),
        get().refreshDo(),
        get().refreshSettings(),
        get().refreshMode(),
      ]);
      set({
        booted: true,
        booting: false,
        nav: get().mode === "DO" ? "do" : "daily",
        dashboardRange:
          get().settings?.dashboard_default_range ?? get().dashboardRange,
      });
    } catch (error) {
      set({
        booted: true,
        booting: false,
        bootError: normalizeError(error).message,
      });
    }
  },

  navigate: (nav, day) => {
    set(nav === "plan" ? { nav, planDay: day ?? null } : { nav });
    if (nav === "daily") {
      void get().refreshDaily();
    }
    if (nav === "try") {
      void get().refreshTry();
    }
    if (nav === "dashboard") {
      void get().refreshDashboard();
    }
    if (nav === "settings") {
      void get().refreshSettings();
    }
    if (nav === "plan") {
      void get().refreshPlan();
    }
    if (nav === "do") {
      void get().refreshDo();
    }
  },

  setCaptureOpen: (open) => set({ captureOpen: open }),
  setFocusTaskId: (id) => set({ focusTaskId: id }),

  refreshPlan: async (day) => {
    try {
      const target = day ?? get().planDay ?? undefined;
      const plan = await runAction<PlanView>(
        "get_plan_view",
        target ? { day: target } : {},
      );
      set({ plan, dataError: null });
      return plan;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshDaily: async () => {
    try {
      const daily = await runAction<DailyView>("get_daily_view", {});
      set({ daily, dataError: null });
      return daily;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshHorizon: async (kind, periodStart) => {
    try {
      const nextKind = kind ?? get().horizonKind;
      const nextPeriod =
        periodStart === undefined ? get().horizonPeriod : periodStart;
      const horizon = await runAction<HorizonView>("get_horizon_view", {
        kind: nextKind,
        periodStart: nextPeriod,
      });
      set({ horizon, horizonKind: nextKind, horizonPeriod: nextPeriod });
      return horizon;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshHabits: async () => {
    try {
      const habits = await runAction<Habit[]>("list_habits");
      set({ habits, dataError: null });
      return habits;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshTry: async () => {
    try {
      const tryItems = await runAction<TryItem[]>("list_try_items");
      set({ tryItems, dataError: null });
      return tryItems;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshDo: async () => {
    try {
      const doView = await runAction<DoView>("get_do_view");
      set({ doView, dataError: null });
      return doView;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshDashboard: async (range) => {
    try {
      const nextRange = range ?? get().dashboardRange;
      const dashboard = await runAction<DashboardView>("get_dashboard", {
        range: nextRange,
      });
      set({ dashboard, dashboardRange: nextRange, dataError: null });
      return dashboard;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshSettings: async () => {
    try {
      const settings = await runAction<Settings>("get_settings");
      set({ settings, dataError: null });
      return settings;
    } catch (error) {
      set({ dataError: normalizeError(error).message });
      return null;
    }
  },

  refreshMode: async () => {
    try {
      const mode = await runAction<Mode>("get_mode");
      set({ mode });
    } catch {
      // The fatal startup screen covers the useful failures here.
    }
  },

  refreshAfter: async (action) => {
    if (refreshExempt.has(action)) {
      return;
    }
    const nav = get().nav;
    const requests: Array<Promise<unknown>> = [
      get().refreshPlan(),
      get().refreshDaily(),
      get().refreshDo(),
      get().refreshMode(),
      get().refreshHabits(),
    ];
    if (nav === "plan") {
      requests.push(get().refreshHorizon());
    }
    if (nav === "try") {
      requests.push(get().refreshTry());
    }
    if (nav === "dashboard") {
      requests.push(get().refreshDashboard());
    }
    if (action === "update_settings") {
      requests.push(get().refreshSettings());
    }
    await Promise.all(requests);
  },

  act: async <T,>(action: string, payload: ActionPayload = {}) => {
    set({ busy: true });
    try {
      const result = await runAction<T>(action, payload);
      set({ busy: false });
      await get().refreshAfter(action);
      return result;
    } catch (raw) {
      const error = normalizeError(raw);
      set({ busy: false });
      if (error.overridable) {
        return new Promise<T | null>((resolve) => {
          set({
            override: {
              action,
              payload,
              error,
              resolve: resolve as (value: unknown | null) => void,
            },
          });
        });
      }
      get().notify(prettyErrorTitle(error), "error", error.message);
      return null;
    }
  },

  resolveOverride: async (choice) => {
    const current = get().override;
    if (!current) {
      return;
    }
    set({ override: null });
    if (!choice) {
      current.resolve(null);
      return;
    }
    const payload: ActionPayload = { ...current.payload };
    if (choice.type === "someday") {
      payload.ifFull = "SOMEDAY";
    } else {
      payload.overrideReason = choice.reason;
      payload.reason = choice.reason;
    }
    set({ busy: true });
    try {
      const result = await runAction(current.action, payload);
      set({ busy: false });
      await get().refreshAfter(current.action);
      get().notify("Saved with an override.", "warning");
      current.resolve(result);
    } catch (raw) {
      const error = normalizeError(raw);
      set({ busy: false });
      get().notify(prettyErrorTitle(error), "error", error.message);
      current.resolve(null);
    }
  },

  notify: (title, tone = "success", detail) => {
    const id = ++toastId;
    set((state) => ({
      toasts: [...state.toasts.slice(-2), { id, title, detail, tone }],
    }));
    window.setTimeout(() => {
      get().dismissToast(id);
    }, tone === "error" ? 8000 : 4800);
  },

  dismissToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) }));
  },
}));

function prettyErrorTitle(error: AppError): string {
  switch (error.code) {
    case "READY_FULL":
      return "Ready list is full";
    case "BUDGET_EXCEEDED":
      return "That goes past today's budget";
    case "PLAN_LOCKED":
      return "The plan is locked";
    case "STUCK_TASK":
      return "This task keeps rolling over";
    case "SLOT_FULL":
      return "That slot is full";
    case "BATCH_REQUIRES_MICRO":
      return "Batch is for 15-minute tasks";
    case "MISSING_REQUIRED_FIELD":
      return "A detail is missing";
    case "SESSION_ALREADY_ACTIVE":
      return "A timer is already running";
    case "EXTENSION_USED":
      return "Extra time was already added";
    case "NOT_PLANNED_FOR_TODAY":
      return "That task is not on today's plan";
    case "MISSING_OUTCOME":
      return "Give every leftover an outcome";
    case "NOTHING_TO_UNDO":
      return "Nothing to undo";
    case "NOT_FOUND":
      return "Not found";
    case "BOULDER":
      return "That is too big for one plan slot";
    case "INVALID_ESTIMATE":
      return "Pick a valid estimate";
    case "INVALID_SETTING":
      return "Check that setting";
    case "INVALID_DAY":
      return "That day cannot be planned";
    case "INVALID_TRANSITION":
      return "That change is not available";
    case "INTERNAL":
      return "Flow could not finish that";
    default:
      return "Something went wrong";
  }
}


