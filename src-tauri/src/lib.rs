//! Tauri commands: a thin layer over `bk_core`. The vault lives in memory while unlocked and is
//! re-encrypted to disk after every change.

use bk_core::analytics::{self, Dashboard, Scope, ScopeOption, TxnPage, TxnQuery};
use bk_core::import::{parse_statement, ParsedStatement};
use bk_core::model::*;
use bk_core::store::{self, ImportPreview, ImportResult};
use bk_core::vault::{self, KdfParams, VaultKey};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};
use zeroize::Zeroizing;

const MIN_PASSWORD: usize = 8;
const MAX_STATEMENT_BYTES: u64 = 50 * 1024 * 1024;

struct Session {
    key: VaultKey,
    vault: Vault,
}

struct AppState {
    path: PathBuf,
    session: Mutex<Option<Session>>,
    pending: Mutex<Option<(ParsedStatement, String)>>,
}

type R<T> = Result<T, String>;

fn save(path: &Path, s: &Session) -> R<()> {
    let json = Zeroizing::new(serde_json::to_vec(&s.vault).map_err(|e| e.to_string())?);
    let sealed = s.key.seal(&json)?;
    vault::write_atomic(path, &sealed)
}

fn read<T>(st: &AppState, f: impl FnOnce(&Vault) -> R<T>) -> R<T> {
    let g = st.session.lock().map_err(|_| "Internal error")?;
    let s = g.as_ref().ok_or("Locked")?;
    f(&s.vault)
}

fn write<T>(st: &AppState, f: impl FnOnce(&mut Vault) -> R<T>) -> R<T> {
    let mut g = st.session.lock().map_err(|_| "Internal error")?;
    let s = g.as_mut().ok_or("Locked")?;
    let out = f(&mut s.vault)?;
    save(&st.path, s)?;
    Ok(out)
}

fn check_password(p: &str) -> R<()> {
    if p.chars().count() < MIN_PASSWORD {
        return Err(format!("The password must be at least {MIN_PASSWORD} characters."));
    }
    Ok(())
}

fn open_vault(data: &[u8], password: &str) -> R<Session> {
    let (key, plain) = vault::unlock(data, password)?;
    let vault: Vault = serde_json::from_slice(&plain).map_err(|e| format!("Data file is damaged: {e}"))?;
    Ok(Session { key, vault })
}

// ---------- vault ----------

#[derive(Serialize)]
struct Status {
    exists: bool,
    unlocked: bool,
    path: String,
    version: String,
    auto_lock_minutes: u32,
}

#[tauri::command]
fn vault_status(app: tauri::AppHandle, st: State<'_, AppState>) -> R<Status> {
    let g = st.session.lock().map_err(|_| "Internal error")?;
    Ok(Status {
        exists: st.path.exists(),
        unlocked: g.is_some(),
        path: st.path.display().to_string(),
        version: app.package_info().version.to_string(),
        auto_lock_minutes: g.as_ref().map(|s| s.vault.settings.auto_lock_minutes).unwrap_or(15),
    })
}

#[tauri::command]
async fn create_vault(password: String, st: State<'_, AppState>) -> R<()> {
    let password = Zeroizing::new(password);
    if st.path.exists() {
        return Err("A data file already exists.".into());
    }
    check_password(&password)?;
    let s = Session { key: VaultKey::create(&password, KdfParams::DEFAULT)?, vault: Vault::new() };
    save(&st.path, &s)?;
    *st.session.lock().map_err(|_| "Internal error")? = Some(s);
    Ok(())
}

#[tauri::command]
async fn unlock(password: String, st: State<'_, AppState>) -> R<()> {
    let password = Zeroizing::new(password);
    let data = std::fs::read(&st.path).map_err(|e| format!("Cannot read data file: {e}"))?;
    let s = open_vault(&data, &password)?;
    *st.session.lock().map_err(|_| "Internal error")? = Some(s);
    Ok(())
}

#[tauri::command]
fn lock(st: State<'_, AppState>) -> R<()> {
    *st.session.lock().map_err(|_| "Internal error")? = None;
    *st.pending.lock().map_err(|_| "Internal error")? = None;
    Ok(())
}

#[tauri::command]
async fn change_password(current: String, new: String, st: State<'_, AppState>) -> R<()> {
    let (current, new) = (Zeroizing::new(current), Zeroizing::new(new));
    check_password(&new)?;
    let data = std::fs::read(&st.path).map_err(|e| format!("Cannot read data file: {e}"))?;
    vault::unlock(&data, &current).map_err(|_| "The current password is wrong.".to_string())?;
    let key = VaultKey::create(&new, KdfParams::DEFAULT)?;
    let mut g = st.session.lock().map_err(|_| "Internal error")?;
    let s = g.as_mut().ok_or("Locked")?;
    s.key = key;
    save(&st.path, s)?;
    // The backup copy is still sealed with the old password.
    let _ = std::fs::remove_file(st.path.with_extension("bak"));
    Ok(())
}

