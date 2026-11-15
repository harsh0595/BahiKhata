<script lang="ts">
  import { api, type CatKind, type ReviewGroup } from "../lib/api";
  import { inr, dmy, KIND_LABEL } from "../lib/format";
  import { app } from "../lib/state.svelte";
  import { untrack } from "svelte";

  const STEP = 25;
  const KIND_ORDER: CatKind[] = ["expense", "income", "investment", "excluded"];

  let tab = $state<"uncategorized" | "guessed">(app.review.uncategorized ? "uncategorized" : "guessed");
  let groups = $state.raw<ReviewGroup[]>([]);
  let loaded = $state(false);
  let shown = $state(STEP);
  let skipped = $state<string[]>([]);
  let choice = $state<Record<string, { category: string; remember: boolean }>>({});
  let busy = $state<string | null>(null);

  async function load() {
    try {
      const g = await api.reviewGroups(tab === "guessed");
      const c: Record<string, { category: string; remember: boolean }> = {};
      for (const x of g) {
        c[x.key] = choice[x.key] ?? { category: tab === "guessed" ? x.category : "", remember: x.pattern !== "" };
      }
      choice = c;
      groups = g;
      loaded = true;
    } catch (e) {
      app.error(e);
    }
  }

  $effect(() => {
    void [tab, app.version];
    untrack(load);
  });

  function switchTab(t: typeof tab) {
    tab = t;
    shown = STEP;
    skipped = [];
    choice = {};
    loaded = false;
  }

  const visible = $derived(groups.filter((g) => !skipped.includes(g.key)));
  const byKind = $derived(
    KIND_ORDER.map((k) => ({
      kind: k,
      cats: app.categories.filter((c) => c.kind === k).sort((a, b) => a.name.localeCompare(b.name)),
    })).filter((x) => x.cats.length),
  );

  async function save(g: ReviewGroup) {
    const c = choice[g.key];
    if (!c?.category) return;
    busy = g.key;
    try {
      const pattern = c.remember && g.pattern ? g.pattern : null;
      const n = await api.assignReviewGroup(g.txn_ids, c.category, pattern, g.direction);
      app.info(`${n} transaction${n === 1 ? "" : "s"} set to ${app.categoryName(c.category)}${pattern ? "; future ones will be too" : ""}.`);
      await app.changed();
    } catch (e) {
      app.error(e);
    } finally {
      busy = null;
    }
  }

  function openTxns(g: ReviewGroup) {
    app.showTxns({ search: g.pattern || g.label, source: tab === "guessed" ? "auto" : "default" });
  }
</script>

<div class="row head">
  <h1>Review</h1>
  <span class="muted">Answer once per payee; the app remembers it for next time.</span>
</div>

<div class="row tabs">
  <button class:active={tab === "uncategorized"} onclick={() => switchTab("uncategorized")}>
    Needs a category ({app.review.uncategorized})
  </button>
  <button class:active={tab === "guessed"} onclick={() => switchTab("guessed")}>
    Auto-guessed ({app.review.guessed})
  </button>
</div>

{#if tab === "guessed"}
  <p class="muted small">
    These were guessed from the amount (small UPI payments) or from earlier payments to the same payee. Confirm or change
    them; anything you leave stays as guessed.
  </p>
{:else}
  <p class="muted small">No rule matched these. They are counted as "Other" until you choose a category.</p>
{/if}

{#if loaded && !visible.length}
  <div class="card empty">Nothing to review here. 🎉</div>
{/if}

{#each visible.slice(0, shown) as g (g.key)}
  <div class="card group">
    <div class="row top">
      <div class="who">
        <strong>{g.label || "Unknown payee"}</strong>
        <span class="chip">{g.direction === "debit" ? "Money out" : "Money in"}</span>
        {#if tab === "guessed"}<span class="chip">guessed: {app.categoryName(g.category)}</span>{/if}
      </div>
      <span class="spacer"></span>
      <span class={g.direction === "debit" ? "bad" : "good"}><strong>{inr(g.total)}</strong></span>
    </div>
    <div class="muted small">
      {g.count} transaction{g.count === 1 ? "" : "s"} · {dmy(g.first)}{g.first !== g.last ? ` – ${dmy(g.last)}` : ""}
      · <button class="link small" onclick={() => openTxns(g)}>view</button>
    </div>
    {#each g.samples as s}
      <div class="muted small narr" title={s}>{s}</div>
    {/each}
    {#if choice[g.key]}
      <div class="row actions">
        <select bind:value={choice[g.key].category}>
          <option value="">Choose a category…</option>
          {#each byKind as k (k.kind)}
            <optgroup label={KIND_LABEL[k.kind]}>
              {#each k.cats as cat (cat.id)}
                <option value={cat.id}>{cat.name}</option>
              {/each}
            </optgroup>
          {/each}
        </select>
        <label class="check" title={g.pattern ? `Rule: transactions containing "${g.pattern}"` : "No stable payee text to remember"}>
          <input type="checkbox" bind:checked={choice[g.key].remember} disabled={!g.pattern} />
          Remember for this payee
        </label>
        <span class="spacer"></span>
        <button onclick={() => (skipped = [...skipped, g.key])}>Skip</button>
        <button class="primary" disabled={!choice[g.key].category || busy !== null} onclick={() => save(g)}>
          {busy === g.key ? "Saving…" : tab === "guessed" && choice[g.key].category === g.category ? "Confirm" : "Save"}
        </button>
      </div>
    {/if}
  </div>
{/each}

{#if visible.length > shown}
  <div class="row more">
    <button onclick={() => (shown += STEP)}>Show more ({visible.length - shown} left)</button>
  </div>
{/if}

<style>
  .head {
    align-items: baseline;
    gap: 16px;
  }

  .tabs {
    gap: 8px;
    margin-bottom: 8px;
  }

  .tabs button.active {
    background: var(--accent, #3949ab);
    color: #fff;
    border-color: transparent;
  }

  .group {
    margin-top: 10px;
  }

  .top {
    align-items: baseline;
  }

  .who {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .narr {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 760px;
  }

  .actions {
    margin-top: 8px;
    gap: 12px;
  }

  .actions select {
    min-width: 220px;
  }

  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .more {
    justify-content: center;
    margin-top: 12px;
  }
</style>
