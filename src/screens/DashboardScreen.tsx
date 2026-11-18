import { useEffect } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { ArrowRight, ChartLineUp, CheckCircle, Circle, Repeat } from "@phosphor-icons/react";
import { formatDayMonth, formatMins, formatShortDay } from "../lib/format";
import { useFlow } from "../store";

const ranges = ["DAY", "WEEK", "MONTH", "QUARTER", "YEAR", "ALL"] as const;

const colors = {
  forest: "#1f4d3b",
  gold: "#a97329",
  line: "#dfe3de",
  soft: "#eef1ed",
  rust: "#9a493a",
};

export function DashboardScreen() {
  const dashboard = useFlow((state) => state.dashboard);
  const range = useFlow((state) => state.dashboardRange);
  const refreshDashboard = useFlow((state) => state.refreshDashboard);
  const navigate = useFlow((state) => state.navigate);

  useEffect(() => {
    void refreshDashboard();
  }, [refreshDashboard]);

  if (!dashboard) {
    return (
      <div className="page">
        <div className="skeleton skeleton-title" />
        <div className="skeleton skeleton-line" style={{ width: "58%" }} />
        <div className="skeleton skeleton-block" />
      </div>
    );
  }

  const completion = [
    { name: "Done", value: dashboard.done },
    { name: "Left", value: dashboard.left },
  ];
  const hasTasks = dashboard.planned > 0;
  const maxSection = Math.max(
    1,
    ...dashboard.sections.map((section) => section.planned),
  );

  return (
    <div className="page dashboard-page" data-testid="dashboard-page">
      <div className="page-head dashboard-head">
        <div>
          <div className="kicker">Dashboard</div>
          <h1 className="page-title">
            The shape of <em>the work.</em>
          </h1>
          <p className="page-sub">
            Planned, done, left, and what keeps slipping. Read the number first.
            The charts explain it.
          </p>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => navigate("daily")}
        >
          Open Daily <ArrowRight size={14} />
        </button>
      </div>

      <div className="segmented dashboard-range">
        {ranges.map((value) => (
          <button
            key={value}
            type="button"
            data-active={range === value}
            onClick={() => void refreshDashboard(value)}
          >
            {value === "ALL" ? "All" : value.charAt(0) + value.slice(1).toLowerCase()}
          </button>
        ))}
      </div>

      <div className="dashboard-summary">
        <article className="summary-card summary-primary">
          <span>Planned</span>
          <strong>{dashboard.planned}</strong>
          <small>
            {dashboard.daily_planned} daily, {dashboard.horizon_planned} horizon
          </small>
        </article>
        <article className="summary-card">
          <span>Done</span>
          <strong>{dashboard.done}</strong>
          <small>{dashboard.completion_rate.toFixed(0)}% complete</small>
        </article>
        <article className="summary-card">
          <span>Left</span>
          <strong>{dashboard.left}</strong>
          <small>{dashboard.overdue} past original date</small>
        </article>
        <article className="summary-card summary-warn">
          <span>Carried</span>
          <strong>{dashboard.carried}</strong>
          <small>open items with carry history</small>
        </article>
      </div>

      <div className="dashboard-grid">
        <section className="dashboard-card dashboard-donut">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Completion</div>
              <h2>Done versus left</h2>
            </div>
            <CheckCircle size={18} />
          </div>
          {hasTasks ? (
            <div className="donut-wrap">
              <ResponsiveContainer width="100%" height={220}>
                <PieChart>
                  <Pie
                    data={completion}
                    dataKey="value"
                    nameKey="name"
                    innerRadius={62}
                    outerRadius={88}
                    startAngle={90}
                    endAngle={-270}
                    stroke="none"
                  >
                    <Cell fill={colors.forest} />
                    <Cell fill={colors.line} />
                  </Pie>
                  <Tooltip />
                </PieChart>
              </ResponsiveContainer>
              <div className="donut-label">
                <strong>{dashboard.completion_rate.toFixed(0)}%</strong>
                <span>complete</span>
              </div>
            </div>
          ) : (
            <div className="dashboard-empty">No tasks planned in this range.</div>
          )}
        </section>

        <section className="dashboard-card dashboard-span-2">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Daily rhythm</div>
              <h2>Planned versus done</h2>
            </div>
            <span className="tiny-label">
              {formatDayMonth(dashboard.start_date)} to {formatDayMonth(dashboard.end_date)}
            </span>
          </div>
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={dashboard.days} barGap={3}>
              <CartesianGrid vertical={false} stroke={colors.line} />
              <XAxis
                dataKey="day"
                tickFormatter={formatShortDay}
                tickLine={false}
                axisLine={false}
                tick={{ fill: "#89918a", fontSize: 10 }}
              />
              <YAxis
                allowDecimals={false}
                width={24}
                tickLine={false}
                axisLine={false}
                tick={{ fill: "#89918a", fontSize: 10 }}
              />
              <Tooltip labelFormatter={(value) => formatShortDay(String(value))} />
              <Bar dataKey="planned" fill={colors.line} radius={[3, 3, 0, 0]} />
              <Bar dataKey="done" fill={colors.forest} radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </section>

        <section className="dashboard-card">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Where it slips</div>
              <h2>Completion by section</h2>
            </div>
            <Circle size={18} />
          </div>
          <div className="section-bars">
            {dashboard.sections.map((section) => (
              <div className="section-bar-row" key={section.label}>
                <div className="section-bar-copy">
                  <strong>{section.label}</strong>
                  <span>
                    {section.done} of {section.planned}
                  </span>
                </div>
                <div className="section-bar-track">
                  <div
                    className="section-bar-fill"
                    style={{ width: `${(section.planned / maxSection) * 100}%` }}
                  />
                  <div
                    className="section-bar-done"
                    style={{
                      width: `${section.planned ? (section.done / section.planned) * (section.planned / maxSection) * 100 : 0}%`,
                    }}
                  />
                </div>
                <span className="section-bar-count">{section.left}</span>
              </div>
            ))}
          </div>
        </section>

        <section className="dashboard-card">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Habits</div>
              <h2>Consistency</h2>
            </div>
            <Repeat size={18} />
          </div>
          <div className="dashboard-habit-summary">
            <strong>
              {dashboard.habit_done} <span>of {dashboard.habit_due}</span>
            </strong>
            <span>check-ins in this range</span>
          </div>
          {dashboard.habits.length === 0 ? (
            <div className="dashboard-empty">No habits due in this range.</div>
          ) : (
            <div className="habit-rate-list">
              {dashboard.habits.slice(0, 8).map((habit) => (
                <div className="habit-rate-row" key={habit.title}>
                  <div>
                    <strong>{habit.title}</strong>
                    <span>
                      {habit.done} of {habit.due}
                    </span>
                  </div>
                  <div className="habit-rate-track">
                    <span style={{ width: `${habit.rate}%` }} />
                  </div>
                  <span>{habit.rate.toFixed(0)}%</span>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="dashboard-card dashboard-span-2">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Focus</div>
              <h2>Minutes in the chair</h2>
            </div>
            <ChartLineUp size={18} />
          </div>
          <div className="focus-total">
            <strong>{formatMins(dashboard.focus_mins)}</strong>
            <span>across the range</span>
          </div>
          <ResponsiveContainer width="100%" height={190}>
            <AreaChart data={dashboard.days}>
              <defs>
                <linearGradient id="focusFill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={colors.forest} stopOpacity={0.28} />
                  <stop offset="100%" stopColor={colors.forest} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid vertical={false} stroke={colors.line} />
              <XAxis
                dataKey="day"
                tickFormatter={formatShortDay}
                tickLine={false}
                axisLine={false}
                tick={{ fill: "#89918a", fontSize: 10 }}
              />
              <YAxis hide />
              <Tooltip labelFormatter={(value) => formatShortDay(String(value))} />
              <Area
                type="monotone"
                dataKey="focus_mins"
                stroke={colors.forest}
                strokeWidth={2}
                fill="url(#focusFill)"
              />
            </AreaChart>
          </ResponsiveContainer>
        </section>

        <section className="dashboard-card">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Friction</div>
              <h2>Most carried</h2>
            </div>
            <Repeat size={18} />
          </div>
          {dashboard.carried_items.length === 0 ? (
            <div className="dashboard-empty">Nothing has been carried in this range.</div>
          ) : (
            <div className="carried-list">
              {dashboard.carried_items.map((item) => (
                <div className="carried-row" key={`${item.kind}-${item.title}`}>
                  <div>
                    <strong>{item.title}</strong>
                    <span>{item.kind}</span>
                  </div>
                  <span className="carried-count">{item.count}x</span>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="dashboard-card">
          <div className="dashboard-card-head">
            <div>
              <div className="kicker">Try</div>
              <h2>The shelf</h2>
            </div>
            <SparkleIcon />
          </div>
          <div className="try-summary">
            <div>
              <strong>{dashboard.try_scheduled}</strong>
              <span>scheduled</span>
            </div>
            <div>
              <strong>{dashboard.try_tried}</strong>
              <span>tried</span>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function SparkleIcon() {
  return <span className="dashboard-spark">✦</span>;
}
