//! Default categories, built-in keyword rules and the rule engine.
//!
//! Matching works on normalised text (upper case, anything that is not a letter or digit becomes a
//! space). A pattern must start at a word boundary; short patterns (4 characters or fewer) must also
//! end at one, so "OLA" does not match "COLABA" and "RENT" does not match "CURRENT".
//! User rules are tried first (newest first), then built-in rules, longest pattern first.

use crate::model::{CatKind, CatSource, Category, Direction, Paise, Rule, Txn, Vault};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const FALLBACK_DEBIT: &str = "other";
pub const FALLBACK_CREDIT: &str = "other_income";
pub const OWN_TRANSFER: &str = "own_transfer";
pub const REVERSAL: &str = "reversal";

pub fn default_categories() -> Vec<Category> {
    use CatKind::*;
    let list: &[(&str, &str, CatKind, &str)] = &[
        ("groceries", "Groceries", Expense, "#2e7d32"),
        ("food", "Food & Dining", Expense, "#ef6c00"),
        ("fuel", "Fuel", Expense, "#6d4c41"),
        ("transport", "Transport", Expense, "#00838f"),
        ("travel", "Travel", Expense, "#1565c0"),
        ("shopping", "Shopping", Expense, "#ad1457"),
        ("entertainment", "Entertainment", Expense, "#6a1b9a"),
        ("healthcare", "Healthcare", Expense, "#c62828"),
        ("rent", "Rent & Housing", Expense, "#4e342e"),
        ("utilities", "Bills & Utilities", Expense, "#f9a825"),
        ("education", "Education", Expense, "#283593"),
        ("insurance", "Insurance", Expense, "#00695c"),
        ("emi", "Loans & EMI", Expense, "#5d4037"),
        ("cash", "Cash Withdrawal", Expense, "#757575"),
        ("fees", "Bank Fees & Charges", Expense, "#9e9d24"),
        ("taxes", "Taxes", Expense, "#827717"),
        ("gifts", "Gifts & Donations", Expense, "#d81b60"),
        ("family", "Family & Friends", Expense, "#8d6e63"),
        ("other", "Other Expenses", Expense, "#90a4ae"),
        ("investments", "Investments", Investment, "#3949ab"),
        ("salary", "Salary", Income, "#1b5e20"),
        ("interest", "Interest", Income, "#558b2f"),
        ("dividends", "Dividends", Income, "#7cb342"),
        ("refunds", "Refunds & Cashback", Income, "#26a69a"),
        ("other_income", "Other Income", Income, "#80cbc4"),
        ("own_transfer", "Own Account Transfer", Excluded, "#b0bec5"),
        ("cc_payment", "Credit Card Payment", Excluded, "#cfd8dc"),
        ("reversal", "Reversed / Failed Payment", Excluded, "#e0e0e0"),
    ];
    list.iter()
        .map(|(id, name, kind, color)| Category {
            id: id.to_string(),
            name: name.to_string(),
            kind: *kind,
            color: color.to_string(),
            builtin: true,
        })
        .collect()
}

