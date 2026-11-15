<script lang="ts">
  import { api, type Dashboard } from "../lib/api";
  import Chart from "../lib/Chart.svelte";
  import { inr, inr0, inrShort, monthLabel, pct, dmy } from "../lib/format";
  import { app } from "../lib/state.svelte";

  let db = $state.raw<Dashboard | null>(null);
  let loading = $state(false);
  let seq = 0;

  $effect(() => {
    const scope = app.scope;
    void app.version;
    if (!scope) return;
    const my = ++seq;
    loading = true;
    api
      .dashboard(scope)
      .then((d) => {
        if (my === seq) db = d;
      })
      .catch((e) => app.error(e))
      .finally(() => {
        if (my === seq) loading = false;
      });
  });

  const r = (p: number) => p / 100;
  const money = { callbacks: { label: (c: any) => `${c.dataset.label ?? c.label}: ${inr(Math.round(c.raw * 100))}` } };
  const axis = { ticks: { callback: (v: any) => inrShort(Number(v)) } };

  const monthly = $derived.by(() => {
    if (!db) return null;
    const months = db.months;
    return {
      type: "bar",
      data: {
        labels: months.map((m) => monthLabel(m.month)),
        datasets: [
          { label: "Income", data: months.map((m) => r(m.income)), backgroundColor: "#43a047" },
          { label: "Expenses", data: months.map((m) => r(m.expenses)), backgroundColor: "#e53935" },
          { label: "Investments", data: months.map((m) => r(m.invested)), backgroundColor: "#1e88e5" },
          {
            type: "line",
            label: "Net savings",
            data: months.map((m) => r(m.net)),
            borderColor: "#37474f",
            backgroundColor: "#37474f",
            tension: 0.25,
          },
        ],
      },
      options: {
        maintainAspectRatio: false,
        plugins: { tooltip: money },
        scales: { y: axis },
        onClick: (_e: any, els: any[]) => {
          if (els.length && db) app.showTxns({ month: db.months[els[0].index].month });
        },
      },
    };
  });

  const donut = $derived.by(() => {
    if (!db || !db.expense_categories.length) return null;
    const cats = db.expense_categories;
    return {
      type: "doughnut",
      data: {
        labels: cats.map((c) => c.name),
        datasets: [{ data: cats.map((c) => r(c.amount)), backgroundColor: cats.map((c) => c.color) }],
      },
      options: {
        maintainAspectRatio: false,
        cutout: "58%",
        plugins: { legend: { position: "right" }, tooltip: money },
        onClick: (_e: any, els: any[]) => {
          if (els.length && db) app.showTxns({ category: db.expense_categories[els[0].index].id });
        },
      },
    };
  });

  const stacked = $derived.by(() => {
    if (!db || !db.category_months.length) return null;
    return {
      type: "bar",
      data: {
        labels: db.month_labels.map(monthLabel),
        datasets: db.category_months.map((s) => ({
          label: s.name,
          data: s.values.map(r),
          backgroundColor: s.color,
        })),
      },
      options: {
        maintainAspectRatio: false,
        plugins: { tooltip: money },
        scales: { x: { stacked: true }, y: { ...axis, stacked: true } },
      },
    };
  });
</script>

