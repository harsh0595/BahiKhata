//! Excel export of the current report.

use crate::analytics::{dashboard, matching, Scope, TxnQuery};
use crate::model::*;
use chrono::NaiveDate;
use rust_xlsxwriter::{Chart, ChartType, Format, Workbook, Worksheet, XlsxError};

fn rupees(p: Paise) -> f64 {
    p as f64 / 100.0
}

fn excel_date(d: NaiveDate) -> f64 {
    (d - NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()).num_days() as f64
}

struct Fmt {
    bold: Format,
    head: Format,
    money: Format,
    date: Format,
    pct: Format,
}

fn header(ws: &mut Worksheet, f: &Fmt, cols: &[(&str, f64)]) -> Result<(), XlsxError> {
    for (i, (name, width)) in cols.iter().enumerate() {
        ws.write_string_with_format(0, i as u16, *name, &f.head)?;
        ws.set_column_width(i as u16, *width)?;
    }
    ws.set_freeze_panes(1, 0)?;
    Ok(())
}

fn build(v: &Vault, scope: &Scope, scope_label: &str) -> Result<Workbook, String> {
    let db = dashboard(v, scope)?;
    let x = |e: XlsxError| e.to_string();
    let f = Fmt {
        bold: Format::new().set_bold(),
        head: Format::new().set_bold().set_background_color("#E8EAF6"),
        money: Format::new().set_num_format("#,##0.00"),
        date: Format::new().set_num_format("dd-mm-yyyy"),
        pct: Format::new().set_num_format("0.0%"),
    };
    let mut wb = Workbook::new();

    // Summary
    let mut ws = Worksheet::new();
    ws.set_name("Summary").map_err(x)?;
    ws.set_column_width(0, 28).map_err(x)?;
    ws.set_column_width(1, 22).map_err(x)?;
    ws.write_string_with_format(0, 0, "Bahi-Khata report", &f.bold).map_err(x)?;
    ws.write_string(1, 0, "Accounts").map_err(x)?;
    ws.write_string(1, 1, scope_label).map_err(x)?;
    let period = match (db.first_date, db.last_date) {
        (Some(a), Some(b)) => format!("{} to {}", a.format("%d-%m-%Y"), b.format("%d-%m-%Y")),
        _ => "No transactions".into(),
    };
    ws.write_string(2, 0, "Period").map_err(x)?;
    ws.write_string(2, 1, &period).map_err(x)?;
    let rows: [(&str, Paise); 5] = [
        ("Income", db.income),
        ("Expenses", db.expenses),
        ("Investments", db.invested),
        ("Net savings", db.net),
        ("Excluded (own transfers, card bills)", db.excluded),
    ];
    for (i, (name, val)) in rows.iter().enumerate() {
        let r = 4 + i as u32;
        ws.write_string(r, 0, *name).map_err(x)?;
        ws.write_number_with_format(r, 1, rupees(*val), &f.money).map_err(x)?;
    }
    ws.write_string(9, 0, "Savings rate").map_err(x)?;
    match db.savings_rate {
        Some(s) => ws.write_number_with_format(9, 1, s, &f.pct).map_err(x)?,
        None => ws.write_string(9, 1, "-").map_err(x)?,
    };
    ws.write_string(10, 0, "Investments counted as expense").map_err(x)?;
    ws.write_string(10, 1, if db.investments_as_expense { "Yes" } else { "No" }).map_err(x)?;
    ws.write_string(11, 0, "Transactions").map_err(x)?;
    ws.write_number(11, 1, db.txn_count as f64).map_err(x)?;
    wb.push_worksheet(ws);

    // Monthly
    let mut ws = Worksheet::new();
    ws.set_name("Monthly").map_err(x)?;
    header(&mut ws, &f, &[("Month", 12.0), ("Income", 16.0), ("Expenses", 16.0), ("Investments", 16.0), ("Net", 16.0)]).map_err(x)?;
    for (i, m) in db.months.iter().enumerate() {
        let r = 1 + i as u32;
        ws.write_string(r, 0, &m.month).map_err(x)?;
        for (c, val) in [m.income, m.expenses, m.invested, m.net].iter().enumerate() {
            ws.write_number_with_format(r, 1 + c as u16, rupees(*val), &f.money).map_err(x)?;
        }
    }
    if !db.months.is_empty() {
        let last = db.months.len() as u32;
        let mut chart = Chart::new(ChartType::Column);
        chart.title().set_name("Income vs expenses by month");
        for (col, name) in [(1u16, "Income"), (2, "Expenses"), (3, "Investments")] {
            chart
                .add_series()
                .set_name(name)
                .set_categories(("Monthly", 1, 0, last, 0))
                .set_values(("Monthly", 1, col, last, col));
        }
        chart.set_width(760).set_height(380);
        ws.insert_chart(1, 6, &chart).map_err(x)?;
    }
    wb.push_worksheet(ws);

    // Categories
    let mut ws = Worksheet::new();
    ws.set_name("Categories").map_err(x)?;
    header(&mut ws, &f, &[("Type", 12.0), ("Category", 26.0), ("Amount", 16.0), ("Share", 10.0), ("Transactions", 13.0)]).map_err(x)?;
    let mut r = 1u32;
    for (kind, list) in [("Expense", &db.expense_categories), ("Income", &db.income_categories), ("Investment", &db.investment_categories)] {
        for c in list.iter() {
            ws.write_string(r, 0, kind).map_err(x)?;
            ws.write_string(r, 1, &c.name).map_err(x)?;
            ws.write_number_with_format(r, 2, rupees(c.amount), &f.money).map_err(x)?;
            ws.write_number_with_format(r, 3, c.share, &f.pct).map_err(x)?;
            ws.write_number(r, 4, c.count as f64).map_err(x)?;
            r += 1;
        }
    }
    wb.push_worksheet(ws);

    // Merchants
    let mut ws = Worksheet::new();
    ws.set_name("Top merchants").map_err(x)?;
    header(&mut ws, &f, &[("Merchant", 32.0), ("Category", 22.0), ("Amount", 16.0), ("Transactions", 13.0)]).map_err(x)?;
    for (i, m) in db.merchants.iter().enumerate() {
        let r = 1 + i as u32;
        ws.write_string(r, 0, &m.merchant).map_err(x)?;
        ws.write_string(r, 1, &m.category).map_err(x)?;
        ws.write_number_with_format(r, 2, rupees(m.amount), &f.money).map_err(x)?;
        ws.write_number(r, 3, m.count as f64).map_err(x)?;
    }
    wb.push_worksheet(ws);

    // Recurring
    let mut ws = Worksheet::new();
    ws.set_name("Recurring").map_err(x)?;
    header(&mut ws, &f, &[("Payee", 32.0), ("Category", 22.0), ("Typical amount", 16.0), ("Months", 9.0), ("Total", 16.0), ("Last paid", 12.0)]).map_err(x)?;
    for (i, m) in db.recurring.iter().enumerate() {
        let r = 1 + i as u32;
        ws.write_string(r, 0, &m.merchant).map_err(x)?;
        ws.write_string(r, 1, &m.category).map_err(x)?;
        ws.write_number_with_format(r, 2, rupees(m.typical), &f.money).map_err(x)?;
        ws.write_number(r, 3, m.months as f64).map_err(x)?;
        ws.write_number_with_format(r, 4, rupees(m.total), &f.money).map_err(x)?;
        ws.write_number_with_format(r, 5, excel_date(m.last_date), &f.date).map_err(x)?;
    }
    wb.push_worksheet(ws);

    // Transactions
    let mut ws = Worksheet::new();
    ws.set_name("Transactions").map_err(x)?;
    header(
        &mut ws,
        &f,
        &[
            ("Date", 12.0),
            ("Account", 16.0),
            ("Narration", 60.0),
            ("Merchant", 24.0),
            ("Channel", 10.0),
            ("Debit", 14.0),
            ("Credit", 14.0),
            ("Balance", 16.0),
            ("Category", 22.0),
            ("Note", 24.0),
        ],
    )
    .map_err(x)?;
    let q = TxnQuery { scope: scope.clone(), sort: "date_asc".into(), ..Default::default() };
    let txns = matching(v, &q);
    for (i, t) in txns.iter().enumerate() {
        let r = 1 + i as u32;
        ws.write_number_with_format(r, 0, excel_date(t.date), &f.date).map_err(x)?;
        ws.write_string(r, 1, v.account(t.account_id).map(account_label).unwrap_or_default()).map_err(x)?;
        ws.write_string(r, 2, &t.narration).map_err(x)?;
        ws.write_string(r, 3, &t.merchant).map_err(x)?;
        ws.write_string(r, 4, &t.channel).map_err(x)?;
        let col = if t.direction == Direction::Debit { 5 } else { 6 };
        ws.write_number_with_format(r, col, rupees(t.amount), &f.money).map_err(x)?;
        if let Some(b) = t.balance {
            ws.write_number_with_format(r, 7, rupees(b), &f.money).map_err(x)?;
        }
        ws.write_string(r, 8, v.category(&t.category).map(|c| c.name.as_str()).unwrap_or(&t.category)).map_err(x)?;
        ws.write_string(r, 9, &t.note).map_err(x)?;
    }
    if !txns.is_empty() {
        ws.autofilter(0, 0, txns.len() as u32, 9).map_err(x)?;
    }
    wb.push_worksheet(ws);
    Ok(wb)
}

/// Builds the .xlsx file in memory.
pub fn export_xlsx(v: &Vault, scope: &Scope, scope_label: &str) -> Result<Vec<u8>, String> {
    let mut wb = build(v, scope, scope_label)?;
    wb.save_to_buffer().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::parse_statement;
    use crate::store::commit;

    #[test]
    fn workbook_is_valid_zip_and_readable() {
        let mut v = Vault::new();
        let s = parse_statement(include_bytes!("../tests/fixtures/hdfc_sample.csv"), "h.csv").unwrap();
        let id = commit(&mut v, &s, "h.csv", None).unwrap().account_id;
        let bytes = export_xlsx(&v, &Scope { accounts: vec![id], from: None, to: None }, "HDFC ••6789").unwrap();
        assert!(bytes.starts_with(b"PK"));
        // Read it back with the importer's Excel reader.
        let grids = crate::import::grid::read(&bytes).unwrap();
        assert_eq!(grids.len(), 6);
        let txns = &grids[5];
        assert_eq!(txns[0][0], "Date");
        assert_eq!(txns.len(), 7);
        assert_eq!(txns[1][0], "01/04/2025");
    }
}