/// Built-in keyword rules: (category, patterns).
const BUILTIN: &[(&str, &[&str])] = &[
    // Listed first so they win ties against merchant keywords of equal length.
    ("refunds", &[
        "REFUND", "REFUNDS", "REVERSAL", "REV", "CASHBACK", "CASH BACK", "CHARGEBACK", "REVERSED", "RETURNED", "UPI DRC",
        "UPI REF", "UPI REV",
    ]),
    ("groceries", &[
        "BLINKIT", "GROFERS", "ZEPTO", "BIGBASKET", "BIG BASKET", "INNOVATIVE RETAIL", "DMART", "D MART", "AVENUE SUPERMARTS",
        "JIOMART", "JIO MART", "RELIANCE FRESH", "RELIANCE SMART", "RELIANCE RETAIL", "MORE RETAIL", "MORE SUPERMARKET", "SPENCER",
        "NATURES BASKET", "STAR BAZAAR", "INSTAMART", "SWIGGY INSTAMART", "DUNZO", "KIRANA", "SUPERMARKET", "SUPER MARKET",
        "GROCERY", "GROCERIES", "GROCER", "PROVISION", "MILKBASKET", "COUNTRY DELIGHT", "LICIOUS", "FRESHTOHOME", "VEGETABLE", "DAIRY",
        "VEGETABLES", "FRUIT", "FRUITS", "SABZI", "SABJI", "MILK",
    ]),
    ("food", &[
        "SWIGGY", "BUNDL TECHNOLOGIES", "ZOMATO", "EATSURE", "DOMINOS", "DOMINO S", "JUBILANT FOODWORKS", "MCDONALD", "KFC",
        "PIZZA HUT", "BURGER KING", "STARBUCKS", "CAFE COFFEE DAY", "CCD", "SUBWAY", "HALDIRAM", "RESTAURANT", "RESTO", "CAFE",
        "BAKERY", "BAKERS", "CHAAYOS", "BARBEQUE NATION", "SWEETS", "DHABA", "HOTEL AND RESTAURANT", "FOODS", "BIRYANI",
        "HOTEL", "PIZZA", "UDUPI", "TEA", "CHAI", "JUICE", "SNACKS", "FOOD", "VADA PAV", "CHINESE",
    ]),
    ("fuel", &[
        "HPCL", "BPCL", "IOCL", "INDIAN OIL", "HINDUSTAN PETROLEUM", "BHARAT PETROLEUM", "PETROL", "PETROLEUM", "FUEL", "FUELS",
        "SHELL", "NAYARA", "FILLING STATION", "SERVICE STATION", "HP PAY", "IOCL FUEL", "PETRO", "AUTO FUEL",
        // SBI cuts payee names to 8 characters: "SAI PETR", "OM FILLI".
        "PETR", "FILL", "FILLI", "FILLIN", "FILLING", "PETROL PUMP",
    ]),
    ("transport", &[
        "UBER", "OLA", "OLACABS", "OLA CABS", "ANI TECHNOLOGIES", "RAPIDO", "ROPPEN", "FASTAG", "NETC", "METRO", "MSRTC",
        "PARKING", "TOLL", "NAMMA YATRI", "BLUSMART", "YULU", "MUMBAI METRO", "DMRC", "BMRCL", "RIDE",
    ]),
    ("travel", &[
        "IRCTC", "MAKEMYTRIP", "MAKE MY TRIP", "GOIBIBO", "CLEARTRIP", "YATRA", "EASEMYTRIP", "IXIGO", "REDBUS", "ABHIBUS",
        "INDIGO", "INTERGLOBE", "AIR INDIA", "VISTARA", "SPICEJET", "AKASA", "OYO", "OYO ROOMS", "AIRBNB", "BOOKING COM",
        "AGODA", "TRIVAGO", "THOMAS COOK",
    ]),
    ("shopping", &[
        "AMAZON", "AMZN", "FLIPKART", "MYNTRA", "AJIO", "MEESHO", "NYKAA", "TATA CLIQ", "CROMA", "RELIANCE DIGITAL", "VIJAY SALES",
        "DECATHLON", "IKEA", "LIFESTYLE", "SHOPPERS STOP", "PANTALOONS", "WESTSIDE", "TRENT", "LENSKART", "FIRSTCRY", "PEPPERFRY",
        "URBAN LADDER", "SNAPDEAL", "UNIQLO", "ZARA", "MAX FASHION", "BATA", "TANISHQ", "APPLE STORE", "SAMSUNG", "PURPLLE",
    ]),
    ("entertainment", &[
        "NETFLIX", "HOTSTAR", "DISNEY", "JIOCINEMA", "JIOHOTSTAR", "PRIME VIDEO", "AMAZON PRIME", "SPOTIFY", "YOUTUBE", "BOOKMYSHOW",
        "BIGTREE", "PVR", "INOX", "CINEPOLIS", "SONYLIV", "ZEE5", "GAANA", "STEAM", "PLAYSTATION", "XBOX", "DISTRICT", "PAYTM INSIDER",
        "APPLE MEDIA", "GOOGLE PLAY", "CINEMA", "MULTIPLEX", "KHELOMORE", "PLAYSTORE", "PLAY STORE",
    ]),
    ("healthcare", &[
        "APOLLO", "PHARMEASY", "NETMEDS", "TATA 1MG", "1MG", "MEDPLUS", "PRACTO", "HOSPITAL", "CLINIC", "PHARMACY", "PHARMA",
        "MEDICAL", "MEDICALS", "MEDICOS", "CHEMIST", "DIAGNOSTIC", "DIAGNOSTICS", "PATHOLOGY", "LAL PATH", "THYROCARE", "DENTAL",
        "HEALTHIANS", "METROPOLIS", "DOCTOR", "DR LAL", "NURSING HOME", "WELLNESS FOREVER", "MEDICINE", "MEDICINES", "HOSP",
        "HOSPI", "CHEMISTS",
    ]),
    ("rent", &["RENT", "HOUSE RENT", "NOBROKER", "HOUSING COM", "NESTAWAY", "SOCIETY MAINTENANCE", "MAINTENANCE", "CRED RENT"]),
    ("utilities", &[
        "ELECTRICITY", "MSEDCL", "MAHAVITARAN", "ADANI ELECTRICITY", "TATA POWER", "BESCOM", "TORRENT POWER", "BSES", "TPDDL",
        "AIRTEL", "BHARTI AIRTEL", "JIO", "RELIANCE JIO", "VODAFONE", "VODAFONE IDEA", "BSNL", "MTNL", "ACT FIBERNET", "HATHWAY",
        "TATA PLAY", "TATASKY", "TATA SKY", "DISH TV", "MAHANAGAR GAS", "MGL", "IGL", "INDRAPRASTHA GAS", "GAS", "INDANE",
        "HP GAS", "BHARAT GAS", "BHARATGAS", "WATER BILL", "WATER TAX", "PROPERTY TAX", "BROADBAND", "RECHARGE", "POSTPAID",
        "PREPAID", "DTH", "BILL PAYMENT", "BILLPAY", "ELECTRIC", "TATAPLAY",
    ]),
    ("education", &[
        "SCHOOL", "COLLEGE", "UNIVERSITY", "BYJU", "BYJUS", "UNACADEMY", "COURSERA", "UDEMY", "UPGRAD", "VEDANTU", "TUITION",
        "EDUCATION", "ACADEMY", "INSTITUTE", "CLASSES", "EXAM FEE",
    ]),
    ("insurance", &[
        "LIC", "LIFE INSURANCE", "LIC OF INDIA", "INSURANCE", "HDFC LIFE", "ICICI PRU", "ICICI LOMBARD", "SBI LIFE", "SBI GENERAL",
        "STAR HEALTH", "POLICYBAZAAR", "POLICY BAZAAR", "ACKO", "GO DIGIT", "DIGIT INSURANCE", "BAJAJ ALLIANZ", "TATA AIG",
        "MAX LIFE", "NIVA BUPA", "CARE HEALTH", "PREMIUM", "STARHEALTH",
    ]),
    ("emi", &[
        "EMI", "LOAN", "HOME LOAN", "CAR LOAN", "PERSONAL LOAN", "BAJAJ FINANCE", "BAJAJ FINSERV", "LOAN RECOVERY", "HOME CREDIT",
        "TATA CAPITAL", "HDB FINANCIAL", "FULLERTON", "LENDINGKART", "KREDITBEE", "MONEYVIEW", "NAVI FINSERV",
    ]),
    ("cash", &["ATM CASH", "ATM WDL", "ATM WITHDRAWAL", "CASH WDL", "CASH WITHDRAWAL", "ATW", "NWD", "EAW", "CASH PAID", "SELF CHQ"]),
    ("fees", &[
        "CHARGES", "CHARGE", "CHRG", "CHRGS", "CHGS", "SMS ALERT", "SMS CHG", "ANNUAL FEE", "CARD FEE", "DEBIT CARD FEE", "GST",
        "IGST", "CGST", "SGST", "CONVENIENCE FEE", "PENALTY", "MIN BAL", "NON MAINTENANCE", "AMC", "PROCESSING FEE", "LATE FEE",
    ]),
    ("investments", &[
        "ZERODHA", "GROWW", "NEXTBILLION", "UPSTOX", "RKSV", "ANGEL ONE", "ANGEL BROKING", "KUVERA", "PAYTM MONEY", "COIN",
        "INDIAN CLEARING CORP", "ICCL", "BSE LIMITED", "BSE", "NSE", "NSE CLEARING", "NSDL", "CDSL", "MUTUAL FUND", "MUTUAL FUNDS",
        "MF", "SIP", "PPF", "NPS", "SGB", "FIXED DEPOSIT", "TO FD", "FD", "RD INSTALLMENT", "RECURRING DEPOSIT", "CAMS", "KFINTECH",
        "KFIN", "KARVY", "ETMONEY", "ET MONEY", "SMALLCASE", "DHAN", "RAISE SECURITIES", "CLEARING CORP", "ICICI SECURITIES",
        "HDFC SECURITIES", "KOTAK SECURITIES", "MOTILAL OSWAL", "SHAREKHAN", "5PAISA", "INDMONEY", "SUKANYA", "NPS TRUST",
    ]),
    ("salary", &["SALARY", "SAL", "SALARY CREDIT", "PAYROLL", "BONUS", "STIPEND", "WAGES", "INCENTIVE"]),
    ("interest", &["INT PD", "INTEREST", "INT CREDIT", "SB INT", "CREDIT INTEREST", "INTEREST CREDIT", "INT CR", "FD INT", "INT ON FD"]),
    ("dividends", &["DIVIDEND", "DIV", "ACH C", "ACH CR", "ACHCR", "NACH CR", "INTERIM DIVIDEND", "FINAL DIVIDEND"]),
    ("taxes", &[
        "INCOMETAX", "INCOME TAX", "ADVANCE TAX", "SELF ASSESSMENT TAX", "TAX PAYMENT", "CBDT", "TDS PAYMENT",
    ]),
    ("gifts", &[
        "DONATION", "DONATIONS", "TEMPLE", "MANDIR", "GURUDWARA", "CHARITY", "CHARITABLE", "SEWA SAMITI", "SEVA SAMITI",
        "GIFT", "GIFTS",
    ]),
    ("cc_payment", &[
        "CRED CLUB", "CREDIT CARD", "CREDITCARD", "CC BILL", "CC PAYMENT", "CARD BILL", "CC AUTOPAY", "CREDIT CARD PAYMENT",
        "ONECARD", "BOBCARD", "SBI CARD", "SBICARD", "SBI CARDS", "CARD DUES",
    ]),
];

