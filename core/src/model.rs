use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! ipc {
    ($($item:item)*) => {
        $(
            #[derive(Debug, Clone, Serialize, Deserialize, TS)]
            #[ts(export, export_to = "../../src/generated/")]
            $item
        )*
    };
}

macro_rules! ipc_enum {
    ($($item:item)*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
            #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
            #[ts(export, export_to = "../../src/generated/", rename_all = "SCREAMING_SNAKE_CASE")]
            $item
        )*
    };
}

ipc_enum! {
    pub enum Status { Inbox, Ready, Waiting, Someday, Done, Dropped }
    pub enum Slot { One, Three, Batch, Extra }
    pub enum ClosedReason { Split, NotDoing }
    pub enum SessionKind { Focus, Batch, Manual }
    pub enum Mode { Plan, Do }
    pub enum DoState { Working, AllDone }
    pub enum TriageTarget { Waiting, Someday, Dropped }
    pub enum OutcomeKind { Carry, Waiting, Someday, Drop, Split, Shrink }
    pub enum Freq { Daily, Weekly, EveryNDays }
    pub enum ReadyFullAction { Someday }
    pub enum IfFull { Someday }
    pub enum HorizonKind { Week, Month, Quarter, Year, LongTerm }
    pub enum Band { Focus, Secondary, Commitment, More }
    pub enum GoalStatus { Open, Done, Dropped }
    pub enum HabitEntryStatus { Done, Skipped }
    pub enum TryStatus { Idea, Scheduled, Tried, Skipped }
    pub enum TryEffort { Low, Medium, High }
    pub enum TryCost { Free, Low, Medium, High }
    pub enum SocialMode { Solo, Social, Either }
    pub enum EnergyLevel { Low, Medium, High }
    pub enum TimeOfDay { Morning, Afternoon, Evening, Anytime }
    pub enum CadenceKind {
        Daily,
        Weekly,
        EveryNDays,
        EveryNWeeks,
        MonthlyDay,
        MonthlyNthWeekday,
        Yearly,
        CustomDates,
        Quota,
        OneOff,
    }
    pub enum QuotaPeriod { Week, Month }
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Inbox => "INBOX",
            Status::Ready => "READY",
            Status::Waiting => "WAITING",
            Status::Someday => "SOMEDAY",
            Status::Done => "DONE",
            Status::Dropped => "DROPPED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "READY" => Status::Ready,
            "WAITING" => Status::Waiting,
            "SOMEDAY" => Status::Someday,
            "DONE" => Status::Done,
            "DROPPED" => Status::Dropped,
            _ => Status::Inbox,
        }
    }
}

impl Slot {
    pub fn as_str(self) -> &'static str {
        match self {
            Slot::One => "ONE",
            Slot::Three => "THREE",
            Slot::Batch => "BATCH",
            Slot::Extra => "EXTRA",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "ONE" => Slot::One,
            "THREE" => Slot::Three,
            "BATCH" => Slot::Batch,
            _ => Slot::Extra,
        }
    }

    pub fn capacity(self) -> i64 {
        match self {
            Slot::One => 1,
            Slot::Three => 3,
            Slot::Batch => 5,
            Slot::Extra => i64::MAX,
        }
    }
}

impl ClosedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            ClosedReason::Split => "SPLIT",
            ClosedReason::NotDoing => "NOT_DOING",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "SPLIT" => ClosedReason::Split,
            _ => ClosedReason::NotDoing,
        }
    }
}

impl SessionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionKind::Focus => "FOCUS",
            SessionKind::Batch => "BATCH",
            SessionKind::Manual => "MANUAL",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "BATCH" => SessionKind::Batch,
            "MANUAL" => SessionKind::Manual,
            _ => SessionKind::Focus,
        }
    }
}

impl Freq {
    pub fn as_str(self) -> &'static str {
        match self {
            Freq::Daily => "DAILY",
            Freq::Weekly => "WEEKLY",
            Freq::EveryNDays => "EVERY_N_DAYS",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "WEEKLY" => Freq::Weekly,
            "EVERY_N_DAYS" => Freq::EveryNDays,
            _ => Freq::Daily,
        }
    }
}

