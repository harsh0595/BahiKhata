<script lang="ts">
  import { api, type CatKind, type Category, type Direction, type Rule } from "../lib/api";
  import { KIND_LABEL } from "../lib/format";
  import { app } from "../lib/state.svelte";

  const KINDS: CatKind[] = ["expense", "income", "investment", "excluded"];
  const KIND_HELP: Record<CatKind, string> = {
    expense: "Counted as spending.",
    income: "Counted as income.",
    investment: "Money put into savings or investments. Shown separately (or as expense, see Settings).",
    excluded: "Ignored in totals, e.g. transfers between your own accounts and credit-card bill payments.",
  };

  let rules = $state.raw<Rule[]>([]);
  let edit = $state<{ id: string | null; name: string; color: string; kind: CatKind; builtin: boolean } | null>(null);
  let rule = $state<{ pattern: string; category: string; direction: Direction | null; matches: number | null }>({
    pattern: "",
    category: "",
    direction: null,
    matches: null,
  });

  $effect(() => {
    void app.version;
    api.rules().then((r) => (rules = r)).catch((e) => app.error(e));
  });

  const byKind = $derived(
    KINDS.map((k) => ({ kind: k, cats: app.categories.filter((c) => c.kind === k).sort((a, b) => a.name.localeCompare(b.name)) })),
  );

  function newCat() {
    edit = { id: null, name: "", color: "#607d8b", kind: "expense", builtin: false };
  }

  function editCat(c: Category) {
    edit = { id: c.id, name: c.name, color: c.color, kind: c.kind, builtin: c.builtin };
  }

  async function saveCat() {
    if (!edit) return;
    try {
      if (edit.id) await api.updateCategory(edit.id, edit.name, edit.color, edit.builtin ? null : edit.kind);
      else await api.addCategory(edit.name, edit.kind, edit.color);
      edit = null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function deleteCat() {
    if (!edit?.id) return;
    if (!window.confirm(`Delete "${edit.name}"? Its rules are removed and its transactions are categorised again.`)) return;
    try {
      await api.deleteCategory(edit.id);
      edit = null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  let t: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const p = rule.pattern;
    const d = rule.direction;
    clearTimeout(t);
    if (p.trim().length < 3) {
      rule.matches = null;
      return;
    }
    t = setTimeout(() => api.ruleMatches(p, d).then((n) => (rule.matches = n)), 250);
  });

  async function addRule() {
    try {
      await api.addRule(rule.pattern, rule.category, rule.direction);
      rule = { pattern: "", category: rule.category, direction: null, matches: null };
      app.info("Rule added and applied.");
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function deleteRule(r: Rule) {
    try {
      await api.deleteRule(r.id);
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<div class="row">
  <h1>Categories & rules</h1>
  <span class="spacer"></span>
  <button class="primary" onclick={newCat}>New category</button>
</div>

<div class="kinds">
  {#each byKind as g (g.kind)}
    <div class="card">
      <h2>{KIND_LABEL[g.kind]}</h2>
      <p class="muted small">{KIND_HELP[g.kind]}</p>
      {#each g.cats as c (c.id)}
        <button class="cat" onclick={() => editCat(c)}>
          <span class="dot" style="background:{c.color}"></span>{c.name}
          {#if !c.builtin}<span class="muted small"> · yours</span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</div>

<div class="card" style="margin-top:20px">
  <h2>Your rules</h2>
  <p class="muted small">
    A rule puts every transaction whose narration contains the text into a category. Your rules win over the built-in ones; the
    newest rule wins if two match. Categories you set by hand on a transaction are never changed by rules.
  </p>
  <div class="row" style="margin-bottom:12px">
    <input type="text" placeholder="Text in narration, e.g. ZOMATO" bind:value={rule.pattern} style="width:240px" />
    <select bind:value={rule.direction}>
      <option value={null}>Money in or out</option>
      <option value="debit">Money out only</option>
      <option value="credit">Money in only</option>
    </select>
    <span>→</span>
    <select bind:value={rule.category}>
      <option value="">Choose category…</option>
      {#each app.categories as c (c.id)}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
    <button class="primary" disabled={!rule.category || rule.pattern.trim().length < 3} onclick={addRule}>Add rule</button>
    {#if rule.matches !== null}<span class="muted small">matches {rule.matches} transactions</span>{/if}
  </div>
  {#if rules.length}
    <table>
      <thead><tr><th>Narration contains</th><th>Applies to</th><th>Category</th><th>Created</th><th></th></tr></thead>
      <tbody>
        {#each rules as r (r.id)}
          <tr>
            <td><code>{r.pattern}</code></td>
            <td class="muted">{r.direction === "debit" ? "Money out" : r.direction === "credit" ? "Money in" : "Both"}</td>
            <td>{app.categoryName(r.category)}</td>
            <td class="muted">{r.created}</td>
            <td><button class="link bad" onclick={() => deleteRule(r)}>Delete</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="muted">No rules yet. You can also create one from the Transactions page when you change a category.</p>
  {/if}
</div>

{#if edit}
  <div class="modal-back" role="presentation" onclick={(e) => e.target === e.currentTarget && (edit = null)}>
    <div class="modal">
      <h2>{edit.id ? "Edit category" : "New category"}</h2>
      <label class="field">
        <span>Name</span>
        <input type="text" bind:value={edit.name} />
      </label>
      <label class="field">
        <span>Colour</span>
        <input type="color" bind:value={edit.color} />
      </label>
      <label class="field">
        <span>Type</span>
        <select bind:value={edit.kind} disabled={edit.builtin}>
          {#each KINDS as k (k)}
            <option value={k}>{KIND_LABEL[k]}</option>
          {/each}
        </select>
      </label>
      {#if edit.builtin}<p class="muted small">Built-in category: the type is fixed and it cannot be deleted.</p>{/if}
      <div class="row">
        {#if edit.id && !edit.builtin}<button class="danger" onclick={deleteCat}>Delete</button>{/if}
        <span class="spacer"></span>
        <button onclick={() => (edit = null)}>Cancel</button>
        <button class="primary" disabled={!edit.name.trim()} onclick={saveCat}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .kinds {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px;
    align-items: start;
  }

  .cat {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    padding: 5px 4px;
    border-radius: 4px;
  }

  .cat:hover {
    background: var(--accent-soft);
  }

  input[type="color"] {
    width: 60px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--line);
  }
</style>