<div class="row head">
  <h1>Dashboard</h1>
  {#if db?.first_date}
    <span class="muted">{dmy(db.first_date)} to {dmy(db.last_date)} · {db.txn_count} transactions</span>
  {/if}
  {#if loading}<span class="muted small">Updating…</span>{/if}
</div>

{#if !app.scopeOptions.length}
  <div class="card empty">
    <p>Import your first bank statement to see the dashboard.</p>
    <button class="primary" onclick={() => (app.page = "import")}>Import a statement</button>
  </div>
{:else if db && db.txn_count === 0}
  <div class="card empty">No transactions in this period.</div>
{:else if db}
  <div class="kpis">
    <button class="card kpi" onclick={() => app.showTxns({ kind: "income" })}>
      <div class="muted small">Income</div>
      <div class="big good">{inr0(db.income)}</div>
    </button>
    <button class="card kpi" onclick={() => app.showTxns({ kind: "expense" })}>
      <div class="muted small">Expenses</div>
      <div class="big bad">{inr0(db.expenses)}</div>
    </button>
    <button class="card kpi" onclick={() => app.showTxns({ kind: "investment" })}>
      <div class="muted small">Investments{db.investments_as_expense ? " (counted in expenses)" : ""}</div>
      <div class="big" style="color:#1e88e5">{inr0(db.invested)}</div>
    </button>
    <div class="card kpi">
      <div class="muted small">Net savings</div>
      <div class="big" class:good={db.net >= 0} class:bad={db.net < 0}>{inr0(db.net)}</div>
    </div>
    <div class="card kpi">
      <div class="muted small">Savings rate</div>
      <div class="big">{pct(db.savings_rate)}</div>
    </div>
  </div>
  {#if db.excluded > 0}
    <p class="muted small">
      {inr0(db.excluded)} of transfers between your own accounts, credit-card bill payments and other excluded items are not
      counted.
      <button class="link small" onclick={() => app.showTxns({ kind: "excluded" })}>View</button>
    </p>
  {/if}

  <div class="grid two">
    <div class="card">
      <h2>Month by month</h2>
      {#if monthly}<Chart config={monthly} height={300} />{/if}
    </div>
    <div class="card">
      <h2>Where the money went</h2>
      {#if donut}<Chart config={donut} height={300} />{:else}<p class="muted">No expenses.</p>{/if}
    </div>
  </div>

  {#if stacked}
    <div class="card" style="margin-top:16px">
      <h2>Spending by category each month</h2>
      <Chart config={stacked} height={300} />
    </div>
  {/if}

  <div class="grid two" style="margin-top:16px">
    <div class="card">
      <h2>Top payees</h2>
      <table>
        <thead><tr><th>Payee</th><th>Category</th><th class="num">Count</th><th class="num">Amount</th></tr></thead>
        <tbody>
          {#each db.merchants as m, i (i)}
            <tr class="clickable" onclick={() => app.showTxns({ search: m.merchant })}>
              <td>{m.merchant}</td>
              <td class="muted">{m.category}</td>
              <td class="num">{m.count}</td>
              <td class="num">{inr0(m.amount)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="grid" style="align-content:start">
      <div class="card">
        <h2>Recurring payments</h2>
        {#if db.recurring.length}
          <table>
            <thead><tr><th>Payee</th><th class="num">Typical</th><th class="num">Months</th><th>Last paid</th></tr></thead>
            <tbody>
              {#each db.recurring as m, i (i)}
                <tr class="clickable" onclick={() => app.showTxns({ search: m.merchant })}>
                  <td>{m.merchant}<div class="muted small">{m.category}</div></td>
                  <td class="num">{inr0(m.typical)}</td>
                  <td class="num">{m.months}</td>
                  <td>{dmy(m.last_date)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="muted">None found. Needs at least 3 months of data.</p>
        {/if}
      </div>
      <div class="card">
        <h2>Income sources</h2>
        <table>
          <tbody>
            {#each db.income_categories as c (c.id)}
              <tr class="clickable" onclick={() => app.showTxns({ category: c.id })}>
                <td><span class="dot" style="background:{c.color}"></span>{c.name}</td>
                <td class="num muted">{pct(c.share)}</td>
                <td class="num">{inr0(c.amount)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if db.investment_categories.length}
          <h3 style="margin-top:16px">Investments</h3>
          <table>
            <tbody>
              {#each db.investment_categories as c (c.id)}
                <tr class="clickable" onclick={() => app.showTxns({ category: c.id })}>
                  <td><span class="dot" style="background:{c.color}"></span>{c.name}</td>
                  <td class="num">{inr0(c.amount)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .head {
    align-items: baseline;
    gap: 16px;
  }

  .kpis {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 16px;
    margin-bottom: 12px;
  }

  .kpi {
    text-align: left;
  }

  .big {
    font-size: 22px;
    font-weight: 600;
    margin-top: 4px;
    font-variant-numeric: tabular-nums;
  }

  .two {
    grid-template-columns: 1fr 1fr;
  }
</style>