/// Upper-case, replace every non-alphanumeric character with a space and collapse runs of spaces.
/// The result is padded with one space on each side.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    let mut last_space = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    if !last_space {
        out.push(' ');
    }
    out
}

/// `text` must come from `normalize`; `pat` is the normalised pattern without padding.
pub fn pattern_matches(text: &str, pat: &str) -> bool {
    if pat.is_empty() {
        return false;
    }
    let need_end = pat.len() <= 4;
    let tb = text.as_bytes();
    let mut from = 0;
    while let Some(pos) = text[from..].find(pat) {
        let i = from + pos;
        let start_ok = i > 0 && tb[i - 1] == b' ';
        let end = i + pat.len();
        let end_ok = !need_end || (end < tb.len() && tb[end] == b' ');
        if start_ok && end_ok {
            return true;
        }
        from = i + 1;
    }
    false
}

fn norm_pattern(p: &str) -> String {
    normalize(p).trim().to_string()
}

struct Compiled {
    pat: String,
    category: String,
    direction: Option<Direction>,
    rule_id: Option<u64>,
}

pub struct Categorizer {
    rules: Vec<Compiled>,
    /// The first `n_user` entries of `rules` are the user's rules.
    n_user: usize,
    kinds: HashMap<String, CatKind>,
}

