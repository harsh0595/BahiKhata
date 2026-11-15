# Bahi-Khata — Specification

Offline Windows desktop app that imports bank statements, categorises transactions and shows an
income/expense dashboard. No internet access is required or used.

## Platform & repo
- Tauri 2 (Rust backend + web UI), Windows 10/11 x64, small installer (.msi/.exe).
- Private repo `harsh0595/BahiKhata` on the personal account; separate deploy key (as with NavRelay).
- GitHub Actions (windows runner) builds the installer and publishes it as a GitHub Release.
  No local toolchain is needed.
- Data: a single encrypted vault file (`vault.bkv` in %APPDATA%\com.harsh0595.bahikhata) holding JSON, sealed with
  XChaCha20-Poly1305; key from Argon2id (64 MiB, t=3) of the **app password** entered at startup. Atomic writes keep an
  encrypted `.bak`. (SQLite/SQLCipher was dropped: needless native build complexity for this data size.)
  Forgotten password = data unrecoverable (statements can be re-imported). The password is never stored.

## Import
- Banks v1: **SBI** and **HDFC** savings accounts. Bank detected automatically from file content.
- Formats: CSV, XLS (incl. SBI's tab-separated ".xls" text), XLSX first; then PDF (text-based, password-protected;
  the app prompts for the password and does not store it). Scanned PDFs (OCR) are out of scope.
- Header-synonym matching (no fixed header strings); skip preamble and summary/footer rows; multiple date formats;
  Indian comma grouping; blank/space-only debit/credit cells.
- Extract account holder name, account no. (masked), PAN if present, and statement period.
- Duplicate/overlapping imports are de-duplicated.
- Real sample statement per bank to be provided by the user later (amounts may be masked).
- Credit-card statements (PDF only): after v1.

## Accounts
- Multiple files/accounts/banks supported; reports can be per account or combined.
- Combining is allowed only when holder name (and PAN, if available) match across accounts.
  **On mismatch, combining is never allowed**; separate reports are shown.
- Transfers between the user's own accounts and credit-card bill payments are excluded from income/expense.

## Categories (user can add/rename later)
- Expense: Groceries, Food & Dining, Fuel, Transport, Travel, Shopping, Entertainment & Subscriptions, Healthcare,
  Rent, Utilities & Bills, Education, Insurance, EMI & Loans, Investments, Cash Withdrawal, Bank Fees & Charges, Other.
- Income: Salary, Interest, Dividends, Refunds & Cashback, Other Income.
- Also: Taxes, Gifts & Donations, Family & Friends (expense).
- Excluded: Own-account Transfer, Credit Card Bill Payment, Reversed / Failed Payment.
- Categorisation is offline and rule-based (keywords, merchant names, UPI VPA/narration parsing).
  Changing a transaction's category offers to create a rule, so the app learns.
- Order: user rules → payment to the holder's own name (own transfer) → card-bill payment → built-in keywords.
  Anything left is guessed: a payee previously categorised by a rule or by the user gets the same category;
  otherwise a UPI debit up to the small-UPI limit (default ₹300, Settings) gets the small-UPI category (default
  Groceries; street vegetable/fruit vendors). Auto guesses never feed the payee history.
- A debit and a same-amount credit on the same account within 7 days that share a reference number are both marked
  Reversed / Failed Payment.
- Review page: uncategorised transactions grouped by payee (and a second tab for auto guesses). Assigning a
  category from the fixed list can also create a rule for the payee. The app opens Review after an import when
  anything is left uncategorised.
- Investments are shown separately by default (Income − Expenses − Investments = Net savings).
  A checkbox switches to counting Investments as an expense.

## Dashboard v1
1. Summary cards: Income, Expenses, Investments, Net savings, Savings rate.
2. Monthly trend: income vs expense bars with a net-savings line.
3. Category donut; clicking drills down to transactions.
4. Category-by-month stacked chart.
5. Top merchants; auto-detected recurring payments (subscriptions, EMIs, SIPs, rent).
6. Transactions table: search, filter (incl. by how it was categorised), edit category (offers a rule).
7. Filters: date range, account, "count investments as expense".
8. Export of reports to Excel and PDF.

## Later / ideas
- Credit-card statements, budgets with alerts, more banks (ICICI, Axis, Kotak), OCR.

## Format research (summary)
- HDFC: `Date, Narration, Chq./Ref.No., Value Dt, Withdrawal Amt., Deposit Amt., Closing Balance`; dd/mm/yy;
  a blank line before the header; `STATEMENT SUMMARY` footer. UPI: `UPI-<merchant>-<vpa>-<IFSC>-<RRN>-<remark>`.
- SBI: `Txn Date, Value Date, Description, Ref No./Cheque No., Debit, Credit, Balance`; mixed date formats;
  ".xls" is often tab-separated text; another XLSX layout has a single signed Amount column.
  UPI: `TO TRANSFER-UPI/DR/<ref>/<NAME>/<BANK>/<vpa>/<remark>`.
- References: pratikPsh/burnrate, swaroop-rayker/LedgerFlow (fixtures), NikethNath/parakkan (SBI formats),
  atul0016/bank-statement-parser (SBI PDF), dmadan86/waves (narrations). PDF password conventions are unverified, so the app always prompts.