ipc! {
    pub struct Task {
        pub id: String,
        pub title: String,
        pub first_step: Option<String>,
        pub done_when: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        pub status: Status,
        #[ts(type = "number")]
        pub value: i64,
        pub pinned: bool,
        pub deadline_on: Option<String>,
        pub waiting_on: Option<String>,
        pub review_on: Option<String>,
        pub breadcrumb: Option<String>,
        pub cue: Option<String>,
        pub parent_id: Option<String>,
        pub closed_reason: Option<ClosedReason>,
        #[ts(type = "number")]
        pub rollover_count: i64,
        pub planned_for: Option<String>,
        pub slot: Option<Slot>,
        #[ts(type = "number | null")]
        pub slot_pos: Option<i64>,
        pub created_at: String,
        pub updated_at: String,
        pub completed_at: Option<String>,
        pub recurrence_id: Option<String>,
        pub occurrence_on: Option<String>,
    }

    pub struct Session {
        pub id: String,
        pub kind: SessionKind,
        pub task_id: Option<String>,
        pub day: String,
        #[ts(type = "number")]
        pub planned_mins: i64,
        pub extended: bool,
        #[ts(type = "number")]
        pub accumulated_secs: i64,
        pub running_since: Option<String>,
        pub last_seen_at: Option<String>,
        pub started_at: String,
        pub ended_at: Option<String>,
        #[ts(type = "number")]
        pub elapsed_secs: i64,
        pub running: bool,
    }

    pub struct Day {
        pub day: String,
        pub plan_locked_at: Option<String>,
        #[ts(type = "number | null")]
        pub multiplier_at_lock: Option<f64>,
        #[ts(type = "number | null")]
        pub budget_at_lock: Option<i64>,
        pub wrapup_done_at: Option<String>,
        pub win_note: Option<String>,
    }

    pub struct Recurrence {
        pub id: String,
        pub title: String,
        pub first_step: String,
        pub done_when: Option<String>,
        #[ts(type = "number")]
        pub estimate_mins: i64,
        #[ts(type = "number")]
        pub value: i64,
        pub cue: Option<String>,
        pub freq: Freq,
        #[ts(type = "number")]
        pub interval_n: i64,
        #[ts(type = "number")]
        pub weekdays_mask: i64,
        pub starts_on: String,
        pub active: bool,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct RecurrenceView {
        pub recurrence: Recurrence,
        pub next_due_on: Option<String>,
        pub has_open_occurrence: bool,
    }

    #[serde(default)]
    pub struct Cadence {
        pub kind: CadenceKind,
        pub anchor_on: String,
        #[ts(type = "number")]
        pub interval_n: i64,
        #[ts(type = "number")]
        pub weekdays_mask: i64,
        #[ts(type = "number | null")]
        pub month_day: Option<i64>,
        #[ts(type = "number | null")]
        pub month: Option<i64>,
        #[ts(type = "number | null")]
        pub nth_weekday: Option<i64>,
        #[ts(type = "number | null")]
        pub weekday: Option<i64>,
        pub dates: Vec<String>,
        #[ts(type = "number | null")]
        pub quota_count: Option<i64>,
        pub quota_period: Option<QuotaPeriod>,
    }

    pub struct Area {
        pub id: String,
        pub name: String,
        pub color: String,
        #[ts(type = "number")]
        pub sort_order: i64,
        pub active: bool,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct Goal {
        pub id: String,
        pub horizon_kind: HorizonKind,
        pub period_start: Option<String>,
        pub area_id: Option<String>,
        pub parent_id: Option<String>,
        pub band: Band,
        #[ts(type = "number")]
        pub position: i64,
        pub title: String,
        pub description: Option<String>,
        pub success_definition: Option<String>,
        pub status: GoalStatus,
        pub target_on: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        pub completed_at: Option<String>,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct GoalView {
        pub goal: Goal,
        #[ts(type = "number")]
        pub linked_tasks: i64,
        #[ts(type = "number")]
        pub done_tasks: i64,
        pub linked_titles: Vec<String>,
        pub linked_task_ids: Vec<String>,
    }

    pub struct GoalBands {
        pub focus: Option<GoalView>,
        pub secondary: Vec<GoalView>,
        pub commitment: Vec<GoalView>,
        pub more: Vec<GoalView>,
    }

    pub struct DaySummary {
        pub day: String,
        #[ts(type = "number")]
        pub planned_mins: i64,
        #[ts(type = "number")]
        pub open_count: i64,
        #[ts(type = "number")]
        pub done_count: i64,
        pub locked: bool,
    }

    pub struct HorizonView {
        pub kind: HorizonKind,
        pub period_start: Option<String>,
        pub period_end: Option<String>,
        pub label: String,
        pub intent: Option<String>,
        pub review_note: Option<String>,
        pub bands: GoalBands,
        pub completed: Vec<GoalView>,
        pub days: Vec<DaySummary>,
    }

    pub struct Note {
        pub scope: String,
        pub period_start: String,
        pub body_markdown: String,
        pub updated_at: Option<String>,
    }

    pub struct GoalInput {
        pub horizon_kind: HorizonKind,
        pub period_start: Option<String>,
        pub area_id: Option<String>,
        pub parent_id: Option<String>,
        pub band: Band,
        pub title: String,
        pub description: Option<String>,
        pub success_definition: Option<String>,
        pub target_on: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
    }

    pub struct AreaInput {
        pub name: String,
        pub color: Option<String>,
        #[ts(type = "number | null")]
        pub sort_order: Option<i64>,
    }

    pub struct Habit {
        pub id: String,
        pub title: String,
        pub why: Option<String>,
        pub cue: Option<String>,
        pub area_id: Option<String>,
        pub schedule: Cadence,
        #[ts(type = "number")]
        pub target_count: i64,
        pub unit_label: Option<String>,
        pub time_of_day: TimeOfDay,
        #[ts(type = "number")]
        pub sort_order: i64,
        pub active: bool,
        pub starts_on: Option<String>,
        pub ends_on: Option<String>,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct HabitEntry {
        pub id: String,
        pub habit_id: String,
        pub on_date: String,
        #[ts(type = "number")]
        pub count: i64,
        pub status: HabitEntryStatus,
        pub note: Option<String>,
        pub task_id: Option<String>,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct HabitToday {
        pub habit: Habit,
        pub entry: Option<HabitEntry>,
        #[ts(type = "number")]
        pub quota_done: i64,
        #[ts(type = "number")]
        pub quota_target: i64,
        #[ts(type = "number")]
        pub streak: i64,
        pub cadence_label: String,
    }

    pub struct HabitInput {
        pub title: String,
        pub why: Option<String>,
        pub cue: Option<String>,
        pub area_id: Option<String>,
        pub schedule: Cadence,
        #[ts(type = "number")]
        pub target_count: i64,
        pub unit_label: Option<String>,
        pub time_of_day: TimeOfDay,
        #[ts(type = "number")]
        pub sort_order: i64,
        pub starts_on: Option<String>,
        pub ends_on: Option<String>,
    }

    pub struct HabitStats {
        #[ts(type = "number")]
        pub current_streak: i64,
        #[ts(type = "number")]
        pub best_streak: i64,
        pub completion_rate: f64,
        #[ts(type = "number")]
        pub done_count: i64,
        #[ts(type = "number")]
        pub scheduled_count: i64,
    }

    pub struct DailyView {
        pub day: String,
        pub locked: bool,
        pub plan: PlanSlots,
        pub budget: BudgetView,
        pub habits: Vec<HabitToday>,
        #[ts(type = "number")]
        pub habit_total: i64,
        #[ts(type = "number")]
        pub habit_done: i64,
        pub completed: Vec<Task>,
    }

    pub struct TryItem {
        pub id: String,
        pub title: String,
        pub notes: Option<String>,
        pub area_id: Option<String>,
        pub category: Option<String>,
        pub link: Option<String>,
        pub effort: TryEffort,
        pub cost: TryCost,
        pub social_mode: SocialMode,
        pub energy: EnergyLevel,
        pub status: TryStatus,
        pub scheduled_for: Option<String>,
        pub scheduled_day_part: Option<TimeOfDay>,
        pub linked_task_id: Option<String>,
        #[ts(type = "number | null")]
        pub rating: Option<i64>,
        pub reflection: Option<String>,
        pub tried_at: Option<String>,
        pub created_at: String,
        pub updated_at: String,
    }

    pub struct TryItemInput {
        pub title: String,
        pub notes: Option<String>,
        pub area_id: Option<String>,
        pub category: Option<String>,
        pub link: Option<String>,
        pub effort: Option<TryEffort>,
        pub cost: Option<TryCost>,
        pub social_mode: Option<SocialMode>,
        pub energy: Option<EnergyLevel>,
    }

    pub struct DashboardDay {
        pub day: String,
        #[ts(type = "number")]
        pub planned: i64,
        #[ts(type = "number")]
        pub done: i64,
        #[ts(type = "number")]
        pub left: i64,
        #[ts(type = "number")]
        pub focus_mins: i64,
    }

    pub struct DashboardSection {
        pub label: String,
        #[ts(type = "number")]
        pub planned: i64,
        #[ts(type = "number")]
        pub done: i64,
        #[ts(type = "number")]
        pub left: i64,
    }

    pub struct DashboardHabit {
        pub title: String,
        #[ts(type = "number")]
        pub due: i64,
        #[ts(type = "number")]
        pub done: i64,
        pub rate: f64,
    }

    pub struct DashboardCarried {
        pub title: String,
        pub kind: String,
        #[ts(type = "number")]
        pub count: i64,
    }

    pub struct DashboardView {
        pub range: String,
        pub start_date: String,
        pub end_date: String,
        #[ts(type = "number")]
        pub planned: i64,
        #[ts(type = "number")]
        pub done: i64,
        #[ts(type = "number")]
        pub left: i64,
        #[ts(type = "number")]
        pub daily_planned: i64,
        #[ts(type = "number")]
        pub daily_done: i64,
        #[ts(type = "number")]
        pub horizon_planned: i64,
        #[ts(type = "number")]
        pub horizon_done: i64,
        #[ts(type = "number")]
        pub carried: i64,
        #[ts(type = "number")]
        pub overdue: i64,
        #[ts(type = "number")]
        pub habit_due: i64,
        #[ts(type = "number")]
        pub habit_done: i64,
        #[ts(type = "number")]
        pub focus_mins: i64,
        pub completion_rate: f64,
        pub days: Vec<DashboardDay>,
        pub sections: Vec<DashboardSection>,
        pub habits: Vec<DashboardHabit>,
        pub carried_items: Vec<DashboardCarried>,
        #[ts(type = "number")]
        pub try_scheduled: i64,
        #[ts(type = "number")]
        pub try_tried: i64,
    }

    pub struct Settings {
        #[ts(type = "number")]
        pub id: i64,
        #[ts(type = "number")]
        pub day_rollover_hour: i64,
        #[ts(type = "number")]
        pub daily_focus_budget_mins: i64,
        #[ts(type = "number")]
        pub ready_cap: i64,
        #[ts(type = "number")]
        pub focus_block_mins: i64,
        #[ts(type = "number")]
        pub batch_block_mins: i64,
        #[ts(type = "number")]
        pub default_multiplier: f64,
        pub last_weekly_review_at: Option<String>,
        #[ts(type = "number")]
        pub week_starts_on: i64,
        pub auto_carry_enabled: bool,
        pub dashboard_default_range: String,
        pub notes_autosave: bool,
        pub notify_block_end: bool,
        pub planning_reminder_enabled: bool,
        pub planning_reminder_time: Option<String>,
        pub reduced_motion: bool,
    }

    pub struct SettingsPatch {
        #[ts(type = "number | null")]
        pub day_rollover_hour: Option<i64>,
        #[ts(type = "number | null")]
        pub daily_focus_budget_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub ready_cap: Option<i64>,
        #[ts(type = "number | null")]
        pub focus_block_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub batch_block_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub default_multiplier: Option<f64>,
        pub last_weekly_review_at: Option<Option<String>>,
        #[ts(type = "number | null")]
        pub week_starts_on: Option<i64>,
        pub auto_carry_enabled: Option<bool>,
        pub dashboard_default_range: Option<String>,
        pub notes_autosave: Option<bool>,
        pub notify_block_end: Option<bool>,
        pub planning_reminder_enabled: Option<bool>,
        pub planning_reminder_time: Option<Option<String>>,
        pub reduced_motion: Option<bool>,
    }

    pub struct Event {
        #[ts(type = "number")]
        pub id: i64,
        pub at: String,
        pub task_id: Option<String>,
        pub kind: String,
        pub reason: Option<String>,
        pub before_json: Option<String>,
    }

    pub struct ClarifyInput {
        pub title: String,
        pub first_step: Option<String>,
        pub done_when: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
        pub deadline_on: Option<String>,
    }

    pub struct QuickAddInput {
        pub title: String,
        #[ts(type = "number")]
        pub estimate_mins: i64,
        pub slot: Slot,
        pub day: String,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
    }

    pub struct ReactivateInput {
        pub first_step: Option<String>,
        pub done_when: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
        pub deadline_on: Option<String>,
    }

    pub struct TaskPatch {
        pub title: Option<String>,
        pub first_step: Option<Option<String>>,
        pub done_when: Option<Option<String>>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
        pub deadline_on: Option<Option<String>>,
        pub cue: Option<Option<String>>,
        pub breadcrumb: Option<Option<String>>,
        pub waiting_on: Option<Option<String>>,
        pub review_on: Option<Option<String>>,
    }

    pub struct SplitChild {
        pub title: String,
        pub first_step: String,
        #[ts(type = "number")]
        pub estimate_mins: i64,
    }

    pub struct TriageInput {
        pub to: TriageTarget,
        pub waiting_on: Option<String>,
        pub review_on: Option<String>,
    }

    pub struct ResolveInput {
        pub outcome: OutcomeKind,
        pub waiting_on: Option<String>,
        pub review_on: Option<String>,
        pub breadcrumb: Option<String>,
        pub children: Option<Vec<SplitChild>>,
        pub first_step: Option<String>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
    }

    pub struct PlanAssignment {
        pub id: String,
        pub slot: Slot,
    }

    pub struct PlanProposal {
        pub assignments: Vec<PlanAssignment>,
        #[ts(type = "number")]
        pub planned_mins: i64,
        #[ts(type = "number")]
        pub multiplier: f64,
    }

    pub struct PlanSlots {
        pub one: Option<Task>,
        pub three: Vec<Task>,
        pub batch: Vec<Task>,
        pub extra: Vec<Task>,
    }

    pub struct BudgetView {
        #[ts(type = "number")]
        pub planned_mins: i64,
        #[ts(type = "number")]
        pub budget_mins: i64,
        #[ts(type = "number")]
        pub multiplier: f64,
    }

    pub struct Banners {
        pub due_for_review: Vec<Task>,
        pub stuck: Vec<Task>,
        pub weekly_review_due: bool,
    }

    pub struct PlanView {
        pub target_day: String,
        pub locked: bool,
        pub deep_candidates: Vec<Task>,
        pub quick_candidates: Vec<Task>,
        pub inbox: Vec<Task>,
        pub plan: PlanSlots,
        pub budget: BudgetView,
        #[ts(type = "number")]
        pub ready_count: i64,
        #[ts(type = "number")]
        pub ready_cap: i64,
        pub banners: Banners,
        pub dropped: Vec<Task>,
        pub recurring: Vec<Task>,
        pub recurrences: Vec<RecurrenceView>,
    }

    pub struct SessionView {
        pub id: String,
        pub kind: SessionKind,
        #[ts(type = "number")]
        pub planned_mins: i64,
        #[ts(type = "number")]
        pub elapsed_secs: i64,
        pub running: bool,
        pub extended: bool,
        pub running_since: Option<String>,
    }

    pub struct CurrentTask {
        pub task: Task,
        pub slot: Slot,
        #[ts(type = "number")]
        pub logged_today_mins: i64,
        pub has_session_today: bool,
    }

    pub struct DoView {
        pub state: DoState,
        pub current: Option<CurrentTask>,
        #[ts(type = "number")]
        pub remaining_count: i64,
        #[ts(type = "number")]
        pub done_count: i64,
        pub session: Option<SessionView>,
        pub others: Vec<CurrentTask>,
        pub win_note: Option<String>,
    }

    pub struct UnfinishedView {
        pub task: Task,
        #[ts(type = "number")]
        pub logged_today_mins: i64,
        pub needs_breadcrumb: bool,
    }

    pub struct WrapupView {
        pub unfinished: Vec<UnfinishedView>,
        pub stuck: Vec<Task>,
        pub inbox: Vec<Task>,
        pub win_note: Option<String>,
        pub tomorrow: PlanView,
    }

    pub struct DayMins {
        pub day: String,
        #[ts(type = "number")]
        pub mins: i64,
    }

    pub struct RolloverStat {
        pub id: String,
        pub title: String,
        #[ts(type = "number")]
        pub rollover_count: i64,
    }

    pub struct WinNote {
        pub day: String,
        pub note: String,
    }

    pub struct Stats {
        #[ts(type = "number")]
        pub multiplier: f64,
        #[ts(type = "number")]
        pub sample_count: i64,
        #[ts(type = "number")]
        pub no_time_data_count: i64,
        pub focus_mins_last_14: Vec<DayMins>,
        pub most_rolled_over: Vec<RolloverStat>,
        pub recent_win_notes: Vec<WinNote>,
    }

    pub struct RecurrenceInput {
        pub title: String,
        pub first_step: String,
        pub done_when: Option<String>,
        #[ts(type = "number")]
        pub estimate_mins: i64,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
        pub cue: Option<String>,
        pub freq: Freq,
        #[ts(type = "number | null")]
        pub interval_n: Option<i64>,
        #[ts(type = "number | null")]
        pub weekdays_mask: Option<i64>,
        pub starts_on: String,
    }

    pub struct RecurrencePatch {
        pub title: Option<String>,
        pub first_step: Option<String>,
        pub done_when: Option<Option<String>>,
        #[ts(type = "number | null")]
        pub estimate_mins: Option<i64>,
        #[ts(type = "number | null")]
        pub value: Option<i64>,
        pub cue: Option<Option<String>>,
        pub freq: Option<Freq>,
        #[ts(type = "number | null")]
        pub interval_n: Option<i64>,
        #[ts(type = "number | null")]
        pub weekdays_mask: Option<i64>,
        pub starts_on: Option<String>,
        pub active: Option<bool>,
    }

    pub struct LockResult {
        pub locked_at: String,
        pub no_one: bool,
    }

    pub struct ReconcileReport {
        pub carried: Vec<String>,
        pub frozen: Vec<String>,
        pub stale_session: Option<Session>,
    }

    pub struct StartupStatus {
        pub ok: bool,
        pub error: Option<String>,
    }

    pub struct Paths {
        pub db: String,
        pub backups: String,
    }
}

pub fn is_micro(estimate: Option<i64>) -> bool {
    estimate == Some(15)
}

pub fn is_deep(estimate: Option<i64>) -> bool {
    matches!(estimate, Some(m) if m >= 30)
}

impl Default for Cadence {
    fn default() -> Self {
        Self {
            kind: CadenceKind::Daily,
            anchor_on: String::new(),
            interval_n: 1,
            weekdays_mask: 127,
            month_day: None,
            month: None,
            nth_weekday: None,
            weekday: None,
            dates: Vec::new(),
            quota_count: None,
            quota_period: None,
        }
    }
}

impl CadenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CadenceKind::Daily => "DAILY",
            CadenceKind::Weekly => "WEEKLY",
            CadenceKind::EveryNDays => "EVERY_N_DAYS",
            CadenceKind::EveryNWeeks => "EVERY_N_WEEKS",
            CadenceKind::MonthlyDay => "MONTHLY_DAY",
            CadenceKind::MonthlyNthWeekday => "MONTHLY_NTH_WEEKDAY",
            CadenceKind::Yearly => "YEARLY",
            CadenceKind::CustomDates => "CUSTOM_DATES",
            CadenceKind::Quota => "QUOTA",
            CadenceKind::OneOff => "ONE_OFF",
        }
    }
}

impl HorizonKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HorizonKind::Week => "WEEK",
            HorizonKind::Month => "MONTH",
            HorizonKind::Quarter => "QUARTER",
            HorizonKind::Year => "YEAR",
            HorizonKind::LongTerm => "LONG_TERM",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "MONTH" => HorizonKind::Month,
            "QUARTER" => HorizonKind::Quarter,
            "YEAR" => HorizonKind::Year,
            "LONG_TERM" => HorizonKind::LongTerm,
            _ => HorizonKind::Week,
        }
    }
}

