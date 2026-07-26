//! Prints one chapter as the JSON the UI receives.
//!
//! The seam between the Rust core and the JavaScript renderer, made
//! inspectable. Useful for eyeballing the boundary contract and for piping
//! into the renderer's tests without a window:
//!
//! ```text
//! cargo run -p bible-app-core --example dump_chapter -- build/corpus.db eng-web PSA 23
//! ```
//!
//! A trailing `--layers a,b` switches annotation layers on, which is how a
//! highlight that does not appear on screen gets diagnosed without a window:
//!
//! ```text
//! cargo run -p bible-app-core --example dump_chapter -- \
//!     build/corpus.db eng-web 2SA 7 --layers messianic-prophecy
//! ```

use bible_app_core::{activate_for_translation, builtin_layers, get_chapter, ActiveLayers};
use bible_storage::CorpusDb;
use bible_versification::SchemeRegistry;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (positional, layer_ids): (&[String], Vec<String>) = match args
        .iter()
        .position(|a| a == "--layers")
    {
        Some(i) => (
            &args[..i],
            args.get(i + 1)
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        ),
        None => (&args[..], Vec::new()),
    };
    let [db_path, translation, book, chapter] = positional else {
        eprintln!(
            "usage: dump_chapter <corpus.db> <translation> <BOOK> <chapter> [--layers a,b]"
        );
        return ExitCode::FAILURE;
    };
    let Ok(chapter) = chapter.parse::<u16>() else {
        eprintln!("chapter must be a number");
        return ExitCode::FAILURE;
    };

    let db = match CorpusDb::open(db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open {db_path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let active = if layer_ids.is_empty() {
        ActiveLayers::default()
    } else {
        let loaded = match builtin_layers() {
            Ok(l) => l,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        let registry = match SchemeRegistry::embedded() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        match activate_for_translation(&db, translation, &loaded, &layer_ids, &registry) {
            Ok(a) => {
                eprintln!("active: {} annotation(s)", a.annotations.len());
                a
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    };

    match get_chapter(&db, translation, book, chapter, &active) {
        Ok(view) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&view).expect("view serialises")
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
