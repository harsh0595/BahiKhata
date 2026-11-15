//! Dashboard figures and the transaction list.
//!
//! Income, expenses and investments are net flows: a refund credited against "Food" reduces food
//! spending; a mutual-fund redemption reduces the amount invested. Excluded categories (own-account
//! transfers, credit-card bill payments) are left out of every total.

use crate::holder;
use crate::model::*;
use crate::money::parse_amount;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Scope {
    pub accounts: Vec<u64>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

impl Scope {
    fn contains(&self, t: &Txn) -> bool {
        self.accounts.contains(&t.account_id)
            && self.from.map(|f| t.date >= f).unwrap_or(true)
            && self.to.map(|e| t.date <= e).unwrap_or(true)
    }
}

pub fn validate(v: &Vault, s: &Scope) -> Result<(), String> {
    if s.accounts.is_empty() {
        return Err("Select at least one account.".into());
    }
    holder::validate_scope(&v.accounts, &s.accounts)
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ScopeOption {
    pub label: String,
    pub accounts: Vec<u64>,
    pub combined: bool,
    pub holder: String,
}

/// "All accounts of <holder>" for every holder with more than one account, then each account.
pub fn scope_options(v: &Vault) -> Vec<ScopeOption> {
    let mut out = Vec::new();
    let groups = holder::groups(&v.accounts);
    for g in &groups {
        if g.len() > 1 {
            let h = v.account(g[0]).map(|a| holder::clean_name(&a.holder)).unwrap_or_default();
            out.push(ScopeOption { label: format!("All accounts — {h}"), accounts: g.clone(), combined: true, holder: h });
        }
    }
    for a in &v.accounts {
        let h = holder::clean_name(&a.holder);
        out.push(ScopeOption { label: format!("{} — {h}", account_label(a)), accounts: vec![a.id], combined: false, holder: h });
    }
    out
}

/// Category kind after applying the "count investments as expense" setting.
pub fn effective_kind(v: &Vault, cat: &str) -> CatKind {
    match v.category(cat).map(|c| c.kind).unwrap_or(CatKind::Expense) {
        CatKind::Investment if v.settings.investments_as_expense => CatKind::Expense,
        k => k,
    }
}

/// Contribution of a transaction to its kind's total (positive = more income/spend/investment).
fn signed(t: &Txn, kind: CatKind) -> Paise {
    let debit = t.direction == Direction::Debit;
    match kind {
        CatKind::Income => if debit { -t.amount } else { t.amount },
        CatKind::Expense | CatKind::Investment => if debit { t.amount } else { -t.amount },
        CatKind::Excluded => 0,
    }
}

fn month_key(d: NaiveDate) -> String {
    format!("{:04}-{:02}", d.year(), d.month())
}

fn months_between(a: NaiveDate, b: NaiveDate) -> Vec<String> {
    let mut out = Vec::new();
    let (mut y, mut m) = (a.year(), a.month());
    while (y, m) <= (b.year(), b.month()) {
        out.push(format!("{y:04}-{m:02}"));
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    out
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct MonthRow {
    pub month: String,
    pub income: Paise,
    pub expenses: Paise,
    pub invested: Paise,
    pub net: Paise,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CatTotal {
    pub id: String,
    pub name: String,
    pub color: String,
    pub amount: Paise,
    pub count: usize,
    pub share: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Series {
    pub id: String,
    pub name: String,
    pub color: String,
    pub values: Vec<Paise>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct MerchantRow {
    pub merchant: String,
    pub category: String,
    pub amount: Paise,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Recurring {
    pub merchant: String,
    pub category: String,
    pub typical: Paise,
    pub months: usize,
    pub total: Paise,
    pub last_date: NaiveDate,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Dashboard {
    pub income: Paise,
    pub expenses: Paise,
    pub invested: Paise,
    pub net: Paise,
    pub savings_rate: Option<f64>,
    pub excluded: Paise,
    pub txn_count: usize,
    pub first_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
    pub months: Vec<MonthRow>,
    pub expense_categories: Vec<CatTotal>,
    pub income_categories: Vec<CatTotal>,
    pub investment_categories: Vec<CatTotal>,
    pub month_labels: Vec<String>,
    pub category_months: Vec<Series>,
    pub merchants: Vec<MerchantRow>,
    pub recurring: Vec<Recurring>,
    pub investments_as_expense: bool,
}

fn cat_totals(v: &Vault, txns: &[(&Txn, CatKind)], kind: CatKind) -> Vec<CatTotal> {
    let mut sums: HashMap<&str, (Paise, usize)> = HashMap::new();
    for (t, k) in txns {
        if *k == kind {
            let e = sums.entry(t.category.as_str()).or_insert((0, 0));
            e.0 += signed(t, kind);
            e.1 += 1;
        }
    }
    let total: Paise = sums.values().map(|(a, _)| (*a).max(0)).sum();
    let mut out: Vec<CatTotal> = sums
        .into_iter()
        .filter(|(_, (a, _))| *a > 0)
        .map(|(id, (amount, count))| {
            let c = v.category(id);
            CatTotal {
                id: id.to_string(),
                name: c.map(|c| c.name.clone()).unwrap_or_else(|| id.to_string()),
                color: c.map(|c| c.color.clone()).unwrap_or_else(|| "#999999".into()),
                amount,
                count,
                share: if total > 0 { amount as f64 / total as f64 } else { 0.0 },
            }
        })
        .collect();
    out.sort_by(|a, b| b.amount.cmp(&a.amount).then(a.name.cmp(&b.name)));
    out
}

fn median(mut xs: Vec<Paise>) -> Paise {
    xs.sort();
    let n = xs.len();
    if n == 0 {
        0
    } else if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2
    }
}

fn recurring(v: &Vault, txns: &[(&Txn, CatKind)], data_end: NaiveDate) -> Vec<Recurring> {
    let mut groups: HashMap<String, Vec<&Txn>> = HashMap::new();
    for (t, k) in txns {
        if t.direction != Direction::Debit || !matches!(k, CatKind::Expense | CatKind::Investment) || t.channel == "ATM" {
            continue;
        }
        let key = if !t.merchant.is_empty() { t.merchant.clone() } else if let Some(vpa) = &t.vpa { vpa.to_uppercase() } else { continue };
        groups.entry(key).or_default().push(*t);
    }
    let mut out = Vec::new();
    for (merchant, ts) in groups {
        let months: BTreeSet<String> = ts.iter().map(|t| month_key(t.date)).collect();
        if months.len() < 3 {
            continue;
        }
        let first = ts.iter().map(|t| t.date).min().unwrap();
        let last = ts.iter().map(|t| t.date).max().unwrap();
        let span = months_between(first, last).len();
        if (months.len() as f64) < 0.75 * span as f64 {
            continue;
        }
        let amounts: Vec<Paise> = ts.iter().map(|t| t.amount).collect();
        let med = median(amounts.clone());
        let close = amounts.iter().filter(|&&a| (a - med).abs() * 5 <= med).count();
        if (close as f64) < 0.7 * amounts.len() as f64 {
            continue;
        }
        if (data_end - last).num_days() > 45 {
            continue;
        }
        // Most common category in the group.
        let mut cats: HashMap<&str, usize> = HashMap::new();
        for t in &ts {
            *cats.entry(t.category.as_str()).or_default() += 1;
        }
        let cat = cats.into_iter().max_by_key(|(c, n)| (*n, std::cmp::Reverse(*c))).map(|(c, _)| c).unwrap_or("other");
        out.push(Recurring {
            merchant,
            category: v.category(cat).map(|c| c.name.clone()).unwrap_or_else(|| cat.to_string()),
            typical: med,
            months: months.len(),
            total: amounts.iter().sum(),
            last_date: last,
        });
    }
    out.sort_by(|a, b| b.typical.cmp(&a.typical).then(a.merchant.cmp(&b.merchant)));
    out
}

pub fn dashboard(v: &Vault, scope: &Scope) -> Result<Dashboard, String> {
    validate(v, scope)?;
    let txns: Vec<(&Txn, CatKind)> = v.txns.iter().filter(|t| scope.contains(t)).map(|t| (t, effective_kind(v, &t.category))).collect();
    let first_date = txns.iter().map(|(t, _)| t.date).min();
    let last_date = txns.iter().map(|(t, _)| t.date).max();

    let mut by_month: BTreeMap<String, MonthRow> = BTreeMap::new();
    if let (Some(a), Some(b)) = (first_date, last_date) {
        for m in months_between(a, b) {
            by_month.insert(m.clone(), MonthRow { month: m, ..Default::default() });
        }
    }
    let (mut income, mut expenses, mut invested, mut excluded) = (0, 0, 0, 0);
    for (t, k) in &txns {
        let row = by_month.get_mut(&month_key(t.date)).unwrap();
        let s = signed(t, *k);
        match k {
            CatKind::Income => {
                income += s;
                row.income += s;
            }
            CatKind::Expense => {
                expenses += s;
                row.expenses += s;
            }
            CatKind::Investment => {
                invested += s;
                row.invested += s;
            }
            CatKind::Excluded => {
                if t.direction == Direction::Debit {
                    excluded += t.amount;
                }
            }
        }
    }
    for r in by_month.values_mut() {
        r.net = r.income - r.expenses - r.invested;
    }
    let months: Vec<MonthRow> = by_month.into_values().collect();
    let month_labels: Vec<String> = months.iter().map(|m| m.month.clone()).collect();

    let expense_categories = cat_totals(v, &txns, CatKind::Expense);
    let income_categories = cat_totals(v, &txns, CatKind::Income);
    let investment_categories = cat_totals(v, &txns, CatKind::Investment);

    // Category-by-month: top 7 expense categories, the rest as "Others".
    let top: Vec<&CatTotal> = expense_categories.iter().take(7).collect();
    let idx: HashMap<&str, usize> = month_labels.iter().enumerate().map(|(i, m)| (m.as_str(), i)).collect();
    let mut series: Vec<Series> = top
        .iter()
        .map(|c| Series { id: c.id.clone(), name: c.name.clone(), color: c.color.clone(), values: vec![0; month_labels.len()] })
        .collect();
    let mut others = Series { id: "_others".into(), name: "Others".into(), color: "#bdbdbd".into(), values: vec![0; month_labels.len()] };
    for (t, k) in &txns {
        if *k != CatKind::Expense {
            continue;
        }
        let i = idx[month_key(t.date).as_str()];
        match series.iter_mut().find(|s| s.id == t.category) {
            Some(s) => s.values[i] += signed(t, *k),
            None => others.values[i] += signed(t, *k),
        }
    }
    if expense_categories.len() > 7 {
        series.push(others);
    }

    let mut merch: HashMap<&str, (Paise, usize, HashMap<&str, usize>)> = HashMap::new();
    for (t, k) in &txns {
        if *k != CatKind::Expense || t.merchant.is_empty() {
            continue;
        }
        let e = merch.entry(t.merchant.as_str()).or_insert((0, 0, HashMap::new()));
        e.0 += signed(t, *k);
        e.1 += 1;
        *e.2.entry(t.category.as_str()).or_default() += 1;
    }
    let mut merchants: Vec<MerchantRow> = merch
        .into_iter()
        .filter(|(_, (a, _, _))| *a > 0)
        .map(|(m, (amount, count, cats))| {
            let cat = cats.into_iter().max_by_key(|(c, n)| (*n, std::cmp::Reverse(*c))).map(|(c, _)| c).unwrap_or("other");
            MerchantRow {
                merchant: m.to_string(),
                category: v.category(cat).map(|c| c.name.clone()).unwrap_or_else(|| cat.to_string()),
                amount,
                count,
            }
        })
        .collect();
    merchants.sort_by(|a, b| b.amount.cmp(&a.amount).then(a.merchant.cmp(&b.merchant)));
    merchants.truncate(15);

    let recurring = match last_date {
        Some(end) => recurring(v, &txns, end),
        None => Vec::new(),
    };

    Ok(Dashboard {
        income,
        expenses,
        invested,
        net: income - expenses - invested,
        savings_rate: if income > 0 { Some((income - expenses) as f64 / income as f64) } else { None },
        excluded,
        txn_count: txns.len(),
        first_date,
        last_date,
        months,
        expense_categories,
        income_categories,
        investment_categories,
        month_labels,
        category_months: series,
        merchants,
        recurring,
        investments_as_expense: v.settings.investments_as_expense,
    })
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct TxnQuery {
    pub scope: Scope,
    #[serde(default)]
    pub search: String,
    pub category: Option<String>,
    pub kind: Option<CatKind>,
    pub direction: Option<Direction>,
    pub month: Option<String>,
    /// Only transactions whose category came this way (e.g. auto-guessed).
    pub source: Option<CatSource>,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TxnRow {
    pub id: u64,
    pub date: NaiveDate,
    pub account: String,
    pub narration: String,
    pub reference: String,
    pub merchant: String,
    pub vpa: Option<String>,
    pub channel: String,
    pub amount: Paise,
    pub direction: Direction,
    pub balance: Option<Paise>,
    pub category: String,
    pub category_name: String,
    pub color: String,
    pub source: CatSource,
    pub note: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TxnPage {
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub debits: Paise,
    pub credits: Paise,
    pub rows: Vec<TxnRow>,
}

pub fn row(v: &Vault, t: &Txn) -> TxnRow {
    let c = v.category(&t.category);
    TxnRow {
        id: t.id,
        date: t.date,
        account: v.account(t.account_id).map(account_label).unwrap_or_default(),
        narration: t.narration.clone(),
        reference: t.reference.clone(),
        merchant: t.merchant.clone(),
        vpa: t.vpa.clone(),
        channel: t.channel.clone(),
        amount: t.amount,
        direction: t.direction,
        balance: t.balance,
        category: t.category.clone(),
        category_name: c.map(|c| c.name.clone()).unwrap_or_else(|| t.category.clone()),
        color: c.map(|c| c.color.clone()).unwrap_or_else(|| "#999999".into()),
        source: t.source,
        note: t.note.clone(),
    }
}

/// Transactions matching the query (all pages when `page_size` is 0).
pub fn matching<'a>(v: &'a Vault, q: &TxnQuery) -> Vec<&'a Txn> {
    let search = q.search.trim().to_lowercase();
    let amount = parse_amount(&search).map(|a| a.paise).filter(|p| *p > 0);
    let mut out: Vec<&Txn> = v
        .txns
        .iter()
        .filter(|t| q.scope.contains(t))
        .filter(|t| q.category.as_deref().map(|c| t.category == c).unwrap_or(true))
        .filter(|t| q.kind.map(|k| effective_kind(v, &t.category) == k).unwrap_or(true))
        .filter(|t| q.direction.map(|d| t.direction == d).unwrap_or(true))
        .filter(|t| q.month.as_deref().map(|m| month_key(t.date) == m).unwrap_or(true))
        .filter(|t| q.source.map(|s| t.source == s).unwrap_or(true))
        .filter(|t| {
            search.is_empty()
                || amount == Some(t.amount)
                || t.narration.to_lowercase().contains(&search)
                || t.merchant.to_lowercase().contains(&search)
                || t.reference.to_lowercase().contains(&search)
                || t.note.to_lowercase().contains(&search)
                || v.category(&t.category).map(|c| c.name.to_lowercase().contains(&search)).unwrap_or(false)
        })
        .collect();
    match q.sort.as_str() {
        "date_asc" => out.sort_by_key(|t| (t.date, t.id)),
        "amount_desc" => out.sort_by_key(|t| (std::cmp::Reverse(t.amount), std::cmp::Reverse(t.date))),
        "amount_asc" => out.sort_by_key(|t| (t.amount, std::cmp::Reverse(t.date))),
        _ => out.sort_by_key(|t| (std::cmp::Reverse(t.date), std::cmp::Reverse(t.id))),
    }
    out
}

pub fn query(v: &Vault, q: &TxnQuery) -> Result<TxnPage, String> {
    validate(v, &q.scope)?;
    let all = matching(v, q);
    let debits = all.iter().filter(|t| t.direction == Direction::Debit).map(|t| t.amount).sum();
    let credits = all.iter().filter(|t| t.direction == Direction::Credit).map(|t| t.amount).sum();
    let size = if q.page_size == 0 { all.len().max(1) } else { q.page_size };
    let rows = all.iter().skip(q.page * size).take(size).map(|t| row(v, t)).collect();
    Ok(TxnPage { total: all.len(), page: q.page, page_size: size, debits, credits, rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ParsedStatement, ParsedTxn};
    use crate::store::commit;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn vault() -> (Vault, u64) {
        let mut v = Vault::new();
        let mut txns = Vec::new();
        for m in 4..=9u32 {
            txns.push((d(2025, m, 1), "NEFT CR-ICIC0000001-ACME LTD-SALARY", 100_000_00, Direction::Credit));
            txns.push((d(2025, m, 3), "UPI-NETFLIX-NETFLIX@ICICI-1-UPI", 649_00, Direction::Debit));
            txns.push((d(2025, m, 5), "ACH D- ZERODHA BROKING LTD-SIP", 10_000_00, Direction::Debit));
            txns.push((d(2025, m, 10), "UPI-SWIGGY-SWIGGY@AXIS-1-UPI", (m as i64) * 100_00, Direction::Debit));
        }
        txns.push((d(2025, 6, 12), "UPI-SWIGGY-SWIGGY@AXIS-2-UPI", 200_00, Direction::Credit));
        txns.push((d(2025, 7, 1), "UPI-CRED CLUB-CRED@AXIS-CREDIT CARD", 5_000_00, Direction::Debit));
        let s = ParsedStatement {
            bank: "HDFC".into(),
            account_number: "50100123456789".into(),
            holder: "Harsh Sharma".into(),
            pan: None,
            ifsc: None,
            period_from: None,
            period_to: None,
            warnings: vec![],
            txns: txns
                .into_iter()
                .map(|(date, n, amount, direction)| ParsedTxn {
                    date,
                    value_date: None,
                    narration: n.into(),
                    reference: String::new(),
                    amount,
                    direction,
                    balance: None,
                })
                .collect(),
        };
        let id = commit(&mut v, &s, "x", None).unwrap().account_id;
        (v, id)
    }

    #[test]
    fn totals_and_recurring() {
        let (mut v, id) = vault();
        let scope = Scope { accounts: vec![id], from: None, to: None };
        let db = dashboard(&v, &scope).unwrap();
        assert_eq!(db.income, 600_000_00);
        // Netflix 6 x 649 + Swiggy (4+5+6+7+8+9)*100 - 200 refund.
        assert_eq!(db.expenses, 6 * 649_00 + 3900_00 - 200_00);
        assert_eq!(db.invested, 60_000_00);
        assert_eq!(db.excluded, 5_000_00);
        assert_eq!(db.net, db.income - db.expenses - db.invested);
        assert_eq!(db.months.len(), 6);
        assert_eq!(db.months[0].month, "2025-04");
        assert_eq!(db.expense_categories[0].id, "entertainment");
        assert_eq!(db.expense_categories[1].amount, 3700_00);
        let rec: Vec<&str> = db.recurring.iter().map(|r| r.merchant.as_str()).collect();
        assert!(rec.contains(&"NETFLIX") && rec.contains(&"ZERODHA BROKING"), "{rec:?}");
        assert!(!rec.contains(&"SWIGGY"), "{rec:?}");

        v.settings.investments_as_expense = true;
        let db = dashboard(&v, &scope).unwrap();
        assert_eq!(db.invested, 0);
        assert_eq!(db.expenses, 6 * 649_00 + 3900_00 - 200_00 + 60_000_00);

        let june = Scope { accounts: vec![id], from: Some(d(2025, 6, 1)), to: Some(d(2025, 6, 30)) };
        assert_eq!(dashboard(&v, &june).unwrap().income, 100_000_00);
        assert!(dashboard(&v, &Scope::default()).is_err());
    }

    #[test]
    fn query_filters() {
        let (v, id) = vault();
        let scope = Scope { accounts: vec![id], from: None, to: None };
        let q = TxnQuery { scope: scope.clone(), search: "swiggy".into(), page_size: 3, ..Default::default() };
        let p = query(&v, &q).unwrap();
        assert_eq!((p.total, p.rows.len()), (7, 3));
        assert_eq!(p.rows[0].date, d(2025, 9, 10));
        let q = TxnQuery { scope: scope.clone(), search: "649".into(), ..Default::default() };
        assert_eq!(query(&v, &q).unwrap().total, 6);
        let q = TxnQuery { scope: scope.clone(), category: Some("food".into()), month: Some("2025-06".into()), ..Default::default() };
        assert_eq!(query(&v, &q).unwrap().total, 2);
        let q = TxnQuery { scope, kind: Some(CatKind::Income), ..Default::default() };
        assert_eq!(query(&v, &q).unwrap().total, 6);
    }
}