/// "IB BILLPAY DR-HDFC92-434155XXXXXX6660": a bill payment to a (masked) card number.
static CARD_BILL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)BILL\s*PAY.*\b\d{4,6}[X*]{4,}\d{4}\b").unwrap());

/// Channels on which money can move between the holder's own accounts.
const SELF_CHANNELS: &[&str] = &["UPI", "NEFT", "RTGS", "IMPS", "OTHER"];

/// What the categoriser looks at for one transaction.
pub struct Facts<'a> {
    pub narration: &'a str,
    pub direction: Direction,
    pub amount: Paise,
    pub channel: &'a str,
    pub merchant: &'a str,
    pub vpa: Option<&'a str>,
    /// Name of the account holder, to recognise transfers to the holder's other accounts.
    pub holder: &'a str,
}

impl<'a> Facts<'a> {
    pub fn of(t: &'a Txn, holder: &'a str) -> Self {
        Facts {
            narration: &t.narration,
            direction: t.direction,
            amount: t.amount,
            channel: &t.channel,
            merchant: &t.merchant,
            vpa: t.vpa.as_deref(),
            holder,
        }
    }

    /// Normalised text identifying the payee, used to group payments to the same payee.
    pub fn payee(&self) -> String {
        normalize(&crate::narration::payee_pattern(self.narration, self.merchant, self.vpa)).trim().to_string()
    }
}

