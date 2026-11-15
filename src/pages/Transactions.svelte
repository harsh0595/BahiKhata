<script lang="ts">
  import { api, type CatKind, type Direction, type TxnPage, type TxnRow } from "../lib/api";
  import { inr, dmy, monthLabel, KIND_LABEL, SOURCE_LABEL } from "../lib/format";
  import { app } from "../lib/state.svelte";

  const PAGE_SIZE = 100;

  let search = $state(app.txnFilter.search);
  let category = $state<string | null>(app.txnFilter.category);
  let kind = $state<CatKind | null>(app.txnFilter.kind as CatKind | null);
  let month = $state<string | null>(app.txnFilter.month);
  let direction = $state<Direction | null>(null);
  let sort = $state("date_desc");
  let pageNo = $state(0);
  let data = $state.raw<TxnPage | null>(null);
  let selected = $state<number[]>([]);
  let bulkCat = $state("");
  let seq = 0;

  // Suggest a rule after a manual category change.
  let suggest = $state<{ pattern: string; category: string; direction: Direction | null; matches: number } | null>(null);
  let noteEdit = $state<{ id: number; text: string } | null>(null);

  // Filters set from the dashboard.
  $effect(() => {
    const f = app.txnFilter;
    search = f.search;
    category = f.category;
    kind = f.kind as CatKind | null;
    month = f.month;
    direction = null;
    pageNo = 0;
  });

  let debounced = $state(search);
  $effect(() => {
    const s = search;
    const t = setTimeout(() => {
      debounced = s;
      pageNo = 0;
    }, 250);
    return () => clearTimeout(t);
  });

  async function load() {
    const scope = app.scope;
    if (!scope) return;
    const my = ++seq;
    try {
      const d = await api.queryTxns({
        scope,
        search: debounced,
        category,
        kind,
        direction,
        month,
        sort,
        page: pageNo,
        page_size: PAGE_SIZE,
      });
      if (my === seq) data = d;
    } catch (e) {
      app.error(e);
    }
  }

  $effect(() => {
    void [app.scope, category, kind, direction, month, sort];
    pageNo = 0;
  });

  $effect(() => {
    void [app.scope, app.version, debounced, category, kind, direction, month, sort, pageNo];
    load();
  });

  const pages = $derived(data ? Math.max(1, Math.ceil(data.total / PAGE_SIZE)) : 1);
  const sortedCats = $derived([...app.categories].sort((a, b) => a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name)));

  function clearFilters() {
    app.txnFilter = { category: null, kind: null, month: null, search: "" };
    sort = "date_desc";
  }

  function rulePattern(t: TxnRow): string {
    if (t.merchant && t.merchant.replace(/[^A-Za-z0-9]/g, "").length >= 3 && t.merchant !== "ATM CASH") return t.merchant;
    return t.narration.split(/[-/]/).filter((p) => p.trim().length >= 3)[0]?.trim() ?? t.narration;
  }

  async function changeCategory(t: TxnRow, cat: string) {
    try {
      await api.setCategory([t.id], cat);
      const pattern = rulePattern(t);
      const matches = await api.ruleMatches(pattern, t.direction);
      suggest = matches > 1 ? { pattern, category: cat, direction: t.direction, matches } : null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function checkSuggest() {
    if (!suggest) return;
    suggest.matches = await api.ruleMatches(suggest.pattern, suggest.direction);
  }

  async function createRule() {
    if (!suggest) return;
    try {
      await api.addRule(suggest.pattern, suggest.category, suggest.direction);
      app.info(`Rule created. Transactions you set by hand keep their category.`);
      suggest = null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function resetCat(t: TxnRow) {
    try {
      await api.resetCategory([t.id]);
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function bulkApply() {
    if (!bulkCat || !selected.length) return;
    try {
      const n = await api.setCategory(selected, bulkCat);
      app.info(`${n} transactions updated.`);
      selected = [];
      bulkCat = "";
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function saveNote() {
    if (!noteEdit) return;
    try {
      await api.setNote(noteEdit.id, noteEdit.text);
      noteEdit = null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  function toggle(id: number) {
    selected = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
  }

  function toggleAll() {
    if (!data) return;
    const ids = data.rows.map((r) => r.id);
    selected = ids.every((id) => selected.includes(id)) ? [] : ids;
  }
</script>

<div class="row head">
  <h1>Transactions</h1>
  {#if data}
    <span class="muted">{data.total} found · out {inr(data.debits)} · in {inr(data.credits)}</span>
  {/if}
</div>

<div class="card filters row">
  <input type="search" placeholder="Search narration, payee, note or amount" bind:value={search} style="width:300px" />
  <select bind:value={category}>
    <option value={null}>All categories</option>
    {#each sortedCats as c (c.id)}
      <option value={c.id}>{c.name} ({KIND_LABEL[c.kind]})</option>
    {/each}
  </select>
  <select bind:value={kind}>
    <option value={null}>All types</option>
    <option value="expense">Expenses</option>
    <option value="income">Income</option>
    <option value="investment">Investments</option>
    <option value="excluded">Excluded</option>
  </select>
  <select bind:value={direction}>
    <option value={null}>Money in and out</option>
    <option value="debit">Money out</option>
    <option value="credit">Money in</option>
  </select>
  {#if month}
    <span class="chip">{monthLabel(month)} <button class="link" onclick={() => (month = null)}>×</button></span>
  {/if}
  <select bind:value={sort}>
    <option value="date_desc">Newest first</option>
    <option value="date_asc">Oldest first</option>
    <option value="amount_desc">Largest first</option>
    <option value="amount_asc">Smallest first</option>
  </select>
  <button class="link" onclick={clearFilters}>Clear filters</button>
</div>

{#if suggest}
  <div class="banner row" style="margin-top:12px">
    <span>Always categorise transactions containing</span>
    <input type="text" bind:value={suggest.pattern} oninput={checkSuggest} style="width:200px" />
    <span>as <strong>{app.categoryName(suggest.category)}</strong>? ({suggest.matches} match)</span>
    <button class="primary" onclick={createRule}>Create rule</button>
    <button class="link" onclick={() => (suggest = null)}>No thanks</button>
  </div>
{/if}

{#if selected.length}
  <div class="banner row" style="margin-top:12px">
    <span>{selected.length} selected. Set category:</span>
    <select bind:value={bulkCat}>
      <option value="">Choose…</option>
      {#each sortedCats as c (c.id)}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
    <button class="primary" disabled={!bulkCat} onclick={bulkApply}>Apply</button>
    <button class="link" onclick={() => (selected = [])}>Clear selection</button>
  </div>
{/if}

<div class="card table" style="margin-top:12px">
  {#if data && data.rows.length}
    <table>
      <thead>
        <tr>
          <th><input type="checkbox" onchange={toggleAll} checked={data.rows.every((r) => selected.includes(r.id))} /></th>
          <th>Date</th>
          <th>Account</th>
          <th>Details</th>
          <th class="num">Money out</th>
          <th class="num">Money in</th>
          <th class="num">Balance</th>
          <th>Category</th>
        </tr>
      </thead>
      <tbody>
        {#each data.rows as t (t.id)}
          <tr>
            <td><input type="checkbox" checked={selected.includes(t.id)} onchange={() => toggle(t.id)} /></td>
            <td class="nowrap">{dmy(t.date)}</td>
            <td class="nowrap muted">{t.account}</td>
            <td class="details">
              <div><strong>{t.merchant || "—"}</strong> <span class="chip">{t.channel}</span></div>
              <div class="muted small narr" title={t.narration}>{t.narration}</div>
              {#if t.note}<div class="small note">📝 {t.note}</div>{/if}
              <button class="link small" onclick={() => (noteEdit = { id: t.id, text: t.note })}>
                {t.note ? "Edit note" : "Add note"}
              </button>
            </td>
            <td class="num bad">{t.direction === "debit" ? inr(t.amount) : ""}</td>
            <td class="num good">{t.direction === "credit" ? inr(t.amount) : ""}</td>
            <td class="num muted">{inr(t.balance)}</td>
            <td>
              <span class="dot" style="background:{t.color}"></span>
              <select value={t.category} onchange={(e) => changeCategory(t, e.currentTarget.value)} title={SOURCE_LABEL[t.source]}>
                {#each sortedCats as c (c.id)}
                  <option value={c.id}>{c.name}</option>
                {/each}
              </select>
              {#if t.source === "user"}
                <button class="link small" title="Let the rules decide again" onclick={() => resetCat(t)}>↺</button>
              {:else if t.source === "transfer"}
                <div class="muted small">auto-matched transfer</div>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if pages > 1}
      <div class="row pager">
        <button disabled={pageNo === 0} onclick={() => pageNo--}>‹ Previous</button>
        <span class="muted">Page {pageNo + 1} of {pages}</span>
        <button disabled={pageNo + 1 >= pages} onclick={() => pageNo++}>Next ›</button>
      </div>
    {/if}
  {:else if data}
    <div class="empty">No transactions match.</div>
  {/if}
</div>

{#if noteEdit}
  <div class="modal-back" role="presentation" onclick={(e) => e.target === e.currentTarget && (noteEdit = null)}>
    <div class="modal">
      <h2>Note</h2>
      <textarea bind:value={noteEdit.text} rows="3" style="width:100%"></textarea>
      <div class="row" style="margin-top:12px">
        <span class="spacer"></span>
        <button onclick={() => (noteEdit = null)}>Cancel</button>
        <button class="primary" onclick={saveNote}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .head {
    align-items: baseline;
    gap: 16px;
  }

  .filters {
    gap: 10px;
  }

  .table {
    padding: 0;
    overflow: auto;
  }

  .nowrap {
    white-space: nowrap;
  }

  .details {
    max-width: 520px;
  }

  .narr {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 520px;
  }

  .note {
    margin-top: 2px;
  }

  td select {
    max-width: 170px;
  }

  .pager {
    justify-content: center;
    padding: 12px;
  }
</style>
