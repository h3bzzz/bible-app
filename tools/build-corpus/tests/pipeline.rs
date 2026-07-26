//! End-to-end tests for the corpus build, including the refusals.
//!
//! A gate nobody tests is a gate that has stopped working.

use build_corpus::{build, BuildError, BuildOptions};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Builds a zip of the real chapter fixtures, returning its SHA-256.
fn make_archive(path: &Path) -> String {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for name in ["psa023-eng-web.usfm", "jhn001-eng-web.usfm"] {
        let src = std::fs::read_to_string(repo_root().join("tests/fixtures/usfm").join(name))
            .expect("fixture");
        zip.start_file(name, opts).unwrap();
        zip.write_all(src.as_bytes()).unwrap();
    }
    // A file with no book code, as real archives contain (front matter).
    zip.start_file("00-FRT.usfm", opts).unwrap();
    zip.write_all(b"\\id FRT Front matter\n\\h Front\n")
        .unwrap();
    zip.finish().unwrap();

    let mut h = Sha256::new();
    h.update(std::fs::read(path).unwrap());
    format!("{:x}", h.finalize())
}

fn write_manifest(dir: &Path, sha: &str, archive: &str, redistributable: bool) {
    let toml = format!(
        r#"
id = "eng-web"
abbrev = "WEB"
name = "World English Bible"
language = "en"
direction = "ltr"
scheme = "eng"
canon = "protestant"

[license]
redistributable = {redistributable}
commercial_use = true
attribution = "World English Bible, public domain."

[source]
url = "https://ebible.org/Scriptures/eng-web_usfm.zip"
sha256 = "{sha}"
publisher = "eBible.org"
archive = "{archive}"
"#
    );
    std::fs::write(dir.join("eng-web.toml"), toml).unwrap();
}

struct Fixture {
    _dir: tempfile::TempDir,
    opts: BuildOptions,
    manifests: PathBuf,
    vendor: PathBuf,
    sha: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("manifests");
    let vendor = dir.path().join("vendor");
    std::fs::create_dir_all(&manifests).unwrap();
    std::fs::create_dir_all(&vendor).unwrap();
    let sha = make_archive(&vendor.join("web.zip"));
    write_manifest(&manifests, &sha, "web.zip", true);
    let opts = BuildOptions {
        manifests: manifests.clone(),
        vendor: vendor.clone(),
        output: dir.path().join("corpus.db"),
        strict: true,
        trigram: true,
    };
    Fixture {
        _dir: dir,
        opts,
        manifests,
        vendor,
        sha,
    }
}

#[test]
fn builds_a_corpus_from_real_fixtures_in_strict_mode() {
    let f = fixture();
    let report = build(&f.opts).expect("build must succeed");
    assert_eq!(report.translations, 1);
    assert_eq!(report.books, 2, "front matter must be skipped, not counted");
    assert_eq!(report.chapters, 2);
    // Psalm 23 has 6 verses, John 1 has 51.
    assert_eq!(report.verses, 57, "{report:?}");
    assert!(f.opts.output.exists());
}

#[test]
fn the_built_corpus_is_readable_through_the_application_layer() {
    let f = fixture();
    build(&f.opts).unwrap();

    let db = bible_storage::CorpusDb::open(&f.opts.output).unwrap();
    let view = bible_app_core::get_chapter(
        &db,
        "eng-web",
        "PSA",
        23,
        &bible_app_core::ActiveLayers::default(),
    )
    .expect("chapter must load");

    assert_eq!(view.title, "Psalms 23");
    assert_eq!(view.translation.abbrev, "WEB");
    assert_eq!(view.translation.direction, "ltr");
    assert_eq!(view.translation.language, "en");
    assert!(view.translation.attribution.contains("public domain"));
    assert_eq!(view.verses, vec![1, 2, 3, 4, 5, 6]);
    assert!(!view.blocks.is_empty());
}

#[test]
fn a_non_redistributable_translation_is_refused_and_nothing_is_written() {
    let f = fixture();
    write_manifest(&f.manifests, &f.sha, "web.zip", false);

    let e = build(&f.opts).expect_err("must refuse");
    assert!(matches!(e, BuildError::Refused(_)), "{e:?}");
    assert!(e.to_string().contains("forbids redistribution"), "{e}");
    assert!(
        !f.opts.output.exists(),
        "no database may be written when a licence is refused"
    );
}

