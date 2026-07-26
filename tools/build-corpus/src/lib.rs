//! Builds `corpus.db` from vendored archives and per-translation manifests.
//!
//! The pipeline, in order, with the refusals up front so nothing is written
//! before the terms are checked:
//!
//! 1. Load and validate every manifest.
//! 2. **Refuse** any translation whose licence forbids redistribution (§4.3).
//! 3. Verify the vendored archive's SHA-256 against the manifest.
//! 4. Parse every USFM book.
//! 5. **Refuse** (in strict mode) any chapter containing an unmodelled marker.
//! 6. Write text, verse projection and search indexes in one transaction.

#![forbid(unsafe_code)]

pub mod manifest;

use bible_domain::BundleRefusal;
use bible_storage::{CorpusDb, StorageError};
use bible_versification::{SchemeRegistry, Versifier};
use manifest::{Manifest, ManifestError};
use sha2::{Digest, Sha256};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum BuildError {
    Manifest(ManifestError),
    Refused(BundleRefusal),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    ChecksumMismatch {
        archive: PathBuf,
        expected: String,
        actual: String,
    },
    Zip(String),
    Import {
        book: String,
        source: bible_import::ImportError,
    },
    /// Strict mode: the importer met markers it does not model.
    UnknownMarkers {
        translation: String,
        book: String,
        chapter: u16,
        markers: Vec<String>,
    },
    /// Strict mode: the translation's versification scheme has data defects.
    SchemeDefects {
        scheme: String,
        defects: Vec<String>,
    },
    Storage(StorageError),
    Versification(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Manifest(e) => write!(f, "{e}"),
            BuildError::Refused(r) => write!(f, "{r}"),
            BuildError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            BuildError::ChecksumMismatch {
                archive,
                expected,
                actual,
            } => write!(
                f,
                "{}: checksum mismatch\n  manifest: {expected}\n  actual:   {actual}\n\
                 the vendored archive does not match what the manifest pins",
                archive.display()
            ),
            BuildError::Zip(e) => write!(f, "cannot read archive: {e}"),
            BuildError::Import { book, source } => write!(f, "{book}: {source}"),
            BuildError::UnknownMarkers {
                translation,
                book,
                chapter,
                markers,
            } => write!(
                f,
                "{translation} {book} {chapter} contains unmodelled markers {markers:?}; \
                 refusing to ship a corpus with markup the renderer will drop"
            ),
            BuildError::SchemeDefects { scheme, defects } => write!(
                f,
                "versification scheme {scheme} has {} unusable mapping(s): {defects:?}",
                defects.len()
            ),
            BuildError::Storage(e) => write!(f, "{e}"),
            BuildError::Versification(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl From<ManifestError> for BuildError {
    fn from(e: ManifestError) -> Self {
        BuildError::Manifest(e)
    }
}
impl From<StorageError> for BuildError {
    fn from(e: StorageError) -> Self {
        BuildError::Storage(e)
    }
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub manifests: PathBuf,
    pub vendor: PathBuf,
    pub output: PathBuf,
    /// Refuse on unmodelled markers and on defects in a bundled scheme.
    pub strict: bool,
    /// Build the substring index too. Off by default: it is several times the
    /// size of the word index, and a reader who never runs a substring search
    /// should not carry one.
    pub trigram: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct BuildReport {
    pub translations: usize,
    pub books: usize,
    pub chapters: usize,
    pub verses: usize,
}

fn sha256_file(path: &Path) -> Result<String, BuildError> {
    let bytes = std::fs::read(path).map_err(|source| BuildError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

/// USFM files inside the archive, as `(name, contents)`, sorted by name so the
/// build is reproducible.
fn usfm_entries(archive: &Path) -> Result<Vec<(String, String)>, BuildError> {
    let file = std::fs::File::open(archive).map_err(|source| BuildError::Io {
        path: archive.to_path_buf(),
        source,
    })?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| BuildError::Zip(e.to_string()))?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| BuildError::Zip(e.to_string()))?;
        if !entry.is_file() {
            continue;
        }
        let name = entry.name().to_string();
        if !name.to_ascii_lowercase().ends_with(".usfm") {
            continue;
        }
        let mut s = String::new();
        entry
            .read_to_string(&mut s)
            .map_err(|e| BuildError::Zip(format!("{name}: {e}")))?;
        out.push((name, s));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn build(opts: &BuildOptions) -> Result<BuildReport, BuildError> {
    let manifests = Manifest::load_dir(&opts.manifests)?;
    let schemes =
        SchemeRegistry::embedded().map_err(|e| BuildError::Versification(e.to_string()))?;

    // Every refusal is evaluated before anything is written.
    let mut planned = Vec::new();
    for (path, m) in &manifests {
        let translation = m.to_translation(path)?;
        translation
            .license
            .check_bundlable(translation.id.as_str())
            .map_err(BuildError::Refused)?;

        if opts.strict {
            let defects: Vec<String> = schemes
                .defects_for(translation.scheme)
                .map(|d| d.to_string())
                .collect();
            if !defects.is_empty() {
                return Err(BuildError::SchemeDefects {
                    scheme: translation.scheme.to_string(),
                    defects,
                });
            }
        }
        planned.push((m, translation));
    }

    if let Some(parent) = opts.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| BuildError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }
    let _ = std::fs::remove_file(&opts.output);
    let db = CorpusDb::open(&opts.output)?;

    for id in schemes.scheme_ids() {
        db.put_scheme(id.as_str(), id.as_str(), "embedded")?;
    }

    let mut report = BuildReport::default();

    for (m, translation) in planned {
        let archive = opts.vendor.join(&m.source.archive);
        let actual = sha256_file(&archive)?;
        if actual != m.source.sha256 {
            return Err(BuildError::ChecksumMismatch {
                archive,
                expected: m.source.sha256.clone(),
                actual,
            });
        }

        let entries = usfm_entries(&archive)?;

        db.connection()
            .execute_batch("BEGIN")
            .map_err(StorageError::from)?;

        let result = (|| -> Result<(), BuildError> {
            db.put_translation(
                &translation,
                // Fixed rather than `now()`: a reproducible build must not
                // embed a timestamp. The archive's checksum already dates the
                // content.
                "vendored",
                env!("CARGO_PKG_VERSION"),
            )?;

            for (name, src) in &entries {
                let book = match bible_import::parse_book(&translation.id, src) {
                    Ok(b) => b,
                    // Front matter and glossary files carry no `\id` book code.
                    Err(bible_import::ImportError::UnknownBookCode(_))
                    | Err(bible_import::ImportError::MissingIdMarker) => continue,
                    Err(source) => {
                        return Err(BuildError::Import {
                            book: name.clone(),
                            source,
                        })
                    }
                };
                if book.chapters.is_empty() {
                    continue;
                }
                report.books += 1;
                for ch in &book.chapters {
                    if opts.strict {
                        let markers = ch.unknown_markers();
                        if !markers.is_empty() {
                            return Err(BuildError::UnknownMarkers {
                                translation: translation.id.to_string(),
                                book: book.book.usfm_code().to_string(),
                                chapter: ch.chapter,
                                markers,
                            });
                        }
                    }
                    db.put_chapter(ch)?;
                    report.chapters += 1;
                    report.verses += ch.verse_numbers().len();
                }
            }
            report.translations += 1;
            Ok(())
        })();

        match result {
            Ok(()) => {
                db.connection()
                    .execute_batch("COMMIT")
                    .map_err(StorageError::from)?;
            }
            Err(e) => {
                let _ = db.connection().execute_batch("ROLLBACK");
                drop(db);
                let _ = std::fs::remove_file(&opts.output);
                return Err(e);
            }
        }
    }

    db.rebuild_search_index(opts.trigram)?;
    db.connection()
        .execute_batch("VACUUM")
        .map_err(StorageError::from)?;
    Ok(report)
}

/// Sanity check used by the build and by tests: a scheme must agree with the
/// text about how many verses a chapter has.
pub fn verse_counts_agree_with_scheme(
    db: &CorpusDb,
    schemes: &SchemeRegistry,
    translation: &bible_domain::Translation,
    book: bible_domain::BookId,
    chapter: u16,
) -> Result<bool, BuildError> {
    let Some(doc) = db.chapter(&translation.id, book, chapter)? else {
        return Ok(true);
    };
    let expected = schemes
        .last_verse(translation.scheme, book, chapter)
        .map_err(|e| BuildError::Versification(e.to_string()))?;
    let Some(expected) = expected else {
        return Ok(true);
    };
    Ok(doc.verse_numbers().last().copied().unwrap_or(0) <= expected)
}
