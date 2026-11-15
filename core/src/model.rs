use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// All amounts are stored in paise (1/100 rupee) to avoid floating point errors.
pub type Paise = i64;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CatKind {
    Expense,
    Income,
    Investment,
    Excluded,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub kind: CatKind,
    pub color: String,
    pub builtin: bool,
}

/// How a transaction got its category.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum CatSource {
    /// No rule matched; fallback category.
    Default,
    /// A built-in or user rule matched.
    Rule,
    /// The user set it by hand; never overwritten automatically.
    User,
    /// Detected as a transfer between the user's own accounts.
    Transfer,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub id: u64,
    /// "SBI", "HDFC" or "Other".
    pub bank: String,
    /// Account number as printed on the statement (may be partly masked).
    pub number: String,
    pub holder: String,
    /// True when the holder name was typed by the user because the statement had none.
    #[serde(default)]
    pub holder_entered: bool,
    pub pan: Option<String>,
    pub nickname: String,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Txn {
    pub id: u64,
    pub account_id: u64,
    pub date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub narration: String,
    pub reference: String,
    pub amount: Paise,
    pub direction: Direction,
    pub balance: Option<Paise>,
    pub merchant: String,
    pub vpa: Option<String>,
    pub channel: String,
    pub category: String,
    pub source: CatSource,
    pub rule_id: Option<u64>,
    /// De-duplication key, unique per account.
    pub key: String,
    pub import_id: u64,
    #[serde(default)]
    pub note: String,
}

/// A user-defined categorisation rule. Built-in rules live in code (see `categorize`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: u64,
    pub pattern: String,
    pub category: String,
    pub direction: Option<Direction>,
    pub created: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImportRecord {
    pub id: u64,
    pub file_name: String,
    pub account_id: u64,
    pub imported_at: String,
    pub period_from: Option<NaiveDate>,
    pub period_to: Option<NaiveDate>,
    pub added: usize,
    pub skipped: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    pub investments_as_expense: bool,
    pub auto_lock_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { investments_as_expense: false, auto_lock_minutes: 15 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vault {
    pub version: u32,
    pub next_id: u64,
    pub accounts: Vec<Account>,
    pub txns: Vec<Txn>,
    pub categories: Vec<Category>,
    pub rules: Vec<Rule>,
    pub imports: Vec<ImportRecord>,
    #[serde(default)]
    pub settings: Settings,
}

impl Vault {
    pub fn new() -> Self {
        Vault {
            version: 1,
            next_id: 1,
            accounts: Vec::new(),
            txns: Vec::new(),
            categories: crate::categorize::default_categories(),
            rules: Vec::new(),
            imports: Vec::new(),
            settings: Settings::default(),
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.id == id)
    }

    pub fn account(&self, id: u64) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }
}

impl Default for Vault {
    fn default() -> Self {
        Self::new()
    }
}

/// Short display label for an account, e.g. "SBI ••1234".
pub fn account_label(a: &Account) -> String {
    if !a.nickname.trim().is_empty() {
        return a.nickname.trim().to_string();
    }
    let digits: String = a.number.chars().filter(|c| c.is_ascii_digit()).collect();
    let tail = if digits.len() >= 4 { &digits[digits.len() - 4..] } else { digits.as_str() };
    if tail.is_empty() {
        a.bank.clone()
    } else {
        format!("{} ••{}", a.bank, tail)
    }
}

pub fn now_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}