#[tauri::command]
fn delete_all(confirm: String, st: State<'_, AppState>) -> R<()> {
    if confirm != "DELETE" {
        return Err("Type DELETE to confirm.".into());
    }
    *st.session.lock().map_err(|_| "Internal error")? = None;
    *st.pending.lock().map_err(|_| "Internal error")? = None;
    for ext in ["bkv", "bak", "tmp"] {
        let p = st.path.with_extension(ext);
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| format!("Cannot delete {}: {e}", p.display()))?;
        }
    }
    Ok(())
}

/// Copies the encrypted data file. It can only be opened with the app password.
#[tauri::command]
fn backup_vault(dest: String, st: State<'_, AppState>) -> R<()> {
    let _g = st.session.lock().map_err(|_| "Internal error")?;
    std::fs::copy(&st.path, &dest).map_err(|e| format!("Backup failed: {e}"))?;
    Ok(())
}

/// Replaces the current data with a backup file, which must open with the given password.
#[tauri::command]
async fn restore_vault(src: String, password: String, st: State<'_, AppState>) -> R<()> {
    let password = Zeroizing::new(password);
    let data = std::fs::read(&src).map_err(|e| format!("Cannot read backup: {e}"))?;
    let s = open_vault(&data, &password)?;
    vault::write_atomic(&st.path, &data)?;
    *st.session.lock().map_err(|_| "Internal error")? = Some(s);
    *st.pending.lock().map_err(|_| "Internal error")? = None;
    Ok(())
}

// ---------- settings ----------

#[tauri::command]
fn get_settings(st: State<'_, AppState>) -> R<Settings> {
    read(&st, |v| Ok(v.settings.clone()))
}

#[tauri::command]
fn set_settings(settings: Settings, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| {
        v.settings = Settings { auto_lock_minutes: settings.auto_lock_minutes.clamp(1, 240), ..settings };
        Ok(())
    })
}

// ---------- import ----------

#[tauri::command]
fn preview_import(path: String, st: State<'_, AppState>) -> R<ImportPreview> {
    let p = Path::new(&path);
    let size = std::fs::metadata(p).map_err(|e| format!("Cannot open file: {e}"))?.len();
    if size > MAX_STATEMENT_BYTES {
        return Err("The file is too large to be a bank statement.".into());
    }
    let bytes = std::fs::read(p).map_err(|e| format!("Cannot open file: {e}"))?;
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let parsed = parse_statement(&bytes, &name)?;
    let pv = read(&st, |v| Ok(store::preview(v, &parsed, &name)))?;
    *st.pending.lock().map_err(|_| "Internal error")? = Some((parsed, name));
    Ok(pv)
}

#[tauri::command]
fn commit_import(holder: Option<String>, st: State<'_, AppState>) -> R<ImportResult> {
    let pending = st.pending.lock().map_err(|_| "Internal error")?.take();
    let (parsed, name) = pending.ok_or("Nothing to import. Please choose the file again.")?;
    let res = write(&st, |v| store::commit(v, &parsed, &name, holder.as_deref()));
    if res.is_err() {
        *st.pending.lock().map_err(|_| "Internal error")? = Some((parsed, name));
    }
    res
}

#[tauri::command]
fn cancel_import(st: State<'_, AppState>) -> R<()> {
    *st.pending.lock().map_err(|_| "Internal error")? = None;
    Ok(())
}

#[derive(Serialize)]
struct ImportRow {
    #[serde(flatten)]
    rec: ImportRecord,
    account: String,
}

#[tauri::command]
fn imports(st: State<'_, AppState>) -> R<Vec<ImportRow>> {
    read(&st, |v| {
        let mut out: Vec<ImportRow> = v
            .imports
            .iter()
            .map(|i| ImportRow { rec: i.clone(), account: v.account(i.account_id).map(account_label).unwrap_or_default() })
            .collect();
        out.reverse();
        Ok(out)
    })
}

#[tauri::command]
fn delete_import(id: u64, st: State<'_, AppState>) -> R<usize> {
    write(&st, |v| store::delete_import(v, id))
}

// ---------- accounts ----------

#[derive(Serialize)]
struct AccountRow {
    #[serde(flatten)]
    acc: Account,
    label: String,
    txn_count: usize,
    first_date: Option<chrono::NaiveDate>,
    last_date: Option<chrono::NaiveDate>,
    balance: Option<Paise>,
}

