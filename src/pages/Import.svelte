<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";
  import { api, type ImportPreview, type ImportRow } from "../lib/api";
  import { inr, dmy } from "../lib/format";
  import { app } from "../lib/state.svelte";

  let preview = $state.raw<ImportPreview | null>(null);
  let holder = $state("");
  let busy = $state(false);
  let error = $state("");
  let history = $state.raw<ImportRow[]>([]);
  let dragging = $state(false);
  let queue = $state<string[]>([]);
  let showAll = $state(false);

  const EXT = ["xls", "xlsx", "csv", "txt", "pdf"];

  async function loadHistory() {
    try {
      history = await api.imports();
    } catch (e) {
      app.error(e);
    }
  }

  async function startFile(path: string) {
    error = "";
    preview = null;
    holder = "";
    showAll = false;
    busy = true;
    try {
      preview = await api.previewImport(path);
    } catch (e) {
      error = `${path.split(/[\\/]/).pop()}: ${e}`;
      next();
    } finally {
      busy = false;
    }
  }

  function next() {
    const [p, ...rest] = queue;
    queue = rest;
    if (p) startFile(p);
  }

  function enqueue(paths: string[]) {
    const ok = paths.filter((p) => EXT.includes(p.split(".").pop()?.toLowerCase() ?? ""));
    if (!ok.length) {
      error = "Please choose an Excel (XLS/XLSX) or CSV statement file.";
      return;
    }
    queue = [...queue, ...ok];
    if (!preview && !busy) next();
  }

  async function choose() {
    const p = await open({
      multiple: true,
      filters: [{ name: "Bank statements", extensions: EXT }],
    });
    if (Array.isArray(p)) enqueue(p);
    else if (typeof p === "string") enqueue([p]);
  }

  async function confirm() {
    if (!preview) return;
    busy = true;
    error = "";
    try {
      const r = await api.commitImport(preview.holder_found ? null : holder);
      app.info(`Imported ${r.added} new transactions${r.skipped ? `, skipped ${r.skipped} already imported` : ""}.`);
      preview = null;
      await app.changed();
      await loadHistory();
      if (!queue.length && app.review.uncategorized > 0) {
        app.info(`Imported. ${app.review.uncategorized} transactions need a category; please review them.`);
        app.page = "review";
      } else next();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function cancel() {
    await api.cancelImport();
    preview = null;
    next();
  }

  async function undo(i: ImportRow) {
    if (!window.confirm(`Remove the ${i.added} transactions added from "${i.file_name}"?`)) return;
    try {
      const n = await api.deleteImport(i.id);
      app.info(`${n} transactions removed.`);
      await app.changed();
      await loadHistory();
    } catch (e) {
      app.error(e);
    }
  }

  onMount(() => {
    loadHistory();
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "enter" || p.type === "over") dragging = true;
        else if (p.type === "leave") dragging = false;
        else if (p.type === "drop") {
          dragging = false;
          enqueue(p.paths);
        }
      })
      .then((u) => (unlisten = u));
    return () => unlisten?.();
  });

  const rows = $derived(preview ? (showAll ? preview.rows : preview.rows.slice(0, 50)) : []);
</script>

<h1>Import statement</h1>