/// Directions a category can apply to when the rule does not say.
fn kind_allows(category: &str, kind: CatKind, dir: Direction) -> bool {
    match (category, kind) {
        ("refunds", _) => dir == Direction::Credit,
        ("cc_payment", _) => dir == Direction::Debit,
        (_, CatKind::Income) => dir == Direction::Credit,
        // Expense keywords on a credit (e.g. a Swiggy refund) net off against that category.
        _ => true,
    }
}

impl Categorizer {
    pub fn new(user_rules: &[Rule], categories: &[Category]) -> Self {
        let kinds: HashMap<String, CatKind> = categories.iter().map(|c| (c.id.clone(), c.kind)).collect();
        let mut rules = Vec::new();
        let mut users: Vec<&Rule> = user_rules.iter().filter(|r| kinds.contains_key(&r.category)).collect();
        users.sort_by(|a, b| b.id.cmp(&a.id));
        for r in users {
            let pat = norm_pattern(&r.pattern);
            if !pat.is_empty() {
                rules.push(Compiled { pat, category: r.category.clone(), direction: r.direction, rule_id: Some(r.id) });
            }
        }
        let n_user = rules.len();
        let mut builtin: Vec<Compiled> = Vec::new();
        for (cat, pats) in BUILTIN {
            if !kinds.contains_key(*cat) {
                continue;
            }
            for p in pats.iter() {
                builtin.push(Compiled { pat: norm_pattern(p), category: cat.to_string(), direction: None, rule_id: None });
            }
        }
        builtin.sort_by(|a, b| b.pat.len().cmp(&a.pat.len()));
        rules.extend(builtin);
        Categorizer { rules, n_user, kinds }
    }

    fn first_match(&self, rules: &[Compiled], text: &str, dir: Direction) -> Option<(String, CatSource, Option<u64>)> {
        for r in rules {
            let kind = self.kinds.get(&r.category).copied().unwrap_or(CatKind::Expense);
            let ok = match r.direction {
                Some(d) => d == dir,
                None => kind_allows(&r.category, kind, dir),
            };
            if ok && pattern_matches(text, &r.pat) {
                return Some((r.category.clone(), CatSource::Rule, r.rule_id));
            }
        }
        None
    }

    fn fallback(dir: Direction) -> (String, CatSource, Option<u64>) {
        let fb = if dir == Direction::Debit { FALLBACK_DEBIT } else { FALLBACK_CREDIT };
        (fb.to_string(), CatSource::Default, None)
    }

