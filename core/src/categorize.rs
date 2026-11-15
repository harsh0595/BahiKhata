//! Default categories, built-in keyword rules and the rule engine.
//!
//! Matching works on normalised text (upper case, anything that is not a letter or digit becomes a
//! space). A pattern must start at a word boundary; short patterns (4 characters or fewer) must also
//! end at one, so "OLA" does not match "COLABA" and "RENT" does not match "CURRENT".
//! User rules are tried first (newest first), then built-in rules, longest pattern first.

use crate::model::{CatKind, CatSource, Category, Direction, Rule, Vault};
use std::collections::HashMap;

pub const FALLBACK_DEBIT: &str = "other";
pub const FALLBACK_CREDIT: &str = "other_income";
pub const OWN_TRANSFER: &str = "own_transfer";

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
        ("other", "Other Expenses", Expense, "#90a4ae"),
        ("investments", "Investments", Investment, "#3949ab"),
        ("salary", "Salary", Income, "#1b5e20"),
        ("interest", "Interest", Income, "#558b2f"),
        ("dividends", "Dividends", Income, "#7cb342"),
        ("refunds", "Refunds & Cashback", Income, "#26a69a"),
        ("other_income", "Other Income", Income, "#80cbc4"),
        ("own_transfer", "Own Account Transfer", Excluded, "#b0bec5"),
        ("cc_payment", "Credit Card Payment", Excluded, "#cfd8dc"),
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
    ("refunds", &["REFUND", "REFUNDS", "REVERSAL", "REV", "CASHBACK", "CASH BACK", "CHARGEBACK", "REVERSED", "RETURNED"]),
    ("groceries", &[
        "BLINKIT", "GROFERS", "ZEPTO", "BIGBASKET", "BIG BASKET", "INNOVATIVE RETAIL", "DMART", "D MART", "AVENUE SUPERMARTS",
        "JIOMART", "JIO MART", "RELIANCE FRESH", "RELIANCE SMART", "RELIANCE RETAIL", "MORE RETAIL", "MORE SUPERMARKET", "SPENCER",
        "NATURES BASKET", "STAR BAZAAR", "INSTAMART", "SWIGGY INSTAMART", "DUNZO", "KIRANA", "SUPERMARKET", "SUPER MARKET",
        "GROCERY", "GROCERIES", "GROCER", "PROVISION", "MILKBASKET", "COUNTRY DELIGHT", "LICIOUS", "FRESHTOHOME", "VEGETABLE", "DAIRY",
    ]),
    ("food", &[
        "SWIGGY", "BUNDL TECHNOLOGIES", "ZOMATO", "EATSURE", "DOMINOS", "DOMINO S", "JUBILANT FOODWORKS", "MCDONALD", "KFC",
        "PIZZA HUT", "BURGER KING", "STARBUCKS", "CAFE COFFEE DAY", "CCD", "SUBWAY", "HALDIRAM", "RESTAURANT", "RESTO", "CAFE",
        "BAKERY", "BAKERS", "CHAAYOS", "BARBEQUE NATION", "SWEETS", "DHABA", "HOTEL AND RESTAURANT", "FOODS", "BIRYANI",
    ]),
    ("fuel", &[
        "HPCL", "BPCL", "IOCL", "INDIAN OIL", "HINDUSTAN PETROLEUM", "BHARAT PETROLEUM", "PETROL", "PETROLEUM", "FUEL", "FUELS",
        "SHELL", "NAYARA", "FILLING STATION", "SERVICE STATION", "HP PAY", "IOCL FUEL", "PETRO", "AUTO FUEL",
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
        "APPLE MEDIA", "GOOGLE PLAY", "CINEMA", "MULTIPLEX",
    ]),
    ("healthcare", &[
        "APOLLO", "PHARMEASY", "NETMEDS", "TATA 1MG", "1MG", "MEDPLUS", "PRACTO", "HOSPITAL", "CLINIC", "PHARMACY", "PHARMA",
        "MEDICAL", "MEDICALS", "MEDICOS", "CHEMIST", "DIAGNOSTIC", "DIAGNOSTICS", "PATHOLOGY", "LAL PATH", "THYROCARE", "DENTAL",
        "HEALTHIANS", "METROPOLIS", "DOCTOR", "DR LAL", "NURSING HOME", "WELLNESS FOREVER", "MEDICINE", "MEDICINES",
    ]),
    ("rent", &["RENT", "HOUSE RENT", "NOBROKER", "HOUSING COM", "NESTAWAY", "SOCIETY MAINTENANCE", "MAINTENANCE", "CRED RENT"]),
    ("utilities", &[
        "ELECTRICITY", "MSEDCL", "MAHAVITARAN", "ADANI ELECTRICITY", "TATA POWER", "BESCOM", "TORRENT POWER", "BSES", "TPDDL",
        "AIRTEL", "BHARTI AIRTEL", "JIO", "RELIANCE JIO", "VODAFONE", "VODAFONE IDEA", "BSNL", "MTNL", "ACT FIBERNET", "HATHWAY",
        "TATA PLAY", "TATASKY", "TATA SKY", "DISH TV", "MAHANAGAR GAS", "MGL", "IGL", "INDRAPRASTHA GAS", "GAS", "INDANE",
        "HP GAS", "BHARAT GAS", "BHARATGAS", "WATER BILL", "WATER TAX", "PROPERTY TAX", "BROADBAND", "RECHARGE", "POSTPAID",
        "PREPAID", "DTH", "BILL PAYMENT", "BILLPAY", "ELECTRIC",
    ]),
    ("education", &[
        "SCHOOL", "COLLEGE", "UNIVERSITY", "BYJU", "BYJUS", "UNACADEMY", "COURSERA", "UDEMY", "UPGRAD", "VEDANTU", "TUITION",
        "EDUCATION", "ACADEMY", "INSTITUTE", "CLASSES", "EXAM FEE",
    ]),
    ("insurance", &[
        "LIC", "LIFE INSURANCE", "LIC OF INDIA", "INSURANCE", "HDFC LIFE", "ICICI PRU", "ICICI LOMBARD", "SBI LIFE", "SBI GENERAL",
        "STAR HEALTH", "POLICYBAZAAR", "POLICY BAZAAR", "ACKO", "GO DIGIT", "DIGIT INSURANCE", "BAJAJ ALLIANZ", "TATA AIG",
        "MAX LIFE", "NIVA BUPA", "CARE HEALTH", "PREMIUM",
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
    ("dividends", &["DIVIDEND", "DIV", "ACH C", "NACH CR", "INTERIM DIVIDEND", "FINAL DIVIDEND"]),
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
    kinds: HashMap<String, CatKind>,
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
        Categorizer { rules, kinds }
    }

    /// Returns (category, source, rule id).
    pub fn categorize(&self, narration: &str, dir: Direction) -> (String, CatSource, Option<u64>) {
        let text = normalize(narration);
        for r in &self.rules {
            let kind = self.kinds.get(&r.category).copied().unwrap_or(CatKind::Expense);
            let ok = match r.direction {
                Some(d) => d == dir,
                None => kind_allows(&r.category, kind, dir),
            };
            if ok && pattern_matches(&text, &r.pat) {
                return (r.category.clone(), CatSource::Rule, r.rule_id);
            }
        }
        let fb = if dir == Direction::Debit { FALLBACK_DEBIT } else { FALLBACK_CREDIT };
        (fb.to_string(), CatSource::Default, None)
    }
}

/// Re-run categorisation on every transaction that was not set by hand or detected as a transfer.
/// Returns how many transactions changed category.
pub fn recategorize_all(v: &mut Vault) -> usize {
    let c = Categorizer::new(&v.rules, &v.categories);
    let mut changed = 0;
    for t in v.txns.iter_mut() {
        if matches!(t.source, CatSource::User | CatSource::Transfer) {
            continue;
        }
        let (cat, src, rid) = c.categorize(&t.narration, t.direction);
        if cat != t.category {
            changed += 1;
        }
        t.category = cat;
        t.source = src;
        t.rule_id = rid;
    }
    changed
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
}
