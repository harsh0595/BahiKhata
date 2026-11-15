//! Operations on the vault: importing statements, editing categories and rules, accounts,
//! and pairing transfers between the user's own accounts.

use crate::categorize::{recategorize_all, Categorizer, OWN_TRANSFER};
use crate::holder::{self, names_match};
use crate::import::{normalize_account, ParsedStatement};
use crate::model::*;
use crate::narration;
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

fn key_text(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
}

/// De-duplication keys for a statement's rows, in order. Identical rows in one file get an
/// occurrence suffix so genuine repeats (two equal payments the same day) are kept.
fn keys_for(s: &ParsedStatement) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    s.txns
        .iter()
        .map(|t| {
            let base = format!(
                "{}|{}|{}|{}|{}",
                t.date,
                if t.direction == Direction::Debit { "D" } else { "C" },
                t.amount,
                t.balance.map(|b| b.to_string()).unwrap_or_default(),
                key_text(&t.narration)
            );
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            format!("{base}#{n}")
        })
        .collect()
}

fn digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn last4(s: &str) -> String {
    let d = digits(s);
    d[d.len().saturating_sub(4)..].to_string()
}

/// Finds the existing account a statement belongs to.
pub fn find_account(v: &Vault, s: &ParsedStatement) -> Option<u64> {
    let num = normalize_account(&s.account_number);
    if num.is_empty() {
        return None;
    }
    let full = num.chars().all(|c| c.is_ascii_digit());
    v.accounts
        .iter()
        .filter(|a| a.bank == s.bank)
        .find(|a| {
            let other = normalize_account(&a.number);
            if full && other.chars().all(|c| c.is_ascii_digit()) {
                other == num
            } else {
                last4(&other).len() == 4 && last4(&other) == last4(&num) && (s.holder.is_empty() || names_match(&a.holder, &s.holder))
            }
        })
        .map(|a| a.id)
}

#[derive(Serialize, Debug, Clone)]
pub struct PreviewRow {
    pub date: NaiveDate,
    pub narration: String,
    pub amount: Paise,
    pub direction: Direction,
    pub category: String,
    pub duplicate: bool,
}

#[derive(Serialize, Debug, Clone)]
pub struct ImportPreview {
    pub file_name: String,
    pub bank: String,
    pub account_number: String,
    pub holder: String,
    pub holder_found: bool,
    pub pan: Option<String>,
    pub ifsc: Option<String>,
    pub period_from: Option<NaiveDate>,
    pub period_to: Option<NaiveDate>,
    pub total: usize,
    pub new_count: usize,
    pub duplicates: usize,
    pub debits: Paise,
    pub credits: Paise,
    pub existing_account: Option<u64>,
    pub existing_label: Option<String>,
    pub warnings: Vec<String>,
    pub rows: Vec<PreviewRow>,
}

