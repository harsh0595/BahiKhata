//! Extracts the payment channel, counterparty ("merchant") and UPI VPA from a narration.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarrInfo {
    pub channel: &'static str,
    pub merchant: String,
    pub vpa: Option<String>,
}

static IFSC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]{4}0[A-Z0-9]{6}$").unwrap());
static MASKED_CARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9X*]{8,}$").unwrap());
/// SBI appends "<10+ digit code> [OF <name>] AT <branch code> <branch name>" to many narrations.
static SBI_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s\d{10,}\s+(?:OF\s+(.+?)\s+)?AT\s+\d{4,6}\b.*$").unwrap());
/// "631413199590ZOMATO": a reference number glued to the merchant name.
static GLUED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d{6,})([A-Za-z]{3,})").unwrap());
/// SBI UPI: UPI/DR/<rrn>/<name, cut to 8 chars>/<bank>/<vpa without @, cut>/<remark>
static SBI_UPI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"UPI/(?:DR|CR|REV|REF|DRC)/\d+/([^/]*)/([A-Z0-9]{4})/([^/]*)").unwrap());

/// Clean-up applied to every narration at import: removes SBI's branch suffix (keeping the
/// "OF <name>" counterparty) and separates reference numbers glued to names.
pub fn tidy(narration: &str) -> String {
    let s = narration.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = match SBI_TAIL.captures(&s) {
        Some(c) => {
            let head = s[..c.get(0).unwrap().start()].trim_end();
            match c.get(1) {
                Some(name) => format!("{head} - {}", name.as_str().trim()),
                None => head.to_string(),
            }
        }
        None => s,
    };
    GLUED.replace_all(&s, "$1 $2").into_owned()
}

const STOP: &[&str] = &[
    "UPI", "DR", "CR", "NEFT", "IMPS", "RTGS", "NEFT CR", "NEFT DR", "IMPS CR", "IMPS DR", "RTGS CR", "RTGS DR",
    "TO TRANSFER", "BY TRANSFER", "TRANSFER", "TRANSFER TO", "TRANSFER FROM", "TRF", "TFR", "DEP TFR", "WDL TFR",
    "P2M", "P2A", "P2P", "ACH", "ACH D", "ACH C", "NACH", "NACH DR", "ECS", "POS", "PAYMENT", "PAY", "MOB", "INB",
    "IB", "MB", "BIL", "ONL", "ECOM", "PUR", "SENT", "RECEIVED", "COLLECT", "REQUEST", "NA", "NULL", "UPI PAYMENT",
    "PAYMENT FROM PHONE", "PAY TO", "UPIINTENT", "INTENT", "OTHERS", "OTHER", "SBIN", "HDFC", "ICIC", "UTIB", "YESB",
    "PUNB", "KKBK", "BARB", "IDIB", "CNRB", "UBIN", "BKID", "IOBA", "INDB", "FDRL", "IDFB", "AIRP", "PYTM", "ICICI",
    "AXIS", "SBI", "HDFC BANK", "YES BANK", "KOTAK", "AXIS BANK", "STATE BANK OF INDIA", "ICICI BANK", "PAYTM",
    "BHIM", "MOBILE BANKING", "NET BANKING", "NETBANKING", "NETBANK", "INET", "MMT", "IMPS P2A", "FROM", "TO", "BY",
    "DEP", "WDL", "ATM", "PURCH", "OTHPG", "CEMTEX", "ACHCR", "ACHDR", "DIRECT", "BILLPAY", "FT", "REV", "REF", "DRC",
];

fn is_stop(t: &str) -> bool {
    STOP.contains(&t)
}

fn letters(t: &str) -> usize {
    t.chars().filter(|c| c.is_ascii_alphabetic()).count()
}

fn digits(t: &str) -> usize {
    t.chars().filter(|c| c.is_ascii_digit()).count()
}

fn name_like(t: &str) -> bool {
    let l = letters(t);
    let d = digits(t);
    l >= 3
        && d * 10 < (l + d) * 4
        // A single word with digits is a code ("A2AINT01", "HARSH154"), not a name.
        && (d == 0 || t.contains(' '))
        && !t.contains('@')
        && !IFSC.is_match(t)
        && !MASKED_CARD.is_match(t)
        && !is_stop(t)
        && t.len() <= 60
}

