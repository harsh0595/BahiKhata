<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type Settings, type Status } from "../lib/api";
  import { app } from "../lib/state.svelte";

  let { version, onlocked }: { version: string; onlocked: () => void } = $props();

  let settings = $state<Settings | null>(null);
  let status = $state.raw<Status | null>(null);
  let pw = $state({ current: "", next: "", confirm: "" });
  let pwBusy = $state(false);
  let restore = $state<{ path: string; password: string } | null>(null);
  let wipe = $state<string | null>(null);

  $effect(() => {
    void app.version;
    api.getSettings().then((s) => (settings = s)).catch((e) => app.error(e));
    api.status().then((s) => (status = s));
  });

  async function saveSettings() {
    if (!settings) return;
    try {
      await api.setSettings($state.snapshot(settings));
      app.info("Settings saved.");
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function changePassword() {
    if (pw.next !== pw.confirm) {
      app.error("The new passwords do not match.");
      return;
    }
    pwBusy = true;
    try {
      await api.changePassword(pw.current, pw.next);
      pw = { current: "", next: "", confirm: "" };
      app.info("Password changed.");
    } catch (e) {
      app.error(e);
    } finally {
      pwBusy = false;
    }
  }

  async function backup() {
    const today = new Date().toISOString().slice(0, 10);
    const dest = await save({
      defaultPath: `Bahi-Khata backup ${today}.bkv`,
      filters: [{ name: "Bahi-Khata backup", extensions: ["bkv"] }],
    });
    if (!dest) return;
    try {
      await api.backupVault(dest);
      app.info("Backup saved. It is encrypted with your current password.");
    } catch (e) {
      app.error(e);
    }
  }

  async function pickRestore() {
    const p = await open({ multiple: false, filters: [{ name: "Bahi-Khata backup", extensions: ["bkv"] }] });
    if (typeof p === "string") restore = { path: p, password: "" };
  }

  async function doRestore() {
    if (!restore) return;
    try {
      await api.restoreVault(restore.path, restore.password);
      restore = null;
      app.info("Backup restored.");
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function doWipe() {
    try {
      await api.deleteAll(wipe ?? "");
      wipe = null;
      onlocked();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<h1>Settings</h1>

<div class="grid cols">
  <div class="card">
    <h2>Reports</h2>
    {#if settings}
      <label class="check">
        <input type="checkbox" bind:checked={settings.investments_as_expense} onchange={saveSettings} />
        Count investments (SIPs, stocks, PPF, insurance…) as expenses
      </label>
      <p class="muted small">
        Off: investments are shown separately and do not reduce the savings rate. Net savings = income − expenses − investments
        either way.
      </p>
      <label class="field" style="max-width:240px;margin-top:16px">
        <span>Lock automatically after (minutes of inactivity)</span>
        <input type="number" min="1" max="240" bind:value={settings.auto_lock_minutes} onchange={saveSettings} />
      </label>
    {/if}
  </div>

  <div class="card">
    <h2>Automatic guesses</h2>
    {#if settings}
      <label class="field" style="max-width:260px">
        <span>Small UPI payments up to (₹, 0 = off)</span>
        <input
          type="number"
          min="0"
          step="10"
          value={settings.small_upi_max / 100}
          onchange={(e) => {
            if (!settings) return;
            settings.small_upi_max = Math.max(0, Math.round(Number(e.currentTarget.value) * 100));
            saveSettings();
          }}
        />
      </label>
      <label class="field" style="max-width:260px">
        <span>are guessed as</span>
        <select bind:value={settings.small_upi_category} onchange={saveSettings}>
          {#each app.categories.filter((c) => c.kind === "expense") as c (c.id)}
            <option value={c.id}>{c.name}</option>
          {/each}
        </select>
      </label>
      <p class="muted small">
        For UPI payments that match no rule, such as street vegetable and fruit vendors. Guesses are marked "auto-guessed"
        and can be checked on the Review page.
      </p>
    {/if}
  </div>

  <div class="card">
    <h2>Change password</h2>
    <label class="field"><span>Current password</span><input type="password" bind:value={pw.current} /></label>
    <label class="field"><span>New password (at least 8 characters)</span><input type="password" bind:value={pw.next} /></label>
    <label class="field"><span>Confirm new password</span><input type="password" bind:value={pw.confirm} /></label>
    <button class="primary" disabled={pwBusy || !pw.current || !pw.next} onclick={changePassword}>
      {pwBusy ? "Please wait…" : "Change password"}
    </button>
    <p class="muted small">Older backups still need the password that was in use when they were made.</p>
  </div>

  <div class="card">
    <h2>Backup</h2>
    <p class="muted small">
      A backup is a copy of your encrypted data file. Keep it on a USB drive or cloud folder; it cannot be read without your
      password.
    </p>
    <div class="row">
      <button onclick={backup}>Save a backup…</button>
      <button onclick={pickRestore}>Restore a backup…</button>
    </div>
    {#if restore}
      <div class="banner" style="margin-top:12px">
        <p>Restoring replaces <strong>all</strong> current data with <span class="small">{restore.path}</span>.</p>
        <label class="field">
          <span>Password of the backup</span>
          <input type="password" bind:value={restore.password} />
        </label>
        <div class="row">
          <button class="primary" disabled={!restore.password} onclick={doRestore}>Restore</button>
          <button onclick={() => (restore = null)}>Cancel</button>
        </div>
      </div>
    {/if}
  </div>

  <div class="card">
    <h2>Delete all data</h2>
    <p class="muted small">Permanently deletes every account, transaction, rule and setting from this computer.</p>
    {#if wipe === null}
      <button class="danger" onclick={() => (wipe = "")}>Delete all data…</button>
    {:else}
      <label class="field">
        <span>Type DELETE to confirm</span>
        <input type="text" bind:value={wipe} />
      </label>
      <div class="row">
        <button class="danger solid" disabled={wipe !== "DELETE"} onclick={doWipe}>Delete everything</button>
        <button onclick={() => (wipe = null)}>Cancel</button>
      </div>
    {/if}
  </div>

  <div class="card">
    <h2>About</h2>
    <p>Bahi-Khata v{version}</p>
    <p class="muted small">
      Works fully offline: nothing is ever sent over the internet. Data is encrypted (Argon2id + XChaCha20-Poly1305) and stored
      only on this computer.
    </p>
    {#if status}<p class="muted small">Data file: {status.path}</p>{/if}
  </div>
</div>

<style>
  .cols {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: start;
  }

  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  input[type="password"],
  input[type="text"] {
    width: 100%;
  }
</style>