pub fn preview(v: &Vault, s: &ParsedStatement, file_name: &str) -> ImportPreview {
    let existing = find_account(v, s);
    let have: HashSet<&str> = match existing {
        Some(id) => v.txns.iter().filter(|t| t.account_id == id).map(|t| t.key.as_str()).collect(),
        None => HashSet::new(),
    };
    let cz = Categorizer::new(&v.rules, &v.categories);
    let keys = keys_for(s);
    let mut rows = Vec::new();
    let (mut new_count, mut debits, mut credits) = (0, 0, 0);
    for (t, k) in s.txns.iter().zip(&keys) {
        let dup = have.contains(k.as_str());
        if !dup {
            new_count += 1;
            match t.direction {
                Direction::Debit => debits += t.amount,
                Direction::Credit => credits += t.amount,
            }
        }
        rows.push(PreviewRow {
            date: t.date,
            narration: t.narration.clone(),
            amount: t.amount,
            direction: t.direction,
            category: cz.categorize(&t.narration, t.direction).0,
            duplicate: dup,
        });
    }
    let mut warnings = s.warnings.clone();
    if let (Some(id), Some(pan)) = (existing, &s.pan) {
        if let Some(Some(p)) = v.account(id).map(|a| a.pan.as_ref()) {
            if p != pan {
                warnings.push("The PAN in this file differs from the PAN saved for this account.".into());
            }
        }
    }
    ImportPreview {
        file_name: file_name.to_string(),
        bank: s.bank.clone(),
        account_number: s.account_number.clone(),
        holder: s.holder.clone(),
        holder_found: !s.holder.trim().is_empty(),
        pan: s.pan.clone(),
        ifsc: s.ifsc.clone(),
        period_from: s.period_from,
        period_to: s.period_to,
        total: s.txns.len(),
        new_count,
        duplicates: s.txns.len() - new_count,
        debits,
        credits,
        existing_account: existing,
        existing_label: existing.and_then(|id| v.account(id)).map(account_label),
        warnings,
        rows,
    }
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ImportResult {
    pub account_id: u64,
    pub import_id: u64,
    pub added: usize,
    pub skipped: usize,
}

pub fn commit(v: &mut Vault, s: &ParsedStatement, file_name: &str, holder_override: Option<&str>) -> Result<ImportResult, String> {
    let entered = holder_override.map(|h| h.trim()).filter(|h| !h.is_empty());
    let holder_name = if s.holder.trim().is_empty() { entered.map(holder::clean_name) } else { Some(holder::clean_name(&s.holder)) };
    let Some(holder_name) = holder_name else {
        return Err("The statement does not show the account holder's name. Please enter it.".into());
    };
    let account_id = match find_account(v, s) {
        Some(id) => {
            let acc = v.accounts.iter_mut().find(|a| a.id == id).unwrap();
            match (&acc.pan, &s.pan) {
                (Some(p), Some(q)) if p != q => {
                    return Err("The PAN in this file does not match the PAN saved for this account. Import cancelled.".into())
                }
                (None, Some(q)) => acc.pan = Some(q.clone()),
                _ => {}
            }
            if acc.holder_entered && !s.holder.trim().is_empty() {
                acc.holder = holder::clean_name(&s.holder);
                acc.holder_entered = false;
            }
            id
        }
        None => {
            let id = v.alloc_id();
            v.accounts.push(Account {
                id,
                bank: s.bank.clone(),
                number: s.account_number.clone(),
                holder: holder_name,
                holder_entered: s.holder.trim().is_empty(),
                pan: s.pan.clone(),
                nickname: String::new(),
                created: now_string(),
            });
            id
        }
    };

    let have: HashSet<String> = v.txns.iter().filter(|t| t.account_id == account_id).map(|t| t.key.clone()).collect();
    let cz = Categorizer::new(&v.rules, &v.categories);
    let import_id = v.alloc_id();
    let keys = keys_for(s);
    let mut added = 0;
    for (t, key) in s.txns.iter().zip(keys) {
        if have.contains(&key) {
            continue;
        }
        let info = narration::analyze(&t.narration, &t.reference);
        let (category, source, rule_id) = cz.categorize(&t.narration, t.direction);
        let id = v.alloc_id();
        v.txns.push(Txn {
            id,
            account_id,
            date: t.date,
            value_date: t.value_date,
            narration: t.narration.clone(),
            reference: t.reference.clone(),
            amount: t.amount,
            direction: t.direction,
            balance: t.balance,
            merchant: info.merchant,
            vpa: info.vpa,
            channel: info.channel.to_string(),
            category,
            source,
            rule_id,
            key,
            import_id,
            note: String::new(),
        });
        added += 1;
    }
    let skipped = s.txns.len() - added;
    v.imports.push(ImportRecord {
        id: import_id,
        file_name: file_name.to_string(),
        account_id,
        imported_at: now_string(),
        period_from: s.period_from,
        period_to: s.period_to,
        added,
        skipped,
    });
    v.txns.sort_by(|a, b| (a.date, a.account_id, a.id).cmp(&(b.date, b.account_id, b.id)));
    detect_transfers(v);
    Ok(ImportResult { account_id, import_id, added, skipped })
}

const TRANSFER_CHANNELS: &[&str] = &["UPI", "NEFT", "RTGS", "IMPS", "OTHER", "CHEQUE"];

/// Pairs a debit in one account with a credit of the same amount in another account of the same
/// holder within 3 days, and marks both as own-account transfers. Re-run after every change.
pub fn detect_transfers(v: &mut Vault) -> usize {
    let cz = Categorizer::new(&v.rules, &v.categories);
    for t in v.txns.iter_mut().filter(|t| t.source == CatSource::Transfer) {
        let (c, s, r) = cz.categorize(&t.narration, t.direction);
        t.category = c;
        t.source = s;
        t.rule_id = r;
    }
    if v.category(OWN_TRANSFER).is_none() {
        return 0;
    }
    let mut pairs = 0;
    for group in holder::groups(&v.accounts).into_iter().filter(|g| g.len() > 1) {
        let eligible = |t: &Txn| group.contains(&t.account_id) && t.source != CatSource::User && TRANSFER_CHANNELS.contains(&t.channel.as_str());
        let debits: Vec<usize> = (0..v.txns.len()).filter(|&i| eligible(&v.txns[i]) && v.txns[i].direction == Direction::Debit).collect();
        let mut credits: Vec<usize> = (0..v.txns.len()).filter(|&i| eligible(&v.txns[i]) && v.txns[i].direction == Direction::Credit).collect();
        for d in debits {
            let dt = &v.txns[d];
            let best = credits
                .iter()
                .enumerate()
                .filter(|&(_, &c)| {
                    let ct = &v.txns[c];
                    ct.account_id != dt.account_id && ct.amount == dt.amount && (ct.date - dt.date).num_days().abs() <= 3
                })
                .min_by_key(|&(_, &c)| (v.txns[c].date - dt.date).num_days().abs())
                .map(|(pos, &c)| (pos, c));
            if let Some((pos, c)) = best {
                credits.remove(pos);
                for i in [d, c] {
                    v.txns[i].category = OWN_TRANSFER.to_string();
                    v.txns[i].source = CatSource::Transfer;
                    v.txns[i].rule_id = None;
                }
                pairs += 1;
            }
        }
    }
    pairs
}

fn require_category(v: &Vault, id: &str) -> Result<(), String> {
    if v.category(id).is_some() { Ok(()) } else { Err(format!("Unknown category '{id}'")) }
}

/// Set the category by hand. It will not be changed by rules afterwards.
pub fn set_category(v: &mut Vault, txn_ids: &[u64], category: &str) -> Result<usize, String> {
    require_category(v, category)?;
    let ids: HashSet<u64> = txn_ids.iter().copied().collect();
    let mut n = 0;
    for t in v.txns.iter_mut().filter(|t| ids.contains(&t.id)) {
        t.category = category.to_string();
        t.source = CatSource::User;
        t.rule_id = None;
        n += 1;
    }
    detect_transfers(v);
    Ok(n)
}

/// Let rules decide the category again.
pub fn reset_category(v: &mut Vault, txn_ids: &[u64]) -> usize {
    let ids: HashSet<u64> = txn_ids.iter().copied().collect();
    let mut n = 0;
    for t in v.txns.iter_mut().filter(|t| ids.contains(&t.id) && t.source == CatSource::User) {
        t.source = CatSource::Default;
        n += 1;
    }
    recategorize_all(v);
    detect_transfers(v);
    n
}

pub fn set_note(v: &mut Vault, txn_id: u64, note: &str) -> Result<(), String> {
    let t = v.txns.iter_mut().find(|t| t.id == txn_id).ok_or("Transaction not found")?;
    t.note = note.trim().to_string();
    Ok(())
}

/// How many transactions a rule pattern would match (for the "create rule" dialog).
pub fn rule_matches(v: &Vault, pattern: &str, direction: Option<Direction>) -> usize {
    let pat = crate::categorize::normalize(pattern).trim().to_string();
    v.txns
        .iter()
        .filter(|t| direction.map(|d| d == t.direction).unwrap_or(true))
        .filter(|t| crate::categorize::pattern_matches(&crate::categorize::normalize(&t.narration), &pat))
        .count()
}

pub fn add_rule(v: &mut Vault, pattern: &str, category: &str, direction: Option<Direction>) -> Result<u64, String> {
    require_category(v, category)?;
    let norm = crate::categorize::normalize(pattern).trim().to_string();
    if norm.len() < 3 {
        return Err("The rule text must have at least 3 letters or digits.".into());
    }
    let id = v.alloc_id();
    v.rules.push(Rule { id, pattern: pattern.trim().to_string(), category: category.to_string(), direction, created: now_string() });
    recategorize_all(v);
    detect_transfers(v);
    Ok(id)
}

pub fn delete_rule(v: &mut Vault, id: u64) -> Result<(), String> {
    let before = v.rules.len();
    v.rules.retain(|r| r.id != id);
    if v.rules.len() == before {
        return Err("Rule not found".into());
    }
    recategorize_all(v);
    detect_transfers(v);
    Ok(())
}

fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let s = s.split('_').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("_");
    if s.is_empty() { "custom".into() } else { s }
}

