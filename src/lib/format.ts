import type { Paise } from "./api";

const inrFmt = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR", minimumFractionDigits: 2 });
const inrWhole = new Intl.NumberFormat("en-IN", { style: "currency", currency: "INR", maximumFractionDigits: 0 });

export function inr(p: Paise | null | undefined): string {
  if (p === null || p === undefined) return "-";
  return inrFmt.format(p / 100);
}

export function inr0(p: Paise | null | undefined): string {
  if (p === null || p === undefined) return "-";
  return inrWhole.format(Math.round(p / 100));
}

/** Compact Indian notation for chart axes: 12.5K, 3.2L, 1.1Cr. */
export function inrShort(rupees: number): string {
  const a = Math.abs(rupees);
  const s = rupees < 0 ? "-" : "";
  if (a >= 1e7) return `${s}₹${(a / 1e7).toFixed(1)}Cr`;
  if (a >= 1e5) return `${s}₹${(a / 1e5).toFixed(1)}L`;
  if (a >= 1e3) return `${s}₹${(a / 1e3).toFixed(1)}K`;
  return `${s}₹${a.toFixed(0)}`;
}

export function pct(x: number | null | undefined): string {
  if (x === null || x === undefined || !isFinite(x)) return "-";
  return `${(x * 100).toFixed(1)}%`;
}

/** "2025-04-01" → "01-04-2025". */
export function dmy(iso: string | null | undefined): string {
  if (!iso) return "-";
  const [y, m, d] = iso.split("-");
  return `${d}-${m}-${y}`;
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "2025-04" → "Apr 2025". */
export function monthLabel(key: string): string {
  const [y, m] = key.split("-");
  const i = Number(m) - 1;
  return i >= 0 && i < 12 ? `${MONTHS[i]} ${y}` : key;
}

export function iso(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export interface Preset {
  id: string;
  label: string;
  range: () => [string | null, string | null];
}

/** Indian financial year starting April of `startYear`. */
function fy(startYear: number): [string, string] {
  return [`${startYear}-04-01`, `${startYear + 1}-03-31`];
}

function currentFyStart(now: Date): number {
  return now.getMonth() >= 3 ? now.getFullYear() : now.getFullYear() - 1;
}

export const PRESETS: Preset[] = [
  { id: "all", label: "All time", range: () => [null, null] },
  {
    id: "this_month",
    label: "This month",
    range: () => {
      const n = new Date();
      return [iso(new Date(n.getFullYear(), n.getMonth(), 1)), iso(new Date(n.getFullYear(), n.getMonth() + 1, 0))];
    },
  },
  {
    id: "last_month",
    label: "Last month",
    range: () => {
      const n = new Date();
      return [iso(new Date(n.getFullYear(), n.getMonth() - 1, 1)), iso(new Date(n.getFullYear(), n.getMonth(), 0))];
    },
  },
  {
    id: "last_3",
    label: "Last 3 months",
    range: () => {
      const n = new Date();
      return [iso(new Date(n.getFullYear(), n.getMonth() - 2, 1)), iso(n)];
    },
  },
  {
    id: "last_12",
    label: "Last 12 months",
    range: () => {
      const n = new Date();
      return [iso(new Date(n.getFullYear(), n.getMonth() - 11, 1)), iso(n)];
    },
  },
  { id: "this_fy", label: "This financial year", range: () => fy(currentFyStart(new Date())) },
  { id: "last_fy", label: "Last financial year", range: () => fy(currentFyStart(new Date()) - 1) },
  {
    id: "this_cy",
    label: "This calendar year",
    range: () => {
      const y = new Date().getFullYear();
      return [`${y}-01-01`, `${y}-12-31`];
    },
  },
  { id: "custom", label: "Custom range", range: () => [null, null] },
];

export const KIND_LABEL: Record<string, string> = {
  expense: "Expense",
  income: "Income",
  investment: "Investment",
  excluded: "Excluded",
};

export const SOURCE_LABEL: Record<string, string> = {
  default: "No rule matched",
  rule: "Matched a rule",
  user: "Set by you",
  transfer: "Own-account transfer",
};
