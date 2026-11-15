<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { app, type Page } from "./lib/state.svelte";
  import ScopeBar from "./lib/ScopeBar.svelte";
  import Lock from "./pages/Lock.svelte";
  import Dashboard from "./pages/Dashboard.svelte";
  import Transactions from "./pages/Transactions.svelte";
  import Import from "./pages/Import.svelte";
  import Accounts from "./pages/Accounts.svelte";
  import Categories from "./pages/Categories.svelte";
  import SettingsPage from "./pages/Settings.svelte";
  import Report from "./pages/Report.svelte";
  import Review from "./pages/Review.svelte";

  let ready = $state(false);
  let exists = $state(false);
  let version = $state("");
  let autoLock = $state(15);
  let lastActivity = Date.now();

  const NAV: { id: Page; label: string; icon: string }[] = [
    { id: "dashboard", label: "Dashboard", icon: "▦" },
    { id: "transactions", label: "Transactions", icon: "≡" },
    { id: "review", label: "Review", icon: "✎" },
    { id: "report", label: "Report", icon: "⎙" },
    { id: "import", label: "Import statement", icon: "⇪" },
    { id: "accounts", label: "Accounts", icon: "◫" },
    { id: "categories", label: "Categories & rules", icon: "◈" },
    { id: "settings", label: "Settings", icon: "⚙" },
  ];

  async function refreshStatus() {
    const s = await api.status();
    exists = s.exists;
    version = s.version;
    autoLock = s.auto_lock_minutes;
    app.unlocked = s.unlocked;
    if (s.unlocked) await app.loadMeta();
  }

  async function onUnlocked() {
    await refreshStatus();
    app.page = app.scopeOptions.length ? "dashboard" : "import";
    lastActivity = Date.now();
  }

  async function lockNow() {
    await api.lock();
    app.reset();
    await refreshStatus();
  }

  onMount(() => {
    refreshStatus().finally(() => (ready = true));
    const touch = () => (lastActivity = Date.now());
    const evs = ["mousemove", "keydown", "mousedown", "wheel"] as const;
    evs.forEach((e) => window.addEventListener(e, touch, { passive: true }));
    const timer = setInterval(() => {
      if (app.unlocked && Date.now() - lastActivity > autoLock * 60_000) lockNow();
    }, 15_000);
    return () => {
      evs.forEach((e) => window.removeEventListener(e, touch));
      clearInterval(timer);
    };
  });

  $effect(() => {
    // Pick up a changed auto-lock setting.
    void app.version;
    if (app.unlocked) api.status().then((s) => (autoLock = s.auto_lock_minutes));
  });

  const showScope = $derived(app.page === "dashboard" || app.page === "transactions" || app.page === "report");
</script>

{#if !ready}
  <div class="empty">Loading…</div>
{:else if !app.unlocked}
  <Lock {exists} onunlocked={onUnlocked} />
{:else}
  <div class="shell">
    <nav class="no-print">
      <div class="brand">बही-खाता</div>
      {#each NAV as n (n.id)}
        <button class:active={app.page === n.id} onclick={() => (app.page = n.id)}>
          <span class="icon">{n.icon}</span>{n.label}
          {#if n.id === "review" && app.review.uncategorized_groups > 0}<span class="badge">{app.review.uncategorized_groups}</span>{/if}
        </button>
      {/each}
      <div class="spacer"></div>
      <button onclick={lockNow}><span class="icon">🔒</span>Lock</button>
      <div class="ver muted small">v{version} · offline</div>
    </nav>
    <main>
      {#if showScope}
        <div class="no-print"><ScopeBar /></div>
      {/if}
      <div class="content">
        {#if app.page === "dashboard"}
          <Dashboard />
        {:else if app.page === "transactions"}
          <Transactions />
        {:else if app.page === "review"}
          <Review />
        {:else if app.page === "report"}
          <Report />
        {:else if app.page === "import"}
          <Import />
        {:else if app.page === "accounts"}
          <Accounts />
        {:else if app.page === "categories"}
          <Categories />
        {:else if app.page === "settings"}
          <SettingsPage {version} onlocked={lockNow} />
        {/if}
      </div>
    </main>
  </div>
{/if}

{#if app.toast}
  <div class="toast no-print" class:err={app.toast.error}>{app.toast.text}</div>
{/if}

<style>
  .shell {
    display: flex;
    height: 100%;
  }

  nav {
    width: 210px;
    flex-shrink: 0;
    background: #1f2547;
    color: #dfe2f5;
    display: flex;
    flex-direction: column;
    padding: 12px 8px;
    gap: 2px;
  }

  .brand {
    font-size: 22px;
    font-weight: 600;
    padding: 8px 12px 18px;
    color: #fff;
  }

  nav button {
    background: none;
    border: none;
    color: inherit;
    text-align: left;
    padding: 9px 12px;
    border-radius: 6px;
  }

  nav button:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  nav button.active {
    background: rgba(255, 255, 255, 0.16);
    color: #fff;
  }

  .badge {
    float: right;
    background: #ffb300;
    color: #1f2547;
    border-radius: 10px;
    padding: 0 7px;
    font-size: 12px;
    font-weight: 600;
  }

  .icon {
    display: inline-block;
    width: 22px;
  }

  .spacer {
    flex: 1;
  }

  .ver {
    padding: 8px 12px 0;
    color: #9da3c7;
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .content {
    flex: 1;
    overflow: auto;
    padding: 20px 24px 40px;
  }

  .toast {
    position: fixed;
    right: 20px;
    bottom: 20px;
    background: #263238;
    color: #fff;
    padding: 10px 16px;
    border-radius: 6px;
    max-width: 480px;
    z-index: 100;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  }

  .toast.err {
    background: var(--bad);
  }

  @media print {
    .shell,
    main,
    .content {
      display: block;
      height: auto;
      overflow: visible;
      padding: 0;
    }
  }
</style>
