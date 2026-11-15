# Bahi-Khata

An offline Windows desktop app that imports bank statements, categorises every transaction and shows where the
money goes. It needs no internet access. All data stays on the PC in a password-encrypted vault.

## Features
- Imports **SBI** and **HDFC** savings-account statements (CSV, XLS, XLSX). The bank and the layout are detected
  automatically. PDF import is planned.
- Categorises transactions offline, using rules plus parsing of UPI, NEFT, IMPS, POS, ATM and ACH narrations.
  When you correct a category, the app offers to turn it into a rule.
- Guesses the rest: small UPI payments (up to ₹300 by default, adjustable in Settings) count as Groceries, and a
  payee you have categorised before gets the same category again. Guesses are marked so you can check them.
- **Review** page: transactions the app could not categorise are grouped by payee. Pick a category once and tick
  "Remember" so future payments to that payee are categorised automatically.
- Detects transfers between your own accounts, payments to your own name and credit-card bill payments, and leaves
  them out of income and expense. A failed payment and its reversal cancel each other out.
- Can combine several accounts, but only when they have the same holder.
- Dashboard: income, expenses, investments, net savings and savings rate; a monthly trend; a category donut and
  category-by-month chart; top payees; recurring payments. Click any chart or list to see the transactions behind it.
- Export to Excel, or print a report to PDF.
- Backup and restore of the encrypted vault, auto-lock, password change, delete everything.

## Install
Download `BahiKhata-<version>-setup.exe` from **Releases** and run it. The installer is unsigned, so Windows
SmartScreen shows a warning: choose *More info → Run anyway*.

## Data and privacy
- The vault is `%APPDATA%\com.harsh0595.bahikhata\vault.bkv`. It is JSON, encrypted with XChaCha20-Poly1305
  using a key derived from your password with Argon2id. The password is never stored.
- **A forgotten password cannot be recovered.** You can re-import your statements into a new vault.
- Never commit real statements to this repository.

## Development
- Stack: Tauri 2 (Rust) and Svelte 5 + TypeScript + Chart.js. The domain logic is in `core/`, a plain Rust crate
  with unit tests. The Tauri commands are in `src-tauri/src/lib.rs`, and the UI is in `src/`.
- CI (`.github/workflows/build.yml`) runs the core tests and `svelte-check`, builds the NSIS installer as version
  `0.1.<run number>` and publishes it as a GitHub Release. Each run's results are pushed to the `ci-results` branch.
- To build locally (Node 22 and Rust stable required): `npm ci`, `cargo test --manifest-path core/Cargo.toml`,
  `npm run tauri dev`.

See [docs/spec.md](docs/spec.md) for the full specification.