pub fn add_category(v: &mut Vault, name: &str, kind: CatKind, color: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Please enter a category name.".into());
    }
    if v.categories.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
        return Err("A category with this name already exists.".into());
    }
    let base = format!("u_{}", slug(name));
    let mut id = base.clone();
    let mut n = 2;
    while v.category(&id).is_some() {
        id = format!("{base}_{n}");
        n += 1;
    }
    v.categories.push(Category { id: id.clone(), name: name.to_string(), kind, color: color.to_string(), builtin: false });
    Ok(id)
}

/// Rename or recolour any category; the kind can only be changed on user-created ones.
pub fn update_category(v: &mut Vault, id: &str, name: &str, color: &str, kind: Option<CatKind>) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Please enter a category name.".into());
    }
    if v.categories.iter().any(|c| c.id != id && c.name.eq_ignore_ascii_case(name)) {
        return Err("A category with this name already exists.".into());
    }
    let c = v.categories.iter_mut().find(|c| c.id == id).ok_or("Category not found")?;
    c.name = name.to_string();
    c.color = color.to_string();
    if let Some(k) = kind {
        if c.builtin && k != c.kind {
            return Err("The type of a built-in category cannot be changed.".into());
        }
        c.kind = k;
    }
    Ok(())
}

pub fn delete_category(v: &mut Vault, id: &str) -> Result<(), String> {
    let c = v.category(id).ok_or("Category not found")?;
    if c.builtin {
        return Err("Built-in categories cannot be deleted (you can rename them).".into());
    }
    v.categories.retain(|c| c.id != id);
    v.rules.retain(|r| r.category != id);
    for t in v.txns.iter_mut().filter(|t| t.category == id) {
        t.source = CatSource::Default;
    }
    recategorize_all(v);
    detect_transfers(v);
    Ok(())
}

