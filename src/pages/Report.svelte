<script lang="ts">
  import { api, type Dashboard } from "../lib/api";
  import Chart from "../lib/Chart.svelte";
  import { inr0, inrShort, monthLabel, pct, dmy } from "../lib/format";
  import { app } from "../lib/state.svelte";

  let db = $state.raw<Dashboard | null>(null);

  $effect(() => {
    const scope = app.scope;
    void app.version;
    if (!scope) return;
    api.dashboard(scope).then((d) => (db = d)).catch((e) => app.error(e));
  });

  const r = (p: number) => p / 100;
  const generated = new Date().toLocaleDateString("en-IN", { day: "2-digit", month: "short", year: "numeric" });

  const monthly = $derived.by(() => {
    if (!db) return null;
    const m = db.months;
    return {
      type: "bar",
      data: {
        labels: m.map((x) => monthLabel(x.month)),
        datasets: [
          { label: "Income", data: m.map((x) => r(x.income)), backgroundColor: "#43a047" },
          { label: "Expenses", data: m.map((x) => r(x.expenses)), backgroundColor: "#e53935" },
          { label: "Investments", data: m.map((x) => r(x.invested)), backgroundColor: "#1e88e5" },
        ],
      },
      options: {
        animation: false,
        maintainAspectRatio: false,
        scales: { y: { ticks: { callback: (v: any) => inrShort(Number(v)) } } },
      },
    };
  });

  const donut = $derived.by(() => {
    if (!db || !db.expense_categories.length) return null;
    const c = db.expense_categories;
    return {
      type: "doughnut",
      data: { labels: c.map((x) => x.name), datasets: [{ data: c.map((x) => r(x.amount)), backgroundColor: c.map((x) => x.color) }] },
      options: { animation: false, maintainAspectRatio: false, plugins: { legend: { position: "right" } } },
    };
  });
</script>

<div class="row no-print" style="margin-bottom:16px">
  <h1 style="margin:0">Report</h1>
  <span class="spacer"></span>
  <button class="primary" onclick={() => window.print()}>Print / Save as PDF</button>
</div>
<p class="muted small no-print">Choose "Microsoft Print to PDF" in the print dialog to save a PDF.</p>

{#if db}
  <div class="report">
    <header>
      <div>
        <div class="title">Bahi-Khata · Income & expense report</div>
        <div>{app.scopeLabel}</div>
        <div class="muted">
          {app.periodLabel}{#if db.first_date} · {dmy(db.first_date)} to {dmy(db.last_date)}{/if} · {db.txn_count} transactions
        </div>
      </div>
      <div class="muted small">Generated {generated}</div>
    </header>

    <section class="kpis">
      <div><span>Income</span><strong class="good">{inr0(db.income)}</strong></div>
      <div><span>Expenses</span><strong class="bad">{inr0(db.expenses)}</strong></div>
      <div><span>Investments</span><strong>{inr0(db.invested)}</strong></div>
      <div><span>Net savings</span><strong>{inr0(db.net)}</strong></div>
      <div><span>Savings rate</span><strong>{pct(db.savings_rate)}</strong></div>
    </section>
    {#if db.investments_as_expense}<p class="muted small">Investments are counted in expenses.</p>{/if}

    <section class="card">
      <h2>Month by month</h2>
      {#if monthly}<Chart config={monthly} height={240} />{/if}
      <table>
        <thead><tr><th>Month</th><th class="num">Income</th><th class="num">Expenses</th><th class="num">Investments</th><th class="num">Net</th></tr></thead>
        <tbody>
          {#each db.months as m (m.month)}
            <tr>
              <td>{monthLabel(m.month)}</td>
              <td class="num">{inr0(m.income)}</td>
              <td class="num">{inr0(m.expenses)}</td>
              <td class="num">{inr0(m.invested)}</td>
              <td class="num" class:bad={m.net < 0}>{inr0(m.net)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>

    <section class="card">
      <h2>Expenses by category</h2>
      {#if donut}<Chart config={donut} height={240} />{/if}
      <table>
        <thead><tr><th>Category</th><th class="num">Transactions</th><th class="num">Share</th><th class="num">Amount</th></tr></thead>
        <tbody>
          {#each db.expense_categories as c (c.id)}
            <tr>
              <td><span class="dot" style="background:{c.color}"></span>{c.name}</td>
              <td class="num">{c.count}</td>
              <td class="num">{pct(c.share)}</td>
              <td class="num">{inr0(c.amount)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>

    <div class="two">
      <section class="card">
        <h2>Income sources</h2>
        <table>
          <tbody>
            {#each db.income_categories as c (c.id)}
              <tr><td>{c.name}</td><td class="num">{pct(c.share)}</td><td class="num">{inr0(c.amount)}</td></tr>
            {/each}
          </tbody>
        </table>
        {#if db.investment_categories.length}
          <h2 style="margin-top:16px">Investments</h2>
          <table>
            <tbody>
              {#each db.investment_categories as c (c.id)}
                <tr><td>{c.name}</td><td class="num">{inr0(c.amount)}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </section>
      <section class="card">
        <h2>Recurring payments</h2>
        {#if db.recurring.length}
          <table>
            <tbody>
              {#each db.recurring as m, i (i)}
                <tr><td>{m.merchant}<div class="muted small">{m.category}</div></td><td class="num">{inr0(m.typical)}/month</td></tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="muted">None found.</p>
        {/if}
      </section>
    </div>

    <section class="card">
      <h2>Top payees</h2>
      <table>
        <thead><tr><th>Payee</th><th>Category</th><th class="num">Transactions</th><th class="num">Amount</th></tr></thead>
        <tbody>
          {#each db.merchants as m, i (i)}
            <tr><td>{m.merchant}</td><td>{m.category}</td><td class="num">{m.count}</td><td class="num">{inr0(m.amount)}</td></tr>
          {/each}
        </tbody>
      </table>
    </section>
  </div>
{/if}

<style>
  .report {
    max-width: 1000px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    border-bottom: 2px solid var(--accent);
    padding-bottom: 10px;
  }

  .title {
    font-size: 20px;
    font-weight: 600;
    color: var(--accent);
  }

  .kpis {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 12px;
  }

  .kpis div {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px;
    background: #fff;
  }

  .kpis span {
    font-size: 12px;
    color: var(--muted);
  }

  .kpis strong {
    font-size: 18px;
  }

  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  @media print {
    .report {
      max-width: none;
    }
  }
</style>