impl Band {
    pub fn as_str(self) -> &'static str {
        match self {
            Band::Focus => "FOCUS",
            Band::Secondary => "SECONDARY",
            Band::Commitment => "COMMITMENT",
            Band::More => "MORE",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "FOCUS" => Band::Focus,
            "SECONDARY" => Band::Secondary,
            "COMMITMENT" => Band::Commitment,
            _ => Band::More,
        }
    }

    pub fn capacity(self) -> i64 {
        match self {
            Band::Focus => 1,
            Band::Secondary => 3,
            Band::Commitment => 5,
            Band::More => i64::MAX,
        }
    }
}

impl GoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalStatus::Open => "OPEN",
            GoalStatus::Done => "DONE",
            GoalStatus::Dropped => "DROPPED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "DONE" => GoalStatus::Done,
            "DROPPED" => GoalStatus::Dropped,
            _ => GoalStatus::Open,
        }
    }
}

impl HabitEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HabitEntryStatus::Done => "DONE",
            HabitEntryStatus::Skipped => "SKIPPED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "SKIPPED" => HabitEntryStatus::Skipped,
            _ => HabitEntryStatus::Done,
        }
    }
}

impl TryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TryStatus::Idea => "IDEA",
            TryStatus::Scheduled => "SCHEDULED",
            TryStatus::Tried => "TRIED",
            TryStatus::Skipped => "SKIPPED",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "SCHEDULED" => TryStatus::Scheduled,
            "TRIED" => TryStatus::Tried,
            "SKIPPED" => TryStatus::Skipped,
            _ => TryStatus::Idea,
        }
    }
}