pub fn detect_channel(upper: &str) -> &'static str {
    let padded = format!(" {} ", upper.replace(['-', '/', '*', ':', '.', ','], " "));
    let has = |w: &str| padded.contains(&format!(" {w} "));
    if has("UPI") {
        "UPI"
    } else if has("NEFT") {
        "NEFT"
    } else if has("RTGS") {
        "RTGS"
    } else if has("IMPS") || has("MMT") {
        "IMPS"
    } else if has("POS") || has("PCD") {
        // "POS ATM PURCH ..." is a card purchase, so POS is checked before ATM.
        "POS"
    } else if has("ATW") || has("NWD") || has("EAW") || has("ATM") || upper.contains("CASH WDL") || upper.contains("CASH WITHDRAWAL") {
        "ATM"
    } else if has("ACH") || has("ACHCR") || has("ACHDR") || has("NACH") || has("ECS") || has("SI") {
        "AUTO-DEBIT"
    } else if upper.contains("INT.PD") || upper.contains("INT PD") || upper.contains("INTEREST") {
        "INTEREST"
    } else if upper.contains("CHRG") || upper.contains("CHARGES") || upper.contains("CHGS") || upper.contains("SMS ALERT") {
        "CHARGES"
    } else if has("CHQ") || upper.contains("CHEQUE") || has("CLG") || upper.contains("CLEARING") {
        "CHEQUE"
    } else {
        "OTHER"
    }
}

/// Remove company suffixes so "ACME LTD" and "ACME LIMITED" group together.
pub fn clean_merchant(m: &str) -> String {
    let mut s = m.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase();
    for suffix in [" PRIVATE LIMITED", " PVT LTD", " PVT. LTD.", " PVT. LTD", " LIMITED", " LTD.", " LTD", " PVT", " LLP"] {
        if s.ends_with(suffix) && s.len() > suffix.len() + 2 {
            s.truncate(s.len() - suffix.len());
        }
    }
    s.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != ')').to_string()
}

/// Text that identifies who a payment went to or came from; usable as a rule pattern because it
/// appears in the narration. SBI cuts both the name and the UPI ID, so neither is unique alone:
/// "SURESH K/SBIN/paytmqr281" keeps them together.
pub fn payee_pattern(narration: &str, merchant: &str, vpa: Option<&str>) -> String {
    let upper = narration.to_uppercase();
    if let Some(c) = SBI_UPI.captures(&upper) {
        let (s, e) = (c.get(1).unwrap().start(), c.get(3).unwrap().end());
        let p = upper[s..e].trim_matches(|ch: char| ch == '/' || ch.is_whitespace());
        if letters(p) >= 3 {
            return p.to_string();
        }
    }
    if let Some(v) = vpa.filter(|v| v.contains('@')) {
        return v.to_uppercase();
    }
    if letters(merchant) >= 3 {
        return merchant.to_string();
    }
    String::new()
}

