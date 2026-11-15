//! Account-holder matching. Accounts may only be combined in a report when they belong to the same
//! person: same PAN when both statements show one, otherwise the same name (ignoring titles, order
//! and middle initials).

use crate::model::Account;

const TITLES: &[&str] = &["MR", "MRS", "MS", "MISS", "DR", "SHRI", "SMT", "KUMARI", "KU", "M S", "MASTER", "PROF", "SRI", "SMT."];

pub fn name_tokens(name: &str) -> Vec<String> {
    let cleaned: String = name
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphabetic() { c } else { ' ' })
        .collect();
    let mut toks: Vec<String> = cleaned.split_whitespace().map(|s| s.to_string()).collect();
    while toks.first().map(|t| TITLES.contains(&t.as_str())).unwrap_or(false) {
        toks.remove(0);
    }
    toks
}

/// Display form: title removed, words capitalised.
pub fn clean_name(name: &str) -> String {
    name_tokens(name)
        .iter()
        .map(|t| {
            let mut c = t.chars();
            match c.next() {
                Some(f) => f.to_string() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn names_match(a: &str, b: &str) -> bool {
    let mut x = name_tokens(a);
    let mut y = name_tokens(b);
    if x.is_empty() || y.is_empty() {
        return false;
    }
    x.sort();
    y.sort();
    if x == y {
        return true;
    }
    let (small, big) = if x.len() <= y.len() { (&x, &y) } else { (&y, &x) };
    let mut used = vec![false; big.len()];
    let mut exact = 0;
    // Exact matches first, then initials.
    let mut pending = Vec::new();
    for s in small.iter() {
        if let Some(i) = (0..big.len()).find(|&i| !used[i] && &big[i] == s) {
            used[i] = true;
            exact += 1;
        } else {
            pending.push(s);
        }
    }
    for s in pending {
        let found = (0..big.len()).find(|&i| {
            !used[i] && ((s.len() == 1 && big[i].starts_with(s.as_str())) || (big[i].len() == 1 && s.starts_with(big[i].as_str())))
        });
        match found {
            Some(i) => used[i] = true,
            None => return false,
        }
    }
    exact >= 2
}

/// Words that appear next to a name in transfer narrations ("HARSH SHARMA SBI") but are not part of it.
const NOT_NAME: &[&str] = &[
    "SBI", "HDFC", "ICICI", "AXIS", "KOTAK", "BANK", "YES", "IDFC", "FIRST", "PNB", "BOB", "CANARA", "UNION", "INDUSIND",
    "FEDERAL", "SAVINGS", "SAVING", "SB", "AC", "ACC", "ACCOUNT", "SELF", "OWN", "LTD",
];

/// True when a transfer's counterparty is the account holder themself. Every word of the holder's
/// name must be present (initials and cut-off words count) and at least two must match exactly.
/// A shorter form such as "HARSH SHARMA" is not accepted for "HARSH YOGESH SHARMA", because banks
/// print names in different orders and "YOGESH SHARMA" (the father) could not be told apart.
pub fn is_self(counterparty: &str, holder: &str) -> bool {
    let h = name_tokens(holder);
    let c: Vec<String> = name_tokens(counterparty).into_iter().filter(|t| !NOT_NAME.contains(&t.as_str())).collect();
    if h.len() < 2 || c.len() != h.len() {
        return false;
    }
    let mut used = vec![false; h.len()];
    let mut exact = 0;
    let mut pending = Vec::new();
    for t in &c {
        match (0..h.len()).find(|&i| !used[i] && &h[i] == t) {
            Some(i) => {
                used[i] = true;
                exact += 1;
            }
            None => pending.push(t),
        }
    }
    for t in pending {
        match (0..h.len()).find(|&i| !used[i] && h[i].starts_with(t.as_str())) {
            Some(i) => used[i] = true,
            None => return false,
        }
    }
    exact >= 2
}

pub fn normalize_pan(p: &str) -> Option<String> {
    let p: String = p.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
    let b = p.as_bytes();
    let ok = b.len() == 10
        && b[..5].iter().all(|c| c.is_ascii_uppercase())
        && b[5..9].iter().all(|c| c.is_ascii_digit())
        && b[9].is_ascii_uppercase();
    if ok { Some(p) } else { None }
}

pub fn same_person(a: &Account, b: &Account) -> bool {
    if a.id == b.id {
        return true;
    }
    if let (Some(pa), Some(pb)) = (&a.pan, &b.pan) {
        return pa == pb;
    }
    names_match(&a.holder, &b.holder)
}

/// Groups of account ids that may be combined. Every pair inside a group matches.
pub fn groups(accounts: &[Account]) -> Vec<Vec<u64>> {
    let mut out: Vec<Vec<&Account>> = Vec::new();
    for a in accounts {
        match out.iter_mut().find(|g| g.iter().all(|m| same_person(m, a))) {
            Some(g) => g.push(a),
            None => out.push(vec![a]),
        }
    }
    out.into_iter().map(|g| g.into_iter().map(|a| a.id).collect()).collect()
}

/// Checks that the requested accounts may be combined into one report.
pub fn validate_scope(accounts: &[Account], ids: &[u64]) -> Result<(), String> {
    let sel: Vec<&Account> = ids
        .iter()
        .map(|id| accounts.iter().find(|a| a.id == *id).ok_or_else(|| format!("Unknown account {id}")))
        .collect::<Result<_, _>>()?;
    for i in 0..sel.len() {
        for j in i + 1..sel.len() {
            if !same_person(sel[i], sel[j]) {
                return Err(format!(
                    "These accounts belong to different holders ({} / {}) and cannot be combined.",
                    clean_name(&sel[i].holder),
                    clean_name(&sel[j].holder)
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: u64, holder: &str, pan: Option<&str>) -> Account {
        Account {
            id,
            bank: "SBI".into(),
            number: String::new(),
            holder: holder.into(),
            holder_entered: false,
            pan: pan.map(|s| s.into()),
            nickname: String::new(),
            created: String::new(),
        }
    }

    #[test]
    fn names() {
        assert!(names_match("MR HARSH SHARMA", "Mr. Harsh Sharma"));
        assert!(names_match("SHARMA HARSH", "harsh sharma"));
        assert!(names_match("HARSH K SHARMA", "MR. HARSH SHARMA"));
        assert!(names_match("HARSH KUMAR SHARMA", "HARSH K SHARMA"));
        assert!(!names_match("H SHARMA", "HARSH SHARMA"));
        assert!(!names_match("HARSH SHARMA", "PRIYA SHARMA"));
        assert!(!names_match("", "HARSH SHARMA"));
        assert_eq!(clean_name("MR HARSH  SHARMA"), "Harsh Sharma");
    }

    #[test]
    fn self_name() {
        let h = "Harsh Yogesh Sharma";
        assert!(is_self("HARSH YOGESH SHARMA", h));
        assert!(is_self("SHARMA HARSH YOGESH", h));
        assert!(is_self("Master HARSH YOGESH SH", h));
        assert!(is_self("HARSH Y SHARMA SBI", h));
        assert!(!is_self("Yogesh S", h));
        assert!(!is_self("YOGESH SHARMA", h));
        assert!(!is_self("MANJU YOGESH SHARMA", h));
        assert!(!is_self("MRS. MANJU YOGESH SHARMA", h));
        assert!(!is_self("SHARMA H", h));
        assert!(is_self("HARSH SHARMA HDFC", "Harsh Sharma"));
        assert!(!is_self("PRIYA SHARMA", "Harsh Sharma"));
    }

    #[test]
    fn pan_rules() {
        assert_eq!(normalize_pan("abcde1234f").as_deref(), Some("ABCDE1234F"));
        assert_eq!(normalize_pan("ABCDE12345"), None);
        // PAN overrides name when both present.
        assert!(!same_person(&acc(1, "HARSH SHARMA", Some("ABCDE1234F")), &acc(2, "HARSH SHARMA", Some("ZZZZZ9999Z"))));
        assert!(same_person(&acc(1, "H SHARMA", Some("ABCDE1234F")), &acc(2, "HARSH SHARMA", Some("ABCDE1234F"))));
    }

    #[test]
    fn grouping_and_scope() {
        let a = vec![
            acc(1, "MR HARSH SHARMA", None),
            acc(2, "PRIYA SHARMA", None),
            acc(3, "Harsh Sharma", Some("ABCDE1234F")),
        ];
        assert_eq!(groups(&a), vec![vec![1, 3], vec![2]]);
        assert!(validate_scope(&a, &[1, 3]).is_ok());
        assert!(validate_scope(&a, &[1, 2]).is_err());
        assert!(validate_scope(&a, &[9]).is_err());
    }
}