pub fn update_account(v: &mut Vault, id: u64, nickname: &str, holder: Option<&str>) -> Result<(), String> {
    let a = v.accounts.iter_mut().find(|a| a.id == id).ok_or("Account not found")?;
    a.nickname = nickname.trim().to_string();
    if let Some(h) = holder.map(str::trim).filter(|h| !h.is_empty()) {
        if !a.holder_entered {
            return Err("The holder name comes from the bank statement and cannot be changed.".into());
        }
        a.holder = holder::clean_name(h);
    }
    detect_transfers(v);
    Ok(())
}

pub fn delete_account(v: &mut Vault, id: u64) -> Result<(), String> {
    if v.account(id).is_none() {
        return Err("Account not found".into());
    }
    v.accounts.retain(|a| a.id != id);
    v.txns.retain(|t| t.account_id != id);
    v.imports.retain(|i| i.account_id != id);
    detect_transfers(v);
    Ok(())
}

/// Undo an import: removes the transactions it added.
pub fn delete_import(v: &mut Vault, import_id: u64) -> Result<usize, String> {
    if !v.imports.iter().any(|i| i.id == import_id) {
        return Err("Import not found".into());
    }
    let before = v.txns.len();
    v.txns.retain(|t| t.import_id != import_id);
    v.imports.retain(|i| i.id != import_id);
    detect_transfers(v);
    Ok(before - v.txns.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ParsedTxn, parse_statement};

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn stmt(bank: &str, num: &str, holder: &str, txns: Vec<(&str, &str, Paise, Direction)>) -> ParsedStatement {
        ParsedStatement {
            bank: bank.into(),
            account_number: num.into(),
            holder: holder.into(),
            pan: None,
            ifsc: None,
            period_from: None,
            period_to: None,
            warnings: vec![],
            txns: txns
                .into_iter()
                .map(|(date, narr, amount, direction)| ParsedTxn {
                    date: d(date),
                    value_date: None,
                    narration: narr.into(),
                    reference: String::new(),
                    amount,
                    direction,
                    balance: None,
                })
                .collect(),
        }
    }

    #[test]
    fn import_twice_is_deduplicated() {
        let mut v = Vault::new();
        let s = parse_statement(include_bytes!("../tests/fixtures/hdfc_sample.csv"), "h.csv").unwrap();
        let p = preview(&v, &s, "h.csv");
        assert_eq!((p.new_count, p.duplicates, p.existing_account), (6, 0, None));
        let r = commit(&mut v, &s, "h.csv", None).unwrap();
        assert_eq!((r.added, r.skipped), (6, 0));
        let p = preview(&v, &s, "h.csv");
        assert_eq!((p.new_count, p.duplicates, p.existing_account), (0, 6, Some(r.account_id)));
        assert_eq!(commit(&mut v, &s, "h.csv", None).unwrap().added, 0);
        assert_eq!(v.accounts.len(), 1);
        let swiggy = v.txns.iter().find(|t| t.narration.contains("SWIGGY")).unwrap();
        assert_eq!((swiggy.category.as_str(), swiggy.merchant.as_str(), swiggy.channel.as_str()), ("food", "SWIGGY", "UPI"));
    }

    #[test]
    fn same_day_repeats_kept() {
        let mut v = Vault::new();
        let s = stmt("SBI", "12345678901", "A B", vec![("2025-04-01", "TEA", 1000, Direction::Debit), ("2025-04-01", "TEA", 1000, Direction::Debit)]);
        assert_eq!(commit(&mut v, &s, "x", None).unwrap().added, 2);
        assert_eq!(commit(&mut v, &s, "x", None).unwrap().added, 0);
    }

    #[test]
    fn holder_required_when_missing() {
        let mut v = Vault::new();
        let s = stmt("SBI", "12345678901", "", vec![("2025-04-01", "TEA", 1000, Direction::Debit)]);
        assert!(commit(&mut v, &s, "x", None).is_err());
        commit(&mut v, &s, "x", Some("mr harsh sharma")).unwrap();
        assert_eq!(v.accounts[0].holder, "Harsh Sharma");
        assert!(v.accounts[0].holder_entered);
    }

    #[test]
    fn transfers_paired_only_for_same_holder() {
        let mut v = Vault::new();
        let a = stmt("SBI", "11111111111", "Harsh Sharma", vec![("2025-04-01", "TO TRANSFER-UPI/DR/1/HARSH/HDFC/x@y", 500000, Direction::Debit)]);
        let b = stmt("HDFC", "50100000000001", "MR HARSH SHARMA", vec![("2025-04-02", "UPI-HARSH SHARMA-X@Y-1-UPI", 500000, Direction::Credit)]);
        let c = stmt("HDFC", "50100000000002", "Priya Sharma", vec![("2025-04-01", "UPI-HARSH-X@Y-1-UPI", 500000, Direction::Credit)]);
        commit(&mut v, &a, "a", None).unwrap();
        commit(&mut v, &c, "c", None).unwrap();
        assert!(v.txns.iter().all(|t| t.category != OWN_TRANSFER));
        commit(&mut v, &b, "b", None).unwrap();
        let own: Vec<_> = v.txns.iter().filter(|t| t.category == OWN_TRANSFER).collect();
        assert_eq!(own.len(), 2);
        assert!(own.iter().all(|t| v.account(t.account_id).unwrap().holder != "Priya Sharma"));
        // Deleting one side un-pairs the other.
        let bid = v.accounts.iter().find(|a| a.bank == "HDFC" && a.holder == "Harsh Sharma").unwrap().id;
        delete_account(&mut v, bid).unwrap();
        assert!(v.txns.iter().all(|t| t.category != OWN_TRANSFER));
    }

    #[test]
    fn rules_and_categories() {
        let mut v = Vault::new();
        let s = stmt("SBI", "12345678901", "A B", vec![
            ("2025-04-01", "UPI/DR/1/RAMESH KIRANA/x@ybl", 10000, Direction::Debit),
            ("2025-04-02", "UPI/DR/2/SURESH/suresh@ybl", 20000, Direction::Debit),
        ]);
        commit(&mut v, &s, "x", None).unwrap();
        assert_eq!(v.txns[1].category, "other");
        assert_eq!(rule_matches(&v, "suresh@ybl", None), 1);
        let cat = add_category(&mut v, "Maid & Help", CatKind::Expense, "#123456").unwrap();
        assert_eq!(cat, "u_maid_help");
        let rid = add_rule(&mut v, "suresh@ybl", &cat, Some(Direction::Debit)).unwrap();
        assert_eq!((v.txns[1].category.as_str(), v.txns[1].rule_id), (cat.as_str(), Some(rid)));
        let first = v.txns[0].id;
        set_category(&mut v, &[first], "food").unwrap();
        delete_rule(&mut v, rid).unwrap();
        assert_eq!(v.txns[1].category, "other");
        assert_eq!(v.txns[0].category, "food");
        reset_category(&mut v, &[first]);
        assert_eq!(v.txns[0].category, "groceries");
        assert!(delete_category(&mut v, "food").is_err());
        add_rule(&mut v, "SURESH", &cat, None).unwrap();
        delete_category(&mut v, &cat).unwrap();
        assert!(v.rules.is_empty());
        assert_eq!(v.txns[1].category, "other");
        assert!(add_rule(&mut v, "a", "food", None).is_err());
    }
}
