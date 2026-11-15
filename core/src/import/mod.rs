//! Bank statement parsing: finds the transaction table, reads rows, and picks up account details
//! (bank, account number, holder, PAN) from the lines above the table.

pub mod grid;

use crate::holder::{clean_name, normalize_pan};
use crate::model::{Direction, Paise};
use crate::money::{parse_amount, parse_date};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedTxn {
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub narration: String,
    pub reference: String,
    pub amount: Paise,
    pub direction: Direction,
    pub balance: Option<Paise>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParsedStatement {
    pub bank: String,
    pub account_number: String,
    pub holder: String,
    pub pan: Option<String>,
    pub ifsc: Option<String>,
    pub period_from: Option<NaiveDate>,
    pub period_to: Option<NaiveDate>,
    pub txns: Vec<ParsedTxn>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Date,
    ValueDate,
    Narration,
    Reference,
    Debit,
    Credit,
    Amount,
    DrCr,
    Balance,
}

fn compact(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

fn role_of(cell: &str) -> Option<Role> {
    let c = compact(cell);
    if c.is_empty() || c.len() > 40 {
        return None;
    }
    let has = |w: &str| c.contains(w);
    if has("value") && (has("dat") || has("dt")) {
        Some(Role::ValueDate)
    } else if ["drcr", "crdr", "dc", "cd", "drcrindicator", "debitcreditindicator", "txntype", "transactiontype", "type"].contains(&c.as_str()) {
        Some(Role::DrCr)
    } else if has("balance") || c == "bal" || c == "closingbal" {
        Some(Role::Balance)
    } else if has("date") || c == "txndt" || c == "trandt" || c == "dt" {
        Some(Role::Date)
    } else if has("narration") || has("description") || has("particular") || has("detail") || has("remark") {
        Some(Role::Narration)
    } else if has("ref") || has("chq") || has("cheque") || c == "utr" || c == "instrumentno" {
        Some(Role::Reference)
    } else if has("withdrawal") || has("paidout") || ((has("debit") || c == "dr") && !has("credit")) {
        Some(Role::Debit)
    } else if has("deposit") || has("paidin") || ((has("credit") || c == "cr") && !has("debit")) {
        Some(Role::Credit)
    } else if has("amount") || has("amt") || c == "inr" {
        Some(Role::Amount)
    } else {
        None
    }
}

#[derive(Default, Debug)]
struct Columns {
    date: Option<usize>,
    value_date: Option<usize>,
    narration: Option<usize>,
    reference: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
    amount: Option<usize>,
    drcr: Option<usize>,
    balance: Option<usize>,
}

impl Columns {
    fn from_row(row: &[String]) -> Columns {
        let mut c = Columns::default();
        for (i, cell) in row.iter().enumerate() {
            let slot = match role_of(cell) {
                Some(Role::Date) => &mut c.date,
                Some(Role::ValueDate) => &mut c.value_date,
                Some(Role::Narration) => &mut c.narration,
                Some(Role::Reference) => &mut c.reference,
                Some(Role::Debit) => &mut c.debit,
                Some(Role::Credit) => &mut c.credit,
                Some(Role::Amount) => &mut c.amount,
                Some(Role::DrCr) => &mut c.drcr,
                Some(Role::Balance) => &mut c.balance,
                None => continue,
            };
            if slot.is_none() {
                *slot = Some(i);
            }
        }
        c
    }

    fn usable(&self) -> bool {
        self.date.is_some() && self.narration.is_some() && ((self.debit.is_some() && self.credit.is_some()) || self.amount.is_some())
    }
}

fn find_header(g: &grid::Grid) -> Option<(usize, Columns)> {
    g.iter().take(80).enumerate().find_map(|(i, row)| {
        let c = Columns::from_row(row);
        if c.usable() { Some((i, c)) } else { None }
    })
}

fn get(row: &[String], idx: Option<usize>) -> &str {
    idx.and_then(|i| row.get(i)).map(|s| s.as_str()).unwrap_or("")
}

const FOOTER: &[&str] = &[
    "statement summary", "computer generated", "computer-generated", "system generated", "dr count", "cr count",
    "opening balance", "closing balance", "end of statement", "generated on", "**end", "page total",
    "grand total",
];

fn is_footer(row: &[String]) -> bool {
    let text = row.join(" ").to_lowercase();
    FOOTER.iter().any(|f| text.contains(f))
}

fn drcr_marker(s: &str) -> Option<Direction> {
    match compact(s).as_str() {
        "dr" | "d" | "debit" | "db" | "withdrawal" => Some(Direction::Debit),
        "cr" | "c" | "credit" | "deposit" => Some(Direction::Credit),
        _ => None,
    }
}

struct Row {
    txn: ParsedTxn,
    dir_known: bool,
}

fn parse_rows(g: &grid::Grid, h: usize, c: &Columns, warnings: &mut Vec<String>) -> Vec<ParsedTxn> {
    let mut rows: Vec<Row> = Vec::new();
    for row in g.iter().skip(h + 1) {
        if row.iter().all(|s| s.trim().is_empty()) {
            continue;
        }
        let narration = get(row, c.narration).trim().to_string();
        let date = parse_date(get(row, c.date));
        let debit = parse_amount(get(row, c.debit)).filter(|a| a.paise != 0);
        let credit = parse_amount(get(row, c.credit)).filter(|a| a.paise != 0);
        let amount = parse_amount(get(row, c.amount)).filter(|a| a.paise != 0);
        let Some(date) = date else {
            let no_amounts = debit.is_none() && credit.is_none() && amount.is_none();
            if get(row, c.date).trim().is_empty() && !narration.is_empty() && no_amounts && !rows.is_empty() && !is_footer(row) {
                // Narration wrapped onto the next line.
                let last = &mut rows.last_mut().unwrap().txn;
                last.narration = format!("{} {}", last.narration, narration).trim().to_string();
                continue;
            }
            if !rows.is_empty() && is_footer(row) {
                break;
            }
            continue;
        };
        let (paise, direction, known) = if c.debit.is_some() && c.credit.is_some() {
            match (debit, credit) {
                (Some(d), _) => (d.paise, Direction::Debit, true),
                (None, Some(cr)) => (cr.paise, Direction::Credit, true),
                (None, None) => match amount {
                    Some(a) => (a.paise, if a.signed() < 0 { Direction::Debit } else { Direction::Credit }, a.negative || a.marker.is_some()),
                    None => continue,
                },
            }
        } else {
            let Some(a) = amount else { continue };
            if let Some(d) = drcr_marker(get(row, c.drcr)) {
                (a.paise, d, true)
            } else if let Some(d) = a.marker {
                (a.paise, d, true)
            } else if a.negative {
                (a.paise, Direction::Debit, true)
            } else {
                (a.paise, Direction::Credit, false)
            }
        };
        let balance = parse_amount(get(row, c.balance)).map(|b| b.signed());
        rows.push(Row {
            txn: ParsedTxn {
                date,
                value_date: parse_date(get(row, c.value_date)),
                narration,
                reference: get(row, c.reference).trim().to_string(),
                amount: paise,
                direction,
                balance,
            },
            dir_known: known,
        });
    }

    if rows.len() >= 2 && rows.first().unwrap().txn.date > rows.last().unwrap().txn.date {
        rows.reverse();
    }

    // Unsigned "Amount" column without a Dr/Cr marker: work out the direction from the balance.
    let mut unresolved = 0;
    for i in 0..rows.len() {
        if rows[i].dir_known {
            continue;
        }
        let prev = if i > 0 { rows[i - 1].txn.balance } else { None };
        match (prev, rows[i].txn.balance) {
            (Some(p), Some(b)) if b - p == -rows[i].txn.amount => rows[i].txn.direction = Direction::Debit,
            (Some(p), Some(b)) if b - p == rows[i].txn.amount => rows[i].txn.direction = Direction::Credit,
            _ => unresolved += 1,
        }
    }
    if unresolved > 0 && rows.iter().any(|r| r.dir_known) {
        warnings.push(format!("{unresolved} row(s) had no debit/credit marker and were treated as credits. Please check them."));
    }

    let txns: Vec<ParsedTxn> = rows.into_iter().map(|r| r.txn).collect();
    let mut mismatches = 0;
    for w in txns.windows(2) {
        if let (Some(p), Some(b)) = (w[0].balance, w[1].balance) {
            let delta = if w[1].direction == Direction::Debit { -w[1].amount } else { w[1].amount };
            if p + delta != b {
                mismatches += 1;
            }
        }
    }
    if mismatches > 0 {
        warnings.push(format!(
            "The running balance does not add up at {mismatches} place(s). Some rows may be missing from the file or were read incorrectly."
        ));
    }
    txns
}

static IFSC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Z]{4}0[A-Z0-9]{6})\b").unwrap());
static PAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([A-Z]{5}[0-9]{4}[A-Z])\b").unwrap());
static ACCT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:account\s*(?:no|number|num)|a/?c\s*(?:no|number)?)\b\.?\s*[:\-]?\s*[_']?\s*([0-9Xx*]{6,24})").unwrap()
});
static TITLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(MR|MRS|MS|MISS|DR|SHRI|SMT|M/S)\.?\s+[A-Z][A-Za-z .]{2,60}$").unwrap());

