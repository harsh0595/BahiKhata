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

const STOP: &[&str] = &[
    "UPI", "DR", "CR", "NEFT", "IMPS", "RTGS", "NEFT CR", "NEFT DR", "IMPS CR", "IMPS DR", "RTGS CR", "RTGS DR",
    "TO TRANSFER", "BY TRANSFER", "TRANSFER", "TRANSFER TO", "TRANSFER FROM", "TRF", "TFR", "DEP TFR", "WDL TFR",
    "P2M", "P2A", "P2P", "ACH", "ACH D", "ACH C", "NACH", "NACH DR", "ECS", "POS", "PAYMENT", "PAY", "MOB", "INB",
    "IB", "MB", "BIL", "ONL", "ECOM", "PUR", "SENT", "RECEIVED", "COLLECT", "REQUEST", "NA", "NULL", "UPI PAYMENT",
    "PAYMENT FROM PHONE", "PAY TO", "UPIINTENT", "INTENT", "OTHERS", "OTHER", "SBIN", "HDFC", "ICIC", "UTIB", "YESB",
    "PUNB", "KKBK", "BARB", "IDIB", "CNRB", "UBIN", "BKID", "IOBA", "INDB", "FDRL", "IDFB", "AIRP", "PYTM", "ICICI",
    "AXIS", "SBI", "HDFC BANK", "YES BANK", "KOTAK", "AXIS BANK", "STATE BANK OF INDIA", "ICICI BANK", "PAYTM",
    "BHIM", "MOBILE BANKING", "NET BANKING", "NETBANKING", "NETBANK", "INET", "MMT", "IMPS P2A", "FROM", "TO", "BY",
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
    } else if has("ATW") || has("NWD") || has("EAW") || has("ATM") || upper.contains("CASH WDL") || upper.contains("CASH WITHDRAWAL") {
        "ATM"
    } else if has("POS") || has("PCD") {
        "POS"
    } else if has("ACH") || has("NACH") || has("ECS") || has("SI") {
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

    let vpa = tokens
        .iter()
        .find(|t| t.contains('@') && !t.contains(' '))
        .map(|t| t.to_lowercase());

    let merchant = if channel == "ATM" {
        "ATM CASH".to_string()
    } else if channel == "INTEREST" {
        "INTEREST".to_string()
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
            if !candidate.is_empty() && name_like(&candidate) {
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
}