    /// Category from the narration alone (user rules, then built-in keywords).
    pub fn categorize(&self, narration: &str, dir: Direction) -> (String, CatSource, Option<u64>) {
        let text = normalize(narration);
        self.first_match(&self.rules, &text, dir).unwrap_or_else(|| Self::fallback(dir))
    }

    /// Full categorisation: user rules, transfer to the holder's own name, card bill payments,
    /// built-in keywords, then the fallback category (source `Default`).
    pub fn classify(&self, f: &Facts) -> (String, CatSource, Option<u64>) {
        let text = normalize(f.narration);
        if let Some(m) = self.first_match(&self.rules[..self.n_user], &text, f.direction) {
            return m;
        }
        if self.kinds.contains_key(OWN_TRANSFER)
            && SELF_CHANNELS.contains(&f.channel)
            && !f.holder.trim().is_empty()
            && crate::holder::is_self(f.merchant, f.holder)
        {
            return (OWN_TRANSFER.to_string(), CatSource::Auto, None);
        }
        if f.direction == Direction::Debit && self.kinds.contains_key("cc_payment") && CARD_BILL.is_match(f.narration) {
            return ("cc_payment".to_string(), CatSource::Rule, None);
        }
        self.first_match(&self.rules[self.n_user..], &text, f.direction).unwrap_or_else(|| Self::fallback(f.direction))
    }
}

/// Second-pass guesses for transactions no rule matched: the category most often given to the
/// same payee, else the small-UPI default (local vegetable and fruit vendors).
pub struct Guesser {
    history: HashMap<(Direction, String), String>,
    small_max: Paise,
    small_cat: Option<String>,
}

impl Guesser {
    pub fn new(v: &Vault) -> Self {
        let holders: HashMap<u64, &str> = v.accounts.iter().map(|a| (a.id, a.holder.as_str())).collect();
        let mut counts: HashMap<(Direction, String), HashMap<&str, usize>> = HashMap::new();
        for t in v.txns.iter().filter(|t| matches!(t.source, CatSource::Rule | CatSource::User)) {
            if matches!(t.category.as_str(), FALLBACK_DEBIT | FALLBACK_CREDIT | OWN_TRANSFER | REVERSAL) {
                continue;
            }
            let key = Facts::of(t, holders.get(&t.account_id).copied().unwrap_or("")).payee();
            if key.chars().filter(|c| c.is_ascii_alphanumeric()).count() < 3 {
                continue;
            }
            *counts.entry((t.direction, key)).or_default().entry(t.category.as_str()).or_default() += 1;
        }
        let history = counts
            .into_iter()
            .map(|(k, cats)| {
                let best = cats.into_iter().max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0))).unwrap().0.to_string();
                (k, best)
            })
            .collect();
        let s = &v.settings;
        let small_cat = Some(s.small_upi_category.clone()).filter(|c| v.category(c).is_some());
        Guesser { history, small_max: s.small_upi_max, small_cat }
    }

    pub fn guess(&self, f: &Facts) -> Option<String> {
        if let Some(c) = self.history.get(&(f.direction, f.payee())) {
            return Some(c.clone());
        }
        if f.direction == Direction::Debit && f.channel == "UPI" && self.small_max > 0 && f.amount <= self.small_max {
            return self.small_cat.clone();
        }
        None
    }
}

