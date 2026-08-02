//! The application shell — shared by every platform.
//!
//! This is the composition root (§5): it opens the database, wires the use
//! cases, and registers the command surface. There is no dependency injection
//! layer, because there is nothing here for one to do — the dependencies are
//! wired explicitly, in `run`, in about twenty lines.
//!
//! There is deliberately no logic below. Everything a command does lives in
//! `bible-app-core`, where it is tested without a window.
//!
//! # Why a library
//!
//! Desktop launches an executable; mobile loads a shared library. The two
//! share every command and the whole wiring, so that all lives here, behind
//! `run`, and the two thin entry points — the `main` binary and the
//! `#[mobile_entry_point]` — both call it. The `crate-type` in `Cargo.toml`
//! (`staticlib`, `cdylib`, `rlib`) is what lets Android and iOS link against
//! this.

use bible_app_core::view::LayerView;
use bible_app_core::{ChapterView, LoadedLayer, TocView, TranslationView};
use bible_storage::CorpusDb;
use bible_versification::SchemeRegistry;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

/// Everything the commands need. Built once in `run`.
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

/// Whether this build runs on a desktop, so the UI can decide whether to draw
/// its own window chrome. On a phone the OS owns the title bar and there is no
/// minimize, maximize, or close — the whole `win-controls` group is a
/// desktop-only affordance, and `desktop_env` / `toggle_maximize` below do not
/// even exist in a mobile build.
#[tauri::command]
fn is_desktop() -> bool {
    cfg!(desktop)
}

/// The desktop environment, so the UI can adapt its window controls.
/// Hyprland has no minimize concept, so the minimize button would be a lie
/// there; the UI hides it.
#[cfg(desktop)]
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
#[cfg(desktop)]
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

/// Locates a `corpus.db` the SQLite driver can open.
///
/// The requirement (§1) is that the reader works with zero network access from
/// first launch, so the corpus always ships *inside* the app. Where it can be
/// opened from differs by platform:
///
/// * **Desktop** — beside the executable in a packaged build, and in the
///   repository's `build/` directory when running from a checkout. The
///   checkout path is anchored to this file at compile time, not to the
///   working directory, so `cargo run` works from anywhere.
///
/// * **Mobile** — the bundled copy lives in the app's read-only resource area.
///   On Android that area is the APK asset bundle, which is not a real
///   filesystem path `rusqlite` can hand to `sqlite3_open`; on iOS it is real
///   but read-only. In both cases we copy the corpus once into the app's
///   private data directory and open it from there. The copy is ~159 MB and
///   happens on first launch only — every launch after finds the file already
///   present.
fn corpus_path(app: &tauri::App) -> Result<PathBuf, String> {
    #[cfg(mobile)]
    {
        return mobile_corpus_path(app);
    }
    #[cfg(desktop)]
    {
        if let Ok(dir) = app.path().resource_dir() {
            let packaged = dir.join("corpus.db");
            if packaged.exists() {
                return Ok(packaged);
            }
        }
        Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/corpus.db"))
    }
}

/// Materializes the corpus in a writable directory on a phone and returns its
/// path.
///
/// A writable path is not optional: `CorpusDb::open` switches the database to
/// WAL and runs migrations, both of which write. On both mobile platforms the
/// bundled copy is read-only (an iOS bundle resource) or not a real file at
/// all (an Android APK asset), so the corpus is staged into the app's private
/// data directory once, and opened from there every launch after.
///
/// The two platforms stage it differently because an Android asset cannot be
/// read through `std::fs` — it lives inside the APK zip and is reachable only
/// through the Java `AssetManager`. So on Android the copy is done in Kotlin
/// (`MainActivity.stageCorpus`) before the Rust runtime starts, and here we
/// only open the result. On iOS `resource_dir` is a real read-only path, so
/// the copy is done here.
#[cfg(mobile)]
fn mobile_corpus_path(app: &tauri::App) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("no app data directory: {e}"))?;
    let target = data_dir.join("corpus.db");

    #[cfg(target_os = "android")]
    {
        // Staged by MainActivity.stageCorpus before this runs. If it is
        // missing, that Kotlin step failed or was removed — say so plainly
        // rather than fall through to a confusing SQLite error.
        if !target.exists() {
            return Err(format!(
                "corpus not staged at {} — MainActivity.stageCorpus must copy \
                 it out of the APK assets before startup",
                target.display()
            ));
        }
        Ok(target)
    }
    #[cfg(target_os = "ios")]
    {
        if !target.exists() {
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("cannot create {}: {e}", data_dir.display()))?;
            let resource = app
                .path()
                .resource_dir()
                .map_err(|e| format!("no resource directory: {e}"))?
                .join("corpus.db");
            std::fs::copy(&resource, &target).map_err(|e| {
                format!(
                    "cannot stage corpus from {} to {}: {e}",
                    resource.display(),
                    target.display()
                )
            })?;
        }
        Ok(target)
    }
}

/// The one entry point every platform funnels through. On mobile the
/// `#[mobile_entry_point]` attribute below makes this the symbol the OS calls;
/// on desktop `main` calls it directly.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().setup(|app| {
        let path = corpus_path(app)?;
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
    });

    // The command surface splits by platform: the window-management commands
    // only compile on desktop, so they can only be registered there. Every
    // reading command, plus `is_desktop`, is shared.
    #[cfg(desktop)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        list_translations,
        get_toc,
        get_chapter,
        list_layers,
        is_desktop,
        desktop_env,
        toggle_maximize
    ]);
    #[cfg(mobile)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        list_translations,
        get_toc,
        get_chapter,
        list_layers,
        is_desktop
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