/// Value for a "Label : value" cell, or the next non-empty cell after a "Label" / "Label :" cell.
fn labelled(rows: &[Vec<String>], labels: &[&str]) -> Option<String> {
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let (key, rest) = match cell.find(':') {
                Some(p) => (&cell[..p], cell[p + 1..].trim()),
                None => (cell.as_str(), ""),
            };
            if !labels.contains(&compact(key).as_str()) {
                continue;
            }
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
            let next = row[i + 1..].iter().map(|s| s.trim().trim_start_matches(':').trim()).find(|s| !s.is_empty());
            if let Some(n) = next {
                return Some(n.to_string());
            }
        }
    }
    None
}

/// Account number as digits (leading zeros removed) or a masked form like "XXXXXX1234".
pub fn normalize_account(s: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_digit() || matches!(c, 'X' | 'x' | '*')).collect::<String>().to_uppercase();
    if t.chars().all(|c| c.is_ascii_digit()) {
        let z = t.trim_start_matches('0');
        if z.is_empty() { t } else { z.to_string() }
    } else {
        t
    }
}

fn detect_bank(text_upper: &str, ifsc: &Option<String>, header: &[String], file_name: &str) -> String {
    if text_upper.contains("HDFC BANK") || ifsc.as_deref().map(|i| i.starts_with("HDFC")).unwrap_or(false) {
        return "HDFC".into();
    }
    if text_upper.contains("STATE BANK OF INDIA") || ifsc.as_deref().map(|i| i.starts_with("SBIN")).unwrap_or(false) {
        return "SBI".into();
    }
    let h: Vec<String> = header.iter().map(|s| compact(s)).collect();
    let hh = |w: &str| h.iter().any(|c| c == w);
    if hh("withdrawalamt") || hh("depositamt") || hh("chqrefno") || hh("debitamount") && hh("chqrefnumber") {
        return "HDFC".into();
    }
    if hh("txndate") && (hh("refnochequeno") || hh("description")) {
        return "SBI".into();
    }
    let f = file_name.to_lowercase();
    if f.contains("hdfc") {
        "HDFC".into()
    } else if f.contains("sbi") {
        "SBI".into()
    } else {
        "Other".into()
    }
}

