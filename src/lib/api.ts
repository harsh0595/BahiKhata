import { invoke } from "@tauri-apps/api/core";

export type Paise = number;
export type Direction = "debit" | "credit";
export type CatKind = "expense" | "income" | "investment" | "excluded";
export type CatSource = "default" | "rule" | "user" | "transfer";

export interface Status {
  exists: boolean;
  unlocked: boolean;
  path: string;
  version: string;
  auto_lock_minutes: number;
}

export interface Settings {
  investments_as_expense: boolean;
  auto_lock_minutes: number;
}

export interface Scope {
  accounts: number[];
  from: string | null;
  to: string | null;
}

export interface ScopeOption {
  label: string;
  accounts: number[];
  combined: boolean;
  holder: string;
}

export interface Category {
  id: string;
  name: string;
  kind: CatKind;
  color: string;
  builtin: boolean;
}

export interface Rule {
  id: number;
  pattern: string;
  category: string;
  direction: Direction | null;
  created: string;
}

export interface Account {
  id: number;
  bank: string;
  number: string;
  holder: string;
  holder_entered: boolean;
  pan: string | null;
  nickname: string;
  created: string;
  label: string;
  txn_count: number;
  first_date: string | null;
  last_date: string | null;
  balance: Paise | null;
}

export interface ImportRow {
  id: number;
  file_name: string;
  account_id: number;
  imported_at: string;
  period_from: string | null;
  period_to: string | null;
  added: number;
  skipped: number;
  account: string;
}

export interface PreviewRow {
  date: string;
  narration: string;
  amount: Paise;
  direction: Direction;
  category: string;
  duplicate: boolean;
}

export interface ImportPreview {
  file_name: string;
  bank: string;
  account_number: string;
  holder: string;
  holder_found: boolean;
  pan: string | null;
  ifsc: string | null;
  period_from: string | null;
  period_to: string | null;
  total: number;
  new_count: number;
  duplicates: number;
  debits: Paise;
  credits: Paise;
  existing_account: number | null;
  existing_label: string | null;
  warnings: string[];
  rows: PreviewRow[];
}

export interface ImportResult {
  account_id: number;
  import_id: number;
  added: number;
  skipped: number;
}

export interface MonthRow {
  month: string;
  income: Paise;
  expenses: Paise;
  invested: Paise;
  net: Paise;
}

export interface CatTotal {
  id: string;
  name: string;
  color: string;
  amount: Paise;
  count: number;
  share: number;
}

export interface Series {
  id: string;
  name: string;
  color: string;
  values: Paise[];
}

export interface MerchantRow {
  merchant: string;
  category: string;
  amount: Paise;
  count: number;
}

export interface Recurring {
  merchant: string;
  category: string;
  typical: Paise;
  months: number;
  total: Paise;
  last_date: string;
}

export interface Dashboard {
  income: Paise;
  expenses: Paise;
  invested: Paise;
  net: Paise;
  savings_rate: number | null;
  excluded: Paise;
  txn_count: number;
  first_date: string | null;
  last_date: string | null;
  months: MonthRow[];
  expense_categories: CatTotal[];
  income_categories: CatTotal[];
  investment_categories: CatTotal[];
  month_labels: string[];
  category_months: Series[];
  merchants: MerchantRow[];
  recurring: Recurring[];
  investments_as_expense: boolean;
}

export interface TxnQuery {
  scope: Scope;
  search: string;
  category: string | null;
  kind: CatKind | null;
  direction: Direction | null;
  month: string | null;
  sort: string;
  page: number;
  page_size: number;
}

export interface TxnRow {
  id: number;
  date: string;
  account: string;
  narration: string;
  reference: string;
  merchant: string;
  vpa: string | null;
  channel: string;
  amount: Paise;
  direction: Direction;
  balance: Paise | null;
  category: string;
  category_name: string;
  color: string;
  source: CatSource;
  note: string;
}

export interface TxnPage {
  total: number;
  page: number;
  page_size: number;
  debits: Paise;
  credits: Paise;
  rows: TxnRow[];
}

export const api = {
  status: () => invoke<Status>("vault_status"),
  createVault: (password: string) => invoke<void>("create_vault", { password }),
  unlock: (password: string) => invoke<void>("unlock", { password }),
  lock: () => invoke<void>("lock"),
  changePassword: (current: string, next: string) => invoke<void>("change_password", { current, new: next }),
  deleteAll: (confirm: string) => invoke<void>("delete_all", { confirm }),
  backupVault: (dest: string) => invoke<void>("backup_vault", { dest }),
  restoreVault: (src: string, password: string) => invoke<void>("restore_vault", { src, password }),

  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),

  previewImport: (path: string) => invoke<ImportPreview>("preview_import", { path }),
  commitImport: (holder: string | null) => invoke<ImportResult>("commit_import", { holder }),
  cancelImport: () => invoke<void>("cancel_import"),
  imports: () => invoke<ImportRow[]>("imports"),
  deleteImport: (id: number) => invoke<number>("delete_import", { id }),

  accounts: () => invoke<Account[]>("accounts"),
  updateAccount: (id: number, nickname: string, holder: string | null) =>
    invoke<void>("update_account", { id, nickname, holder }),
  deleteAccount: (id: number) => invoke<void>("delete_account", { id }),

  categories: () => invoke<Category[]>("categories"),
  addCategory: (name: string, kind: CatKind, color: string) => invoke<string>("add_category", { name, kind, color }),
  updateCategory: (id: string, name: string, color: string, kind: CatKind | null) =>
    invoke<void>("update_category", { id, name, color, kind }),
  deleteCategory: (id: string) => invoke<void>("delete_category", { id }),
  rules: () => invoke<Rule[]>("rules"),
  ruleMatches: (pattern: string, direction: Direction | null) => invoke<number>("rule_matches", { pattern, direction }),
  addRule: (pattern: string, category: string, direction: Direction | null) =>
    invoke<number>("add_rule", { pattern, category, direction }),
  deleteRule: (id: number) => invoke<void>("delete_rule", { id }),

  queryTxns: (query: TxnQuery) => invoke<TxnPage>("query_txns", { query }),
  setCategory: (ids: number[], category: string) => invoke<number>("set_category", { ids, category }),
  resetCategory: (ids: number[]) => invoke<number>("reset_category", { ids }),
  setNote: (id: number, note: string) => invoke<void>("set_note", { id, note }),

  scopeOptions: () => invoke<ScopeOption[]>("scope_options"),
  dashboard: (scope: Scope) => invoke<Dashboard>("dashboard", { scope }),
  exportExcel: (dest: string, scope: Scope, label: string) => invoke<void>("export_excel", { dest, scope, label }),
};

export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
