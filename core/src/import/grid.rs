//! Turns a statement file into rows of text cells, whatever the container format.
//! Banks often send HTML or tab-separated text with an ".xls" name, so the format is chosen from the
//! file content, not the extension.

use calamine::{Data, DataType, Reader, Xls, Xlsx};
use regex::Regex;
use std::io::Cursor;
use std::sync::LazyLock;

pub type Grid = Vec<Vec<String>>;

pub const PDF_NOT_YET: &str = "PDF statements are not supported yet. Please download the statement from net banking as Excel (XLS/XLSX) or CSV.";

fn clean(s: &str) -> String {
    s.replace(['\u{a0}', '\r', '\n', '\t'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cell_text(c: &Data) -> String {
    match c {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => clean(s),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                format!("{f}")
            }
        }
        Data::DateTime(_) => match DataType::as_datetime(c) {
            Some(dt) => dt.format("%d/%m/%Y").to_string(),
            None => String::new(),
        },
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
    }
}

fn sheets<R: Reader<Cursor<Vec<u8>>>>(mut wb: R) -> Result<Vec<Grid>, String> {
    let mut out = Vec::new();
    for name in wb.sheet_names().to_vec() {
        if let Ok(range) = wb.worksheet_range(&name) {
            let grid: Grid = range.rows().map(|r| r.iter().map(cell_text).collect()).collect();
            if !grid.is_empty() {
                out.push(grid);
            }
        }
    }
    if out.is_empty() {
        return Err("The workbook has no readable sheets.".into());
    }
    Ok(out)
}

fn decode_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && (bytes[..2] == [0xFF, 0xFE] || bytes[..2] == [0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    String::from_utf8_lossy(b).into_owned()
}

static TR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<tr[^>]*>(.*?)</tr\s*>").unwrap());
static TD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<t[dh][^>]*>(.*?)</t[dh]\s*>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());

fn html_unescape(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn html_grid(text: &str) -> Grid {
    TR.captures_iter(text)
        .map(|tr| {
            TD.captures_iter(&tr[1])
                .map(|td| clean(&html_unescape(&TAG.replace_all(&td[1], " "))))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn text_grid(text: &str) -> Result<Grid, String> {
    let sample: Vec<&str> = text.lines().take(60).collect();
    let count = |ch: char| sample.iter().map(|l| l.matches(ch).count()).sum::<usize>();
    let candidates = [('\t', count('\t')), (',', count(',')), (';', count(';')), ('|', count('|'))];
    let (delim, n) = candidates.iter().copied().max_by_key(|(_, n)| *n).unwrap();
    if n == 0 {
        return Err("Could not read the file: no columns found.".into());
    }
    let mut rb = csv::ReaderBuilder::new();
    rb.has_headers(false).flexible(true).delimiter(delim as u8);
    // Tab-separated exports sometimes contain stray quote characters inside narrations.
    if delim == '\t' {
        rb.quoting(false);
    }
    let mut out = Vec::new();
    for rec in rb.from_reader(text.as_bytes()).records() {
        let rec = rec.map_err(|e| format!("Could not read the file: {e}"))?;
        out.push(rec.iter().map(|c| clean(c.trim().trim_matches('"').trim_start_matches('\'').trim())).collect());
    }
    Ok(out)
}

/// Returns one grid per sheet (text formats give a single grid).
pub fn read(bytes: &[u8]) -> Result<Vec<Grid>, String> {
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        let wb: Xls<_> = Xls::new(Cursor::new(bytes.to_vec())).map_err(|e| format!("Could not open the Excel file: {e}"))?;
        return sheets(wb);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        let wb: Xlsx<_> = Xlsx::new(Cursor::new(bytes.to_vec())).map_err(|e| format!("Could not open the Excel file: {e}"))?;
        return sheets(wb);
    }
    if bytes.starts_with(b"%PDF") {
        return Err(PDF_NOT_YET.into());
    }
    let text = decode_text(bytes);
    let head = text.trim_start();
    if head.starts_with('<') && TR.is_match(&text) {
        return Ok(vec![html_grid(&text)]);
    }
    Ok(vec![text_grid(&text)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_and_tsv() {
        let g = read(b"\xEF\xBB\xBFDate,Narration,Amount\n01/04/25,\"Swiggy, order\",\"1,234.00\"\n").unwrap();
        assert_eq!(g[0][1], vec!["01/04/25", "Swiggy, order", "1,234.00"]);
        let g = read(b"Txn Date\tDescription\tDebit\n1 Apr 2025\t  TO TRANSFER-UPI \"x\t 500.00\n").unwrap();
        assert_eq!(g[0][1], vec!["1 Apr 2025", "TO TRANSFER-UPI \"x", "500.00"]);
    }

    #[test]
    fn html_and_pdf() {
        let g = read(b"<html><table><tr><th>Date</th><th>Narration</th></tr><tr><td>01/04/2025</td><td>A &amp; B<br/>C</td></tr></table>").unwrap();
        assert_eq!(g[0][1], vec!["01/04/2025", "A & B C"]);
        assert_eq!(read(b"%PDF-1.7 ...").err().unwrap(), PDF_NOT_YET);
    }

    #[test]
    fn utf16() {
        let s = "Date\tAmount\n01/04/2025\t10\n";
        let mut b = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(read(&b).unwrap()[0][1], vec!["01/04/2025", "10"]);
    }
}