#[tauri::command]
fn accounts(st: State<'_, AppState>) -> R<Vec<AccountRow>> {
    read(&st, |v| {
        Ok(v.accounts
            .iter()
            .map(|a| {
                let txns: Vec<&Txn> = v.txns.iter().filter(|t| t.account_id == a.id).collect();
                AccountRow {
                    acc: a.clone(),
                    label: account_label(a),
                    txn_count: txns.len(),
                    first_date: txns.first().map(|t| t.date),
                    last_date: txns.last().map(|t| t.date),
                    balance: txns.iter().rev().find_map(|t| t.balance),
                }
            })
            .collect())
    })
}

#[tauri::command]
fn update_account(id: u64, nickname: String, holder: Option<String>, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::update_account(v, id, &nickname, holder.as_deref()))
}

#[tauri::command]
fn delete_account(id: u64, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::delete_account(v, id))
}

// ---------- categories and rules ----------

#[tauri::command]
fn categories(st: State<'_, AppState>) -> R<Vec<Category>> {
    read(&st, |v| Ok(v.categories.clone()))
}

#[tauri::command]
fn add_category(name: String, kind: CatKind, color: String, st: State<'_, AppState>) -> R<String> {
    write(&st, |v| store::add_category(v, &name, kind, &color))
}

#[tauri::command]
fn update_category(id: String, name: String, color: String, kind: Option<CatKind>, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::update_category(v, &id, &name, &color, kind))
}

#[tauri::command]
fn delete_category(id: String, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::delete_category(v, &id))
}

#[tauri::command]
fn rules(st: State<'_, AppState>) -> R<Vec<Rule>> {
    read(&st, |v| {
        let mut r = v.rules.clone();
        r.reverse();
        Ok(r)
    })
}

#[tauri::command]
fn rule_matches(pattern: String, direction: Option<Direction>, st: State<'_, AppState>) -> R<usize> {
    read(&st, |v| Ok(store::rule_matches(v, &pattern, direction)))
}

#[tauri::command]
fn add_rule(pattern: String, category: String, direction: Option<Direction>, st: State<'_, AppState>) -> R<u64> {
    write(&st, |v| store::add_rule(v, &pattern, &category, direction))
}

#[tauri::command]
fn delete_rule(id: u64, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::delete_rule(v, id))
}

// ---------- transactions ----------

#[tauri::command]
fn query_txns(query: TxnQuery, st: State<'_, AppState>) -> R<TxnPage> {
    read(&st, |v| analytics::query(v, &query))
}

#[tauri::command]
fn set_category(ids: Vec<u64>, category: String, st: State<'_, AppState>) -> R<usize> {
    write(&st, |v| store::set_category(v, &ids, &category))
}

#[tauri::command]
fn reset_category(ids: Vec<u64>, st: State<'_, AppState>) -> R<usize> {
    write(&st, |v| Ok(store::reset_category(v, &ids)))
}

#[tauri::command]
fn set_note(id: u64, note: String, st: State<'_, AppState>) -> R<()> {
    write(&st, |v| store::set_note(v, id, &note))
}

// ---------- reports ----------

#[tauri::command]
fn scope_options(st: State<'_, AppState>) -> R<Vec<ScopeOption>> {
    read(&st, |v| Ok(analytics::scope_options(v)))
}

#[tauri::command]
fn dashboard(scope: Scope, st: State<'_, AppState>) -> R<Dashboard> {
    read(&st, |v| analytics::dashboard(v, &scope))
}

#[tauri::command]
fn export_excel(dest: String, scope: Scope, label: String, st: State<'_, AppState>) -> R<()> {
    let bytes = read(&st, |v| bk_core::export::export_xlsx(v, &scope, &label))?;
    std::fs::write(&dest, bytes).map_err(|e| format!("Cannot save the Excel file (is it open in Excel?): {e}"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.manage(AppState { path: dir.join("vault.bkv"), session: Mutex::new(None), pending: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_status,
            create_vault,
            unlock,
            lock,
            change_password,
            delete_all,
            backup_vault,
            restore_vault,
            get_settings,
            set_settings,
            preview_import,
            commit_import,
            cancel_import,
            imports,
            delete_import,
            accounts,
            update_account,
            delete_account,
            categories,
            add_category,
            update_category,
            delete_category,
            rules,
            rule_matches,
            add_rule,
            delete_rule,
            query_txns,
            set_category,
            reset_category,
            set_note,
            scope_options,
            dashboard,
            export_excel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bahi-Khata");
}