{#if !preview}
  <button class="card drop" class:dragging onclick={choose} disabled={busy}>
    {#if busy}
      Reading the statement…
    {:else}
      <div class="big">⇪</div>
      <div><strong>Drop bank statement files here</strong> or click to choose</div>
      <div class="muted small">SBI or HDFC statements in Excel (XLS / XLSX) or CSV format, as downloaded from net banking</div>
    {/if}
  </button>
{/if}

{#if error}<p class="bad">{error}</p>{/if}

{#if preview}
  <div class="card">
    <div class="row">
      <h2 style="margin:0">{preview.file_name}</h2>
      <span class="spacer"></span>
      {#if queue.length}<span class="muted small">{queue.length} more file(s) waiting</span>{/if}
    </div>
    <div class="facts">
      <div><span class="muted">Bank</span>{preview.bank}</div>
      <div><span class="muted">Account</span>{preview.account_number || "—"}</div>
      <div><span class="muted">Holder</span>{preview.holder || "Not shown in the file"}</div>
      <div><span class="muted">Period</span>{dmy(preview.period_from)} to {dmy(preview.period_to)}</div>
      <div>
        <span class="muted">Into</span>{preview.existing_account ? preview.existing_label : "A new account"}
      </div>
      <div>
        <span class="muted">Transactions</span>{preview.new_count} new{preview.duplicates ? `, ${preview.duplicates} already imported` : ""}
      </div>
      <div><span class="muted">Money out</span><span class="bad">{inr(preview.debits)}</span></div>
      <div><span class="muted">Money in</span><span class="good">{inr(preview.credits)}</span></div>
    </div>

    {#each preview.warnings as w, i (i)}
      <div class="banner" style="margin-bottom:8px">{w}</div>
    {/each}

    {#if !preview.holder_found}
      <label class="field" style="max-width:420px">
        <span>The statement does not show the account holder's name. Please enter it (used to group your accounts):</span>
        <input type="text" bind:value={holder} placeholder="e.g. Harsh Sharma" />
      </label>
    {/if}

    <div class="row" style="margin:12px 0">
      <button class="primary" onclick={confirm} disabled={busy || preview.new_count === 0 || (!preview.holder_found && !holder.trim())}>
        {preview.new_count === 0 ? "Nothing new to import" : `Import ${preview.new_count} transactions`}
      </button>
      <button onclick={cancel} disabled={busy}>Cancel</button>
    </div>

    <div class="scroll">
      <table>
        <thead>
          <tr><th>Date</th><th>Narration</th><th class="num">Out</th><th class="num">In</th><th>Category</th><th></th></tr>
        </thead>
        <tbody>
          {#each rows as r, i (i)}
            <tr class:dup={r.duplicate}>
              <td>{dmy(r.date)}</td>
              <td class="small">{r.narration}</td>
              <td class="num">{r.direction === "debit" ? inr(r.amount) : ""}</td>
              <td class="num">{r.direction === "credit" ? inr(r.amount) : ""}</td>
              <td>{app.categoryName(r.category)}</td>
              <td class="muted small">{r.duplicate ? "already imported" : ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if !showAll && preview.rows.length > 50}
        <button class="link" onclick={() => (showAll = true)}>Show all {preview.rows.length} rows</button>
      {/if}
    </div>
  </div>
{/if}

<div class="card" style="margin-top:20px">
  <h2>Imported files</h2>
  {#if history.length}
    <table>
      <thead>
        <tr><th>Imported</th><th>File</th><th>Account</th><th>Period</th><th class="num">Added</th><th class="num">Skipped</th><th></th></tr>
      </thead>
      <tbody>
        {#each history as i (i.id)}
          <tr>
            <td class="muted">{i.imported_at}</td>
            <td>{i.file_name}</td>
            <td>{i.account}</td>
            <td>{dmy(i.period_from)} – {dmy(i.period_to)}</td>
            <td class="num">{i.added}</td>
            <td class="num">{i.skipped}</td>
            <td><button class="link danger" onclick={() => undo(i)}>Undo</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">Nothing imported yet.</p>
  {/if}
</div>

<div class="card muted small" style="margin-top:20px">
  <h3>Where to get the statement</h3>
  <p><strong>SBI:</strong> OnlineSBI → My Accounts → Account Statement → choose the period → download as Excel.</p>
  <p><strong>HDFC:</strong> NetBanking → Accounts → Account Statement → choose the period → download as Delimited (CSV) or Excel.</p>
  <p>Overlapping periods are fine: transactions already imported are skipped automatically.</p>
</div>

<style>
  .drop {
    width: 100%;
    padding: 48px 16px;
    text-align: center;
    border: 2px dashed var(--line);
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
  }

  .drop.dragging,
  .drop:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .big {
    font-size: 36px;
    color: var(--accent);
  }

  .facts {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px 20px;
    margin: 16px 0;
  }

  .facts div {
    display: flex;
    flex-direction: column;
  }

  .facts .muted {
    font-size: 12px;
  }

  .scroll {
    max-height: 420px;
    overflow: auto;
  }

  tr.dup td {
    color: var(--muted);
    text-decoration: line-through;
  }

  button.link.danger {
    color: var(--bad);
  }
</style>
