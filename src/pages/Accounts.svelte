<script lang="ts">
  import { api, type Account } from "../lib/api";
  import { inr, dmy } from "../lib/format";
  import { app } from "../lib/state.svelte";

  let list = $state.raw<Account[]>([]);
  let edit = $state<{ id: number; nickname: string; holder: string; holderEditable: boolean } | null>(null);

  async function load() {
    try {
      list = await api.accounts();
    } catch (e) {
      app.error(e);
    }
  }

  $effect(() => {
    void app.version;
    load();
  });

  async function saveEdit() {
    if (!edit) return;
    try {
      await api.updateAccount(edit.id, edit.nickname, edit.holderEditable ? edit.holder : null);
      edit = null;
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }

  async function remove(a: Account) {
    if (!window.confirm(`Delete ${a.label} and all its ${a.txn_count} transactions? This cannot be undone.`)) return;
    try {
      await api.deleteAccount(a.id);
      app.info("Account deleted.");
      await app.changed();
    } catch (e) {
      app.error(e);
    }
  }
</script>

<h1>Accounts</h1>

{#if !list.length}
  <div class="card empty">
    <p>No accounts yet. Accounts are created automatically when you import a statement.</p>
    <button class="primary" onclick={() => (app.page = "import")}>Import a statement</button>
  </div>
{:else}
  <div class="card" style="padding:0">
    <table>
      <thead>
        <tr>
          <th>Account</th>
          <th>Bank</th>
          <th>Number</th>
          <th>Holder</th>
          <th>PAN</th>
          <th>Data from</th>
          <th class="num">Transactions</th>
          <th class="num">Last balance</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each list as a (a.id)}
          <tr>
            <td><strong>{a.label}</strong></td>
            <td>{a.bank}</td>
            <td class="muted">{a.number}</td>
            <td>{a.holder}{#if a.holder_entered}<span class="muted small"> (entered by you)</span>{/if}</td>
            <td class="muted">{a.pan ?? "—"}</td>
            <td class="muted">{dmy(a.first_date)} – {dmy(a.last_date)}</td>
            <td class="num">{a.txn_count}</td>
            <td class="num">{inr(a.balance)}</td>
            <td class="row" style="flex-wrap:nowrap">
              <button
                class="link"
                onclick={() => (edit = { id: a.id, nickname: a.nickname, holder: a.holder, holderEditable: a.holder_entered })}
                >Edit</button
              >
              <button class="link bad" onclick={() => remove(a)}>Delete</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <p class="muted small">
    Accounts of the same person (same PAN, or matching name) can be viewed together. Transfers between them are detected and
    left out of income and expenses.
  </p>
{/if}

{#if edit}
  <div class="modal-back" role="presentation" onclick={(e) => e.target === e.currentTarget && (edit = null)}>
    <div class="modal">
      <h2>Edit account</h2>
      <label class="field">
        <span>Nickname (optional, e.g. "Salary account")</span>
        <input type="text" bind:value={edit.nickname} />
      </label>
      {#if edit.holderEditable}
        <label class="field">
          <span>Account holder</span>
          <input type="text" bind:value={edit.holder} />
        </label>
      {:else}
        <p class="muted small">The holder name comes from the bank statement and cannot be changed.</p>
      {/if}
      <div class="row">
        <span class="spacer"></span>
        <button onclick={() => (edit = null)}>Cancel</button>
        <button class="primary" onclick={saveEdit}>Save</button>
      </div>
    </div>
  </div>
{/if}
