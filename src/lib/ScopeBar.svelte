<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  import { PRESETS } from "./format";
  import { app } from "./state.svelte";

  let exporting = $state(false);

  async function exportExcel() {
    const scope = app.scope;
    if (!scope) return;
    const today = new Date().toISOString().slice(0, 10);
    const dest = await save({
      defaultPath: `Bahi-Khata report ${today}.xlsx`,
      filters: [{ name: "Excel workbook", extensions: ["xlsx"] }],
    });
    if (!dest) return;
    exporting = true;
    try {
      await api.exportExcel(dest, scope, `${app.scopeLabel} · ${app.periodLabel}`);
      app.info("Excel report saved.");
    } catch (e) {
      app.error(e);
    } finally {
      exporting = false;
    }
  }
</script>

<div class="bar row">
  {#if app.scopeOptions.length}
    <label class="row">
      <span class="muted small">Accounts</span>
      <select bind:value={app.scopeIndex}>
        {#each app.scopeOptions as o, i (i)}
          <option value={i}>{o.label}</option>
        {/each}
      </select>
    </label>
    <label class="row">
      <span class="muted small">Period</span>
      <select value={app.preset} onchange={(e) => app.setPreset(e.currentTarget.value)}>
        {#each PRESETS as p (p.id)}
          <option value={p.id}>{p.label}</option>
        {/each}
      </select>
    </label>
    {#if app.preset === "custom"}
      <input type="date" bind:value={app.from} />
      <span class="muted">to</span>
      <input type="date" bind:value={app.to} />
    {:else if app.from || app.to}
      <span class="muted small">{app.from ?? "…"} → {app.to ?? "…"}</span>
    {/if}
    <span class="spacer"></span>
    <button onclick={exportExcel} disabled={exporting}>{exporting ? "Exporting…" : "Export to Excel"}</button>
  {:else}
    <span class="muted">No statements imported yet.</span>
    <button class="primary" onclick={() => (app.page = "import")}>Import a statement</button>
  {/if}
</div>

<style>
  .bar {
    padding: 10px 24px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    gap: 16px;
  }

  label.row {
    gap: 6px;
  }

  select {
    max-width: 420px;
  }
</style>