/// Re-run categorisation on every transaction that was not set by hand or paired as a transfer.
/// Returns how many transactions changed category.
pub fn recategorize_all(v: &mut Vault) -> usize {
    let c = Categorizer::new(&v.rules, &v.categories);
    let holders: HashMap<u64, String> = v.accounts.iter().map(|a| (a.id, a.holder.clone())).collect();
    let before: Vec<String> = v.txns.iter().map(|t| t.category.clone()).collect();
    for t in v.txns.iter_mut() {
        if matches!(t.source, CatSource::User | CatSource::Transfer) {
            continue;
        }
        let holder = holders.get(&t.account_id).map(String::as_str).unwrap_or("");
        let (cat, src, rid) = c.classify(&Facts::of(t, holder));
        t.category = cat;
        t.source = src;
        t.rule_id = rid;
    }
    let g = Guesser::new(v);
    for t in v.txns.iter_mut().filter(|t| t.source == CatSource::Default) {
        let holder = holders.get(&t.account_id).map(String::as_str).unwrap_or("");
        if let Some(cat) = g.guess(&Facts::of(t, holder)) {
            t.category = cat;
            t.source = CatSource::Auto;
        }
    }
    v.txns.iter().zip(before).filter(|(t, b)| &t.category != b).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    fn cat(narr: &str, dir: Direction) -> String {
        Categorizer::new(&[], &default_categories()).categorize(narr, dir).0
    }

    #[test]
    fn boundaries() {
        let t = normalize("UPI-OLA CABS/123");
        assert!(pattern_matches(&t, "OLA"));
        assert!(!pattern_matches(&normalize("COLABA CAUSEWAY"), "OLA"));
        assert!(!pattern_matches(&normalize("CURRENT ACCOUNT"), "RENT"));
        assert!(!pattern_matches(&normalize("RENTAL"), "RENT"));
        assert!(pattern_matches(&normalize("SWIGGYINSTAMART"), "SWIGGY"));
    }

    #[test]
    fn builtins() {
        assert_eq!(cat("UPI-SWIGGY-SWIGGY@AXISBANK-UTIB0000100-1234-UPI", Debit), "food");
        assert_eq!(cat("UPI/DR/123/SWIGGY INSTAMART/YESB/x@ybl", Debit), "groceries");
        assert_eq!(cat("POS 512345XXXXXX6789 HPCL PETROL PUMP", Debit), "fuel");
        assert_eq!(cat("ACH D- ZERODHA BROKING LTD-ABCD1234", Debit), "investments");
        assert_eq!(cat("NEFT CR-ICIC0000001-ACME LTD-SALARY OCT", Credit), "salary");
        assert_eq!(cat("SALARY TO MAID", Debit), "other");
        assert_eq!(cat("CREDIT INTEREST", Credit), "interest");
        assert_eq!(cat("ATW-512345XXXXXX6789-S1ANMU13-MUMBAI", Debit), "cash");
        assert_eq!(cat("UPI-CRED CLUB-CRED.CLUB@AXISB-UTIB0000114-1-CREDIT CARD BILL", Debit), "cc_payment");
        assert_eq!(cat("NETFLIX.COM", Debit), "entertainment");
        assert_eq!(cat("AMAZON PRIME MEMBERSHIP", Debit), "entertainment");
        assert_eq!(cat("AMAZON PAY INDIA", Debit), "shopping");
        assert_eq!(cat("REFUND FROM AMAZON", Credit), "refunds");
        assert_eq!(cat("UPI-ZOMATO-ZOMATO@HDFC-1-UPI", Credit), "food");
        assert_eq!(cat("UPI-RAHUL KUMAR-RAHUL@OKHDFC-1-UPI", Credit), "other_income");
        assert_eq!(cat("IMPS-1234-MERCHANT", Debit), "other");
    }

    #[test]
    fn user_rules_win() {
        let rules = vec![
            Rule { id: 5, pattern: "rahul@okhdfc".into(), category: "rent".into(), direction: Some(Debit), created: String::new() },
            Rule { id: 9, pattern: "SWIGGY".into(), category: "entertainment".into(), direction: None, created: String::new() },
        ];
        let c = Categorizer::new(&rules, &default_categories());
        let (cat, src, rid) = c.categorize("UPI-RAHUL KUMAR-RAHUL@OKHDFC-1-UPI", Debit);
        assert_eq!((cat.as_str(), src, rid), ("rent", CatSource::Rule, Some(5)));
        assert_eq!(c.categorize("UPI-RAHUL KUMAR-RAHUL@OKHDFC-1-UPI", Credit).0, "other_income");
        assert_eq!(c.categorize("UPI-SWIGGY-X", Debit).0, "entertainment");
    }

    fn txn(id: u64, narr: &str, amount: Paise, dir: Direction) -> Txn {
        let info = crate::narration::analyze(narr, "");
        Txn {
            id,
            account_id: 1,
            date: chrono::NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
            value_date: None,
            narration: narr.into(),
            reference: String::new(),
            amount,
            direction: dir,
            balance: None,
            merchant: info.merchant,
            vpa: info.vpa,
            channel: info.channel.to_string(),
            category: String::new(),
            source: CatSource::Default,
            rule_id: None,
            key: id.to_string(),
            import_id: 1,
            note: String::new(),
        }
    }

    fn vault(txns: Vec<Txn>) -> Vault {
        let mut v = Vault::new();
        v.accounts.push(crate::model::Account {
            id: 1,
            bank: "SBI".into(),
            number: "1234".into(),
            holder: "HARSH YOGESH SHARMA".into(),
            holder_entered: false,
            pan: None,
            nickname: String::new(),
            created: String::new(),
        });
        v.txns = txns;
        v
    }

    #[test]
    fn card_bill_and_self_name() {
        let c = Categorizer::new(&[], &default_categories());
        let t = txn(1, "IB BILLPAY DR-HDFC92-434155XXXXXX6660", 5_000_00, Debit);
        assert_eq!(c.classify(&Facts::of(&t, "HARSH YOGESH SHARMA")).0, "cc_payment");
        let t = txn(2, "UPI-HARSH YOGESH SHARMA-HARSH@OKSBI-SBIN0001-1234-UPI", 20_000_00, Debit);
        let r = c.classify(&Facts::of(&t, "HARSH YOGESH SHARMA"));
        assert_eq!((r.0.as_str(), r.1), (OWN_TRANSFER, CatSource::Auto));
        let t = txn(3, "UPI-YOGESH SHARMA-YS@OKSBI-SBIN0001-1234-UPI", 20_000_00, Debit);
        assert_eq!(c.classify(&Facts::of(&t, "HARSH YOGESH SHARMA")).0, "other");
    }

    #[test]
    fn small_upi_and_history() {
        let mut v = vault(vec![
            txn(1, "UPI/DR/401234567890/RAMU VEG/YESB/paytmqr281/Payment", 120_00, Debit),
            txn(2, "UPI/DR/401234567891/SURESH K/SBIN/suresh1234/Payment", 2_500_00, Debit),
            txn(3, "A2AINT01-TELEDYNE INDIA-SALARY", 90_000_00, Credit),
            txn(4, "FT- HARSH154-50100012345678 - TELEDYNE INDIA", 90_000_00, Credit),
            txn(5, "UPI/DR/401234567892/RAMU VEG/YESB/paytmqr281/Payment", 350_00, Debit),
            txn(6, "UPI/DR/401234567893/SHYAM/YESB/paytmqr999/Payment", 300_00, Debit),
        ]);
        recategorize_all(&mut v);
        let got: Vec<(&str, CatSource)> = v.txns.iter().map(|t| (t.category.as_str(), t.source)).collect();
        assert_eq!(got[0], ("groceries", CatSource::Auto));
        assert_eq!(got[1], ("other", CatSource::Default));
        assert_eq!(got[2], ("salary", CatSource::Rule));
        assert_eq!(got[3], ("salary", CatSource::Auto));
        // Auto guesses do not feed the payee history, so a bigger payment to the same vendor is left for review.
        assert_eq!(got[4], ("other", CatSource::Default));
        assert_eq!(got[5], ("groceries", CatSource::Auto));

        // A category set by hand is reused for the same payee.
        v.txns[1].category = "family".into();
        v.txns[1].source = CatSource::User;
        v.txns.push(txn(7, "UPI/DR/401234567894/SURESH K/SBIN/suresh1234/Payment", 3_000_00, Debit));
        recategorize_all(&mut v);
        assert_eq!((v.txns[6].category.as_str(), v.txns[6].source), ("family", CatSource::Auto));

        v.settings.small_upi_max = 0;
        recategorize_all(&mut v);
        assert_eq!((v.txns[5].category.as_str(), v.txns[5].source), ("other", CatSource::Default));
    }
}