pub fn parse_statement(bytes: &[u8], file_name: &str) -> Result<ParsedStatement, String> {
    let grids = grid::read(bytes)?;
    let (g, h, cols) = grids
        .iter()
        .find_map(|g| find_header(g).map(|(h, c)| (g, h, c)))
        .ok_or("Could not find the transaction table. Expected columns like Date, Narration/Description, Debit/Withdrawal, Credit/Deposit and Balance.")?;

    let mut warnings = Vec::new();
    let txns = parse_rows(g, h, &cols, &mut warnings);
    if txns.is_empty() {
        return Err("The transaction table was found but it has no rows.".into());
    }

    let pre = &g[..h];
    // Some banks repeat details below the table, so look there too.
    let post: Vec<Vec<String>> = g.iter().skip(h + 1).filter(|r| parse_date(get(r, cols.date)).is_none()).cloned().collect();
    let meta_rows: Vec<Vec<String>> = pre.iter().cloned().chain(post).collect();
    let text: String = meta_rows.iter().map(|r| r.join(" ")).collect::<Vec<_>>().join("\n");
    let upper = text.to_uppercase();

    let ifsc = labelled(&meta_rows, &["ifsc", "ifsccode", "ifscode", "rtgsneftifsc"])
        .and_then(|v| IFSC.captures(&v.to_uppercase()).map(|c| c[1].to_string()))
        .or_else(|| IFSC.captures(&upper).map(|c| c[1].to_string()));
    let bank = detect_bank(&upper, &ifsc, &g[h], file_name);

    let account_number = labelled(&meta_rows, &["accountno", "accountnumber", "acno", "accno", "acnumber", "accountnum"])
        .map(|v| normalize_account(&v))
        .filter(|v| v.chars().filter(|c| c.is_ascii_digit()).count() >= 4)
        .or_else(|| ACCT.captures(&text).map(|c| normalize_account(&c[1])))
        .unwrap_or_default();

    let holder = labelled(&meta_rows, &["accountname", "customername", "accountholder", "accountholdername", "acname", "name", "holdername"])
        .filter(|v| v.chars().any(|c| c.is_ascii_alphabetic()))
        .or_else(|| meta_rows.iter().flatten().find(|c| TITLED.is_match(c.trim())).cloned())
        .map(|n| clean_name(&n))
        .unwrap_or_default();

    let pan = labelled(&meta_rows, &["pan", "panno", "pannumber", "customerpan", "pancard"])
        .and_then(|v| normalize_pan(&v))
        .or_else(|| text.lines().filter(|l| l.to_uppercase().contains("PAN")).find_map(|l| PAN.captures(&l.to_uppercase()).map(|c| c[1].to_string())));

    let period_from = txns.iter().map(|t| t.date).min();
    let period_to = txns.iter().map(|t| t.date).max();
    if account_number.is_empty() {
        warnings.push("Account number not found in the file.".into());
    }
    Ok(ParsedStatement { bank, account_number, holder, pan, ifsc, period_from, period_to, txns, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDFC: &str = include_str!("../../tests/fixtures/hdfc_sample.csv");
    const SBI: &str = include_str!("../../tests/fixtures/sbi_sample.xls");

    #[test]
    fn hdfc_csv() {
        let s = parse_statement(HDFC.as_bytes(), "Acct_Statement.csv").unwrap();
        assert_eq!(s.bank, "HDFC");
        assert_eq!(s.account_number, "50100123456789");
        assert_eq!(s.holder, "Harsh Sharma");
        assert_eq!(s.ifsc.as_deref(), Some("HDFC0001234"));
        assert_eq!(s.txns.len(), 6);
        let t = &s.txns[0];
        assert_eq!(t.date, NaiveDate::from_ymd_opt(2025, 4, 1).unwrap());
        assert_eq!((t.amount, t.direction), (45_000, Direction::Debit));
        assert!(t.narration.starts_with("UPI-SWIGGY"));
        let sal = s.txns.iter().find(|t| t.narration.contains("SALARY")).unwrap();
        assert_eq!((sal.amount, sal.direction), (85_000_00, Direction::Credit));
        assert!(s.txns[2].narration.ends_with("CONTINUED TEXT"), "{:?}", s.txns[2].narration);
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    #[test]
    fn sbi_tsv() {
        let s = parse_statement(SBI.as_bytes(), "1234.xls").unwrap();
        assert_eq!(s.bank, "SBI");
        assert_eq!(s.account_number, "31234567890");
        assert_eq!(s.holder, "Harsh Sharma");
        assert_eq!(s.pan, None);
        assert_eq!(s.txns.len(), 5);
        // File is newest-first; parser returns oldest-first.
        assert!(s.txns[0].date <= s.txns[4].date);
        assert_eq!(s.txns[0].direction, Direction::Credit);
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    #[test]
    fn signed_amount_column_and_balance_inference() {
        let csv = "Name: MR TEST USER\nAccount Number: 'XXXXXXX9876\nDate,Details,Amount,Balance\n01-04-2025,Opening,,1000.00\n02-04-2025,UPI/ZEPTO,-200.00,800.00\n03-04-2025,INTEREST,50.00,850.00\n";
        let s = parse_statement(csv.as_bytes(), "x.csv").unwrap();
        assert_eq!(s.txns.len(), 2);
        assert_eq!(s.txns[0].direction, Direction::Debit);
        assert_eq!(s.txns[1].direction, Direction::Credit);
        assert_eq!(s.account_number, "XXXXXXX9876");
        assert_eq!(s.holder, "Test User");

        let csv = "Date,Narration,Amount,Balance\n01/04/2025,A,100.00,900.00\n02/04/2025,B,100.00,800.00\n03/04/2025,C,300.00,1100.00\n";
        let s = parse_statement(csv.as_bytes(), "x.csv").unwrap();
        assert_eq!(s.txns[1].direction, Direction::Debit);
        assert_eq!(s.txns[2].direction, Direction::Credit);
    }

    #[test]
    fn no_table() {
        assert!(parse_statement(b"hello,world\n1,2\n", "x.csv").is_err());
    }
}