impl TryEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            TryEffort::Low => "LOW",
            TryEffort::Medium => "MEDIUM",
            TryEffort::High => "HIGH",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "LOW" => TryEffort::Low,
            "HIGH" => TryEffort::High,
            _ => TryEffort::Medium,
        }
    }
}

impl TryCost {
    pub fn as_str(self) -> &'static str {
        match self {
            TryCost::Free => "FREE",
            TryCost::Low => "LOW",
            TryCost::Medium => "MEDIUM",
            TryCost::High => "HIGH",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "FREE" => TryCost::Free,
            "LOW" => TryCost::Low,
            "HIGH" => TryCost::High,
            _ => TryCost::Medium,
        }
    }
}

impl SocialMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SocialMode::Solo => "SOLO",
            SocialMode::Social => "SOCIAL",
            SocialMode::Either => "EITHER",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "SOLO" => SocialMode::Solo,
            "SOCIAL" => SocialMode::Social,
            _ => SocialMode::Either,
        }
    }
}

impl EnergyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            EnergyLevel::Low => "LOW",
            EnergyLevel::Medium => "MEDIUM",
            EnergyLevel::High => "HIGH",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "LOW" => EnergyLevel::Low,
            "HIGH" => EnergyLevel::High,
            _ => EnergyLevel::Medium,
        }
    }
}

impl TimeOfDay {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeOfDay::Morning => "MORNING",
            TimeOfDay::Afternoon => "AFTERNOON",
            TimeOfDay::Evening => "EVENING",
            TimeOfDay::Anytime => "ANYTIME",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "MORNING" => TimeOfDay::Morning,
            "AFTERNOON" => TimeOfDay::Afternoon,
            "EVENING" => TimeOfDay::Evening,
            _ => TimeOfDay::Anytime,
        }
    }
}

impl QuotaPeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            QuotaPeriod::Week => "WEEK",
            QuotaPeriod::Month => "MONTH",
        }
    }
    pub fn from_db(value: &str) -> Self {
        match value {
            "MONTH" => QuotaPeriod::Month,
            _ => QuotaPeriod::Week,
        }
    }
}
