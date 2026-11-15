<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errText } from "../lib/api";

  let { exists, onunlocked }: { exists: boolean; onunlocked: () => void } = $props();

  let password = $state("");
  let confirm = $state("");
  let busy = $state(false);
  let error = $state("");
  let restoring = $state(false);
  let restorePath = $state("");

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!exists && !restoring && password !== confirm) {
      error = "The two passwords do not match.";
      return;
    }
    busy = true;
    try {
      if (restoring) await api.restoreVault(restorePath, password);
      else if (exists) await api.unlock(password);
      else await api.createVault(password);
      password = confirm = "";
      onunlocked();
    } catch (err) {
      error = errText(err);
    } finally {
      busy = false;
    }
  }

  async function pickBackup() {
    const p = await open({ multiple: false, filters: [{ name: "Bahi-Khata backup", extensions: ["bkv"] }] });
    if (typeof p === "string") {
      restorePath = p;
      restoring = true;
      error = "";
    }
  }
</script>

<div class="wrap">
  <form class="card box" onsubmit={submit}>
    <div class="brand">बही-खाता <span>Bahi-Khata</span></div>
    {#if restoring}
      <h2>Restore from backup</h2>
      <p class="muted small">{restorePath}</p>
      <p class="muted">Enter the password that was in use when this backup was made.</p>
    {:else if exists}
      <h2>Unlock</h2>
    {:else}
      <h2>Set up</h2>
      <p class="muted">
        Choose a password (at least 8 characters). All your data is encrypted with it and stays on this computer.
        <strong>If you forget it, the data cannot be recovered.</strong>
      </p>
    {/if}

    <label class="field">
      <span>Password</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="password" bind:value={password} autofocus autocomplete="off" />
    </label>
    {#if !exists && !restoring}
      <label class="field">
        <span>Confirm password</span>
        <input type="password" bind:value={confirm} autocomplete="off" />
      </label>
    {/if}

    {#if error}<p class="bad">{error}</p>{/if}

    <div class="row">
      <button class="primary" type="submit" disabled={busy || !password}>
        {busy ? "Please wait…" : restoring ? "Restore" : exists ? "Unlock" : "Create"}
      </button>
      <span class="spacer"></span>
      {#if restoring}
        <button type="button" class="link" onclick={() => (restoring = false)}>Cancel</button>
      {:else if !exists}
        <button type="button" class="link" onclick={pickBackup}>Restore a backup…</button>
      {/if}
    </div>
  </form>
</div>

<style>
  .wrap {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .box {
    width: 420px;
    padding: 28px;
  }

  .brand {
    font-size: 26px;
    font-weight: 600;
    color: var(--accent);
    margin-bottom: 20px;
  }

  .brand span {
    font-size: 14px;
    color: var(--muted);
    font-weight: 400;
    margin-left: 6px;
  }

  input {
    width: 100%;
  }
</style>