pub fn analyze(narration: &str, reference: &str) -> NarrInfo {
    let upper = narration.to_uppercase();
    let upper = upper.trim();
    let mut channel = detect_channel(upper);
    if channel == "OTHER" && !reference.is_empty() {
        let rc = detect_channel(&reference.to_uppercase());
        if rc != "OTHER" {
            channel = rc;
        }
    }

    let tokens: Vec<String> = upper
        .split(['-', '/', '*', ':', '|', '\\'])
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty())
        .collect();

    let mut vpa = tokens
        .iter()
        .find(|t| t.contains('@') && !t.contains(' '))
        .map(|t| t.to_lowercase());

    // SBI cuts the payee name to 8 characters and drops the "@bank" part of the UPI ID; keep what
    // is there so payments to the same shop group together.
    let mut sbi_name: Option<String> = None;
    if let Some(c) = SBI_UPI.captures(upper) {
        let name = c[1].split_whitespace().collect::<Vec<_>>().join(" ");
        let id = c[3].trim();
        if vpa.is_none() && id.len() >= 3 {
            vpa = Some(id.to_lowercase());
        }
        if name_like(&name) && !name.split(' ').all(is_stop) {
            sbi_name = Some(name);
        }
    }

    let merchant = if channel == "ATM" {
        "ATM CASH".to_string()
    } else if channel == "INTEREST" {
        "INTEREST".to_string()
    } else if let Some(n) = sbi_name {
        n
    } else {
        let mut found: Option<String> = None;
        for t in &tokens {
            // "POS 512345XXXXXX6789 CORNER GROCER": drop leading words that are channel codes or card numbers.
            let words: Vec<&str> = t.split(' ').collect();
            let mut start = 0;
            while start < words.len() && (is_stop(words[start]) || MASKED_CARD.is_match(words[start]) || digits(words[start]) == words[start].len()) {
                start += 1;
            }
            let candidate = words[start..].join(" ");
            if !candidate.is_empty() && name_like(&candidate) && !candidate.split(' ').all(is_stop) {
                found = Some(candidate);
                break;
            }
        }
        match (found, &vpa) {
            (Some(m), _) => m,
            (None, Some(v)) => {
                let handle = v.split('@').next().unwrap_or("").to_uppercase();
                if letters(&handle) >= 3 { handle } else { String::new() }
            }
            (None, None) => String::new(),
        }
    };
    let mut merchant = clean_merchant(&merchant);
    if merchant.len() > 40 {
        merchant = merchant.chars().take(40).collect::<String>().trim().to_string();
    }
    NarrInfo { channel, merchant, vpa }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdfc_upi() {
        let i = analyze("UPI-SWIGGY-SWIGGY@AXISBANK-UTIB0000100-628812345678-UPI", "0000628812345678");
        assert_eq!(i.channel, "UPI");
        assert_eq!(i.merchant, "SWIGGY");
        assert_eq!(i.vpa.as_deref(), Some("swiggy@axisbank"));
    }

    #[test]
    fn sbi_upi() {
        let i = analyze("TO TRANSFER-UPI/DR/627412345678/SWIGGY/ICIC/swiggy@icici/Payment", "");
        assert_eq!(i.channel, "UPI");
        assert_eq!(i.merchant, "SWIGGY");
        assert_eq!(i.vpa.as_deref(), Some("swiggy@icici"));
        let i = analyze("DEP TFR UPI/CR/512345678901/RAHUL KUMAR/HDFC/rahul@okhdfc/Lunch", "");
        assert_eq!(i.merchant, "RAHUL KUMAR");
    }

    #[test]
    fn neft_pos_atm_ach() {
        assert_eq!(analyze("NEFT CR-ICIC0000001-ACME LTD-SALARY OCT", "").merchant, "ACME");
        let i = analyze("NEFT DR-BDBL0001468-SPOUSE NAME-NETBANK, MUM-HDFCH01098537395-SALARY JUNE", "");
        assert_eq!(i.channel, "NEFT");
        assert_eq!(i.merchant, "SPOUSE NAME");
        let i = analyze("POS 512345XXXXXX6789 CORNER GROCER", "");
        assert_eq!(i.channel, "POS");
        assert_eq!(i.merchant, "CORNER GROCER");
        let i = analyze("ATW-512345XXXXXX6789-S1ANMU13-MUMBAI", "");
        assert_eq!(i.channel, "ATM");
        assert_eq!(i.merchant, "ATM CASH");
        let i = analyze("ACH D- ZERODHA BROKING LTD-ABCD1234", "");
        assert_eq!(i.channel, "AUTO-DEBIT");
        assert_eq!(i.merchant, "ZERODHA BROKING");
        let i = analyze("BY TRANSFER-NEFT*ACME LTD*SALARY", "");
        assert_eq!(i.merchant, "ACME");
    }

    #[test]
    fn plain_text_and_reference_channel() {
        let i = analyze("Apollo Pharmacy Medicines", "UPI/SBI/781029");
        assert_eq!(i.channel, "UPI");
        assert_eq!(i.merchant, "APOLLO PHARMACY MEDICINES");
        let i = analyze("INT.PD:01-01-2026 TO 31-03-2026", "");
        assert_eq!(i.channel, "INTEREST");
    }

    #[test]
    fn sbi_real_format() {
        let n = tidy("WDL TFR UPI/DR/645960500001/GOLDEN T/YESB/q213100001/UPI 0097600000001 AT 10414 SOMEPLACE - WEST");
        assert_eq!(n, "WDL TFR UPI/DR/645960500001/GOLDEN T/YESB/q213100001/UPI");
        let i = analyze(&n, "");
        assert_eq!((i.channel, i.merchant.as_str(), i.vpa.as_deref()), ("UPI", "GOLDEN T", Some("q213100001")));

        let n = tidy("DIRECT DR 0038100000001 OF Master TEST KUMAR US AT 10414 SOMEPLACE");
        assert_eq!(n, "DIRECT DR - Master TEST KUMAR US");
        assert_eq!(analyze(&n, "").merchant, "MASTER TEST KUMAR US");

        let n = tidy("POS ATM PURCH OTHPG 631413100001ZOMATO LIMITED NEW DELHI");
        assert_eq!(n, "POS ATM PURCH OTHPG 631413100001 ZOMATO LIMITED NEW DELHI");
        let i = analyze(&n, "");
        assert_eq!((i.channel, i.merchant.as_str()), ("POS", "ZOMATO LIMITED NEW DELHI"));

        let i = analyze("ATM WDL ATM CASH 1234 SOMEPLACE", "");
        assert_eq!((i.channel, i.merchant.as_str()), ("ATM", "ATM CASH"));

        // Codes such as "A2AINT01" or "USER154" are not names.
        assert_eq!(analyze("A2AINT01-ACME TRADING PL SEZ-SALARY-USER123", "").merchant, "ACME TRADING PL SEZ");
        assert_eq!(analyze("FT- USER154-50200000000001 - ACME TRADING PL SEZ -", "").merchant, "ACME TRADING PL SEZ");
    }
}
