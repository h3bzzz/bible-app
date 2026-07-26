//! The desktop shell.
//!
//! This file is the composition root (§5): it opens the database, wires the
//! use cases, and registers the command surface. There is no dependency
//! injection layer, because there is nothing here for one to do — the
//! dependencies are wired explicitly, in `main`, in about twenty lines.
//!
//! There is deliberately no logic below. Everything a command does lives in
//! `bible-app-core`, where it is tested without a window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bible_app_core::view::LayerView;
use bible_app_core::{ChapterView, LoadedLayer, TocView, TranslationView};
use bible_storage::CorpusDb;
use bible_versification::SchemeRegistry;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

/// Everything the commands need. Built once in `main`.
///
/// The corpus sits behind a `Mutex` because `rusqlite::Connection` is `Send`
/// but not `Sync`, and Tauri state must be both. This costs nothing real: the
/// boundary is one coarse call per screen (ADR-0001), so the lock is taken a
/// handful of times per user interaction, never per verse.
struct AppState {
    corpus: Mutex<CorpusDb>,
    /// Every annotation layer compiled into the binary (ADR-0009), still in
    /// `org`. Which of them are *active* is the UI's per-request choice.
    layers: Vec<LoadedLayer>,
    /// Versification tables, for mapping layer ranges into the scheme of
    /// whichever translation the reader is in.
    registry: SchemeRegistry,
}

impl AppState {
    fn corpus(&self) -> Result<std::sync::MutexGuard<'_, CorpusDb>, String> {
        // A poisoned lock means a command panicked mid-read. Nothing here
        // mutates, so the data is intact; report rather than unwrap.
        self.corpus.lock().map_err(|_| "corpus lock poisoned".to_string())
    }
}

// One coarse call per screen (ADR-0001): the picker, the contents, the page.
// The function names below are the wire protocol — `invoke('get_chapter')` in
// the UI resolves by this exact name, which is why none of them carry a
// `_cmd` suffix.

#[tauri::command]
fn list_translations(state: State<'_, AppState>) -> Result<Vec<TranslationView>, String> {
    let corpus = state.corpus()?;
    bible_app_core::list_translations(&corpus).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_toc(state: State<'_, AppState>, translation: String) -> Result<TocView, String> {
    let corpus = state.corpus()?;
    bible_app_core::table_of_contents(&corpus, &translation).map_err(|e| e.to_string())
}

/// The desktop environment, so the UI can adapt its window controls.
/// Hyprland has no minimize concept, so the minimize button would be a lie
/// there; the UI hides it.
#[tauri::command]
fn desktop_env() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
}

/// Toggles between maximized and the smaller window.
///
/// Not done through the JS window API because a common Hyprland config rule
/// (`suppress_event = "maximize"`, present in this machine's config and in
/// the stock example config) silently swallows client maximize requests.
/// The `fullscreen 1` *dispatcher* is Hyprland's own maximize toggle and is
/// not suppressible; it acts on the focused window, which is this one —
/// the user just clicked our button.
#[tauri::command]
fn toggle_maximize(window: tauri::Window) -> Result<(), String> {
    if std::env::var("XDG_CURRENT_DESKTOP").as_deref() == Ok("Hyprland") {
        std::process::Command::new("hyprctl")
            .args(["dispatch", "fullscreen", "1"])
            .status()
            .map_err(|e| format!("hyprctl failed: {e}"))?;
        return Ok(());
    }
    let maximized = window.is_maximized().map_err(|e| e.to_string())?;
    if maximized {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn list_layers(state: State<'_, AppState>) -> Vec<LayerView> {
    bible_app_core::layers::layer_views(&state.layers)
}

#[tauri::command]
fn get_chapter(
    state: State<'_, AppState>,
    translation: String,
    book: String,
    chapter: u16,
    layers: Vec<String>,
) -> Result<ChapterView, String> {
    let corpus = state.corpus()?;
    let active = bible_app_core::layers::activate_for_translation(
        &corpus,
        &translation,
        &state.layers,
        &layers,
        &state.registry,
    )
    .map_err(|e| e.to_string())?;
    bible_app_core::get_chapter(&corpus, &translation, &book, chapter, &active)
        .map_err(|e| e.to_string())
}

/// Locates `corpus.db`.
///
/// Beside the executable for a packaged build, and in the repository's
/// `build/` directory when running from a checkout. The repository path is
/// anchored to this file's location at compile time, not to the working
/// directory — `cargo run` must work from any directory. No network path and
/// no download: requirement §1 is that the reader works with zero network
/// access from first launch.
fn corpus_path(app: &tauri::App) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let packaged = dir.join("corpus.db");
        if packaged.exists() {
            return packaged;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/corpus.db")
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let path = corpus_path(app);
            let corpus = CorpusDb::open(&path).map_err(|e| {
                format!("cannot open corpus at {}: {e}\nrun: cargo run -p build-corpus", path.display())
            })?;
            let layers = bible_app_core::builtin_layers()
                .map_err(|e| format!("bundled annotation layer is defective: {e}"))?;
            let registry = SchemeRegistry::embedded()
                .map_err(|e| format!("embedded versification is defective: {e}"))?;
            app.manage(AppState {
                corpus: Mutex::new(corpus),
                layers,
                registry,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_translations,
            get_toc,
            get_chapter,
            list_layers,
            desktop_env,
            toggle_maximize
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