#[test]
fn the_committed_niv_fixture_is_refused() {
    // The real fixture, in its real location, so the gate is exercised against
    // the file a contributor would actually add.
    let dir = tempfile::tempdir().unwrap();
    let opts = BuildOptions {
        manifests: repo_root().join("tests/fixtures/manifests"),
        vendor: repo_root().join("data/vendor"),
        output: dir.path().join("corpus.db"),
        strict: true,
        trigram: false,
    };
    let e = build(&opts).expect_err("the NIV manifest must be refused");
    assert!(matches!(e, BuildError::Refused(_)), "{e:?}");
    assert!(e.to_string().contains("eng-niv"), "{e}");
    assert!(!opts.output.exists());
}

#[test]
fn a_tampered_archive_is_detected_before_anything_is_imported() {
    let f = fixture();
    // Same manifest, different bytes.
    std::fs::write(f.vendor.join("web.zip"), b"not a zip at all").unwrap();

    let e = build(&f.opts).expect_err("checksum must be verified");
    assert!(matches!(e, BuildError::ChecksumMismatch { .. }), "{e:?}");
    assert!(
        e.to_string()
            .contains("does not match what the manifest pins"),
        "{e}"
    );
}

#[test]
fn strict_mode_refuses_a_corpus_containing_unmodelled_markers() {
    let f = fixture();
    // Re-make the archive with a marker the importer does not model.
    let path = f.vendor.join("web.zip");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zip.start_file("gen.usfm", opts).unwrap();
        zip.write_all(b"\\id GEN Test\n\\c 1\n\\p \\v 1 In the \\zsparkle beginning\\zsparkle*.\n")
            .unwrap();
        zip.finish().unwrap();
    }
    let mut h = Sha256::new();
    h.update(std::fs::read(&path).unwrap());
    let sha = format!("{:x}", h.finalize());
    write_manifest(&f.manifests, &sha, "web.zip", true);

    let e = build(&f.opts).expect_err("strict mode must refuse unknown markers");
    assert!(matches!(e, BuildError::UnknownMarkers { .. }), "{e:?}");
    assert!(e.to_string().contains("zsparkle"), "{e}");
    assert!(
        !f.opts.output.exists(),
        "a refused build must leave no artefact"
    );
}

#[test]
fn a_non_strict_build_accepts_unmodelled_markers_but_preserves_them() {
    let f = fixture();
    let path = f.vendor.join("web.zip");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zip.start_file("gen.usfm", opts).unwrap();
        zip.write_all(b"\\id GEN Test\n\\c 1\n\\p \\v 1 In the \\zsparkle beginning\\zsparkle*.\n")
            .unwrap();
        zip.finish().unwrap();
    }
    let mut h = Sha256::new();
    h.update(std::fs::read(&path).unwrap());
    write_manifest(
        &f.manifests,
        &format!("{:x}", h.finalize()),
        "web.zip",
        true,
    );

    let mut opts = f.opts.clone();
    opts.strict = false;
    build(&opts).expect("a non-strict build must succeed");

    let db = bible_storage::CorpusDb::open(&opts.output).unwrap();
    let tid = bible_domain::TranslationId::new("eng-web").unwrap();
    let doc = db
        .chapter(&tid, bible_domain::BookId::Gen, 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        doc.unknown_markers(),
        vec!["\\zsparkle"],
        "the marker must be preserved in the document, not dropped"
    );
    assert_eq!(
        doc.verse_plain_text(1).as_deref(),
        Some("In the beginning.")
    );
}

#[test]
fn rebuilding_over_an_existing_database_produces_the_same_result() {
    // Reproducibility: the build embeds no timestamp, so two builds of the
    // same inputs must agree byte for byte.
    let f = fixture();
    build(&f.opts).unwrap();
    let first = std::fs::read(&f.opts.output).unwrap();
    build(&f.opts).unwrap();
    let second = std::fs::read(&f.opts.output).unwrap();
    assert_eq!(first, second, "the build is not reproducible");
}
