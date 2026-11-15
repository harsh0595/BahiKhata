import { api, errText, type Category, type Scope, type ScopeOption } from "./api";
import { PRESETS } from "./format";

export type Page = "dashboard" | "transactions" | "import" | "accounts" | "categories" | "settings" | "report";

export interface TxnFilter {
  category: string | null;
  kind: string | null;
  month: string | null;
  search: string;
}

class AppStore {
  unlocked = $state(false);
  page = $state<Page>("dashboard");
  scopeOptions = $state<ScopeOption[]>([]);
  scopeIndex = $state(0);
  preset = $state("all");
  from = $state<string | null>(null);
  to = $state<string | null>(null);
  categories = $state<Category[]>([]);
  /** Bumped after every change so pages reload their data. */
  version = $state(0);
  toast = $state<{ text: string; error: boolean } | null>(null);
  txnFilter = $state<TxnFilter>({ category: null, kind: null, month: null, search: "" });
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  get scope(): Scope | null {
    const o = this.scopeOptions[this.scopeIndex];
    return o ? { accounts: [...o.accounts], from: this.from || null, to: this.to || null } : null;
  }

  get scopeLabel(): string {
    return this.scopeOptions[this.scopeIndex]?.label ?? "";
  }

  get periodLabel(): string {
    if (!this.from && !this.to) return "All time";
    const p = PRESETS.find((x) => x.id === this.preset);
    if (p && p.id !== "custom") return p.label;
    return `${this.from ?? "start"} to ${this.to ?? "today"}`;
  }

  categoryName(id: string): string {
    return this.categories.find((c) => c.id === id)?.name ?? id;
  }

  setPreset(id: string) {
    this.preset = id;
    if (id !== "custom") {
      const [f, t] = PRESETS.find((p) => p.id === id)?.range() ?? [null, null];
      this.from = f;
      this.to = t;
    }
  }

  async loadMeta() {
    const prev = this.scopeOptions[this.scopeIndex];
    const [opts, cats] = await Promise.all([api.scopeOptions(), api.categories()]);
    this.scopeOptions = opts;
    this.categories = cats;
    const key = prev ? prev.accounts.join(",") : "";
    const i = opts.findIndex((o) => o.accounts.join(",") === key);
    this.scopeIndex = i >= 0 ? i : 0;
  }

  async changed() {
    try {
      await this.loadMeta();
    } catch (e) {
      this.error(e);
    }
    this.version++;
  }

  showTxns(f: Partial<TxnFilter>) {
    this.txnFilter = { category: null, kind: null, month: null, search: "", ...f };
    this.page = "transactions";
  }

  info(text: string) {
    this.show(text, false);
  }

  error(e: unknown) {
    this.show(errText(e), true);
  }

  private show(text: string, error: boolean) {
    this.toast = { text, error };
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), error ? 7000 : 3500);
  }

  reset() {
    this.unlocked = false;
    this.page = "dashboard";
    this.scopeOptions = [];
    this.scopeIndex = 0;
    this.categories = [];
    this.setPreset("all");
  }
}

export const app = new AppStore();
