//! Versioned, forward-only schema migrations.
//!
//! §4.6: there is no server to repair bad data, so migrations are embedded in
//! the binary, applied in order inside a transaction, and checksummed. The
//! checksum catches the classic mistake of editing an already-released
//! migration instead of adding a new one — which would leave two users with
//! the same recorded schema version and different actual schemas.

use rusqlite::{Connection, OptionalExtension};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Which database a migration set belongs to (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Database {
    Corpus,
    User,
}

const CORPUS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("migrations/corpus/001_init.sql"),
    },
    Migration {
        version: 2,
        name: "fts",
        sql: include_str!("migrations/corpus/002_fts.sql"),
    },
];

const USER: &[Migration] = &[Migration {
    version: 1,
    name: "init",
    sql: include_str!("migrations/user/001_init.sql"),
}];

impl Database {
    pub fn migrations(self) -> &'static [Migration] {
        match self {
            Database::Corpus => CORPUS,
            Database::User => USER,
        }
    }

    /// The version a freshly built database ends at.
    pub fn latest_version(self) -> u32 {
        self.migrations().last().map(|m| m.version).unwrap_or(0)
    }
}

#[derive(Debug)]
pub enum MigrationError {
    Sqlite(rusqlite::Error),
    /// An applied migration's text no longer matches what was recorded. Either
    /// a released migration was edited or the database is from another product.
    ChecksumMismatch {
        version: u32,
        recorded: String,
        computed: String,
    },
    /// The database is newer than this build knows about.
    FromTheFuture {
        database_version: u32,
        binary_version: u32,
    },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationError::Sqlite(e) => write!(f, "sqlite error: {e}"),
            MigrationError::ChecksumMismatch { version, recorded, computed } => write!(
                f,
                "migration {version} was applied as {recorded} but is now {computed}; \
                 a released migration must never be edited — add a new one"
            ),
            MigrationError::FromTheFuture { database_version, binary_version } => write!(
                f,
                "database is at schema version {database_version} but this build only knows {binary_version}; \
                 refusing to downgrade"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(e: rusqlite::Error) -> Self {
        MigrationError::Sqlite(e)
    }
}

/// FNV-1a, rendered as 16 hex digits.
///
/// Not a cryptographic hash and not required to be: this detects accidental
/// edits to migration files, a threat model with no adversary. Using SHA-256
/// here would mean adding a dependency to defend against nobody.
fn checksum(s: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}

fn ensure_bookkeeping(conn: &Connection) -> Result<(), MigrationError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT NOT NULL,
             checksum   TEXT NOT NULL,
             applied_at TEXT NOT NULL
         ) STRICT;",
    )?;
    Ok(())
}

/// Current schema version, or 0 for an empty database.
pub fn current_version(conn: &Connection) -> Result<u32, MigrationError> {
    ensure_bookkeeping(conn)?;
    let v: Option<u32> = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    Ok(v.unwrap_or(0))
}

/// Applies every pending migration for `db`, in order, each in its own
/// transaction. Returns the versions applied.
pub fn migrate(conn: &Connection, db: Database) -> Result<Vec<u32>, MigrationError> {
    ensure_bookkeeping(conn)?;

    // Verify what is already applied before applying anything more.
    for m in db.migrations() {
        let recorded: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [m.version],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(recorded) = recorded {
            let computed = checksum(m.sql);
            if recorded != computed {
                return Err(MigrationError::ChecksumMismatch {
                    version: m.version,
                    recorded,
                    computed,
                });
            }
        }
    }

    let current = current_version(conn)?;
    let latest = db.latest_version();
    if current > latest {
        return Err(MigrationError::FromTheFuture {
            database_version: current,
            binary_version: latest,
        });
    }

    let mut applied = Vec::new();
    for m in db.migrations().iter().filter(|m| m.version > current) {
        conn.execute_batch("BEGIN")?;
        let result = (|| -> Result<(), MigrationError> {
            conn.execute_batch(m.sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, datetime('now'))",
                rusqlite::params![m.version, m.name, checksum(m.sql)],
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                applied.push(m.version);
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn migration_versions_are_sequential_and_unique() {
        for db in [Database::Corpus, Database::User] {
            let versions: Vec<u32> = db.migrations().iter().map(|m| m.version).collect();
            let mut expected: Vec<u32> = (1..=versions.len() as u32).collect();
            expected.sort_unstable();
            assert_eq!(
                versions, expected,
                "{db:?} migrations must be 1..n with no gaps"
            );
        }
    }

    #[test]
    fn a_fresh_corpus_database_reaches_the_latest_version() {
        let c = mem();
        let applied = migrate(&c, Database::Corpus).unwrap();
        assert_eq!(applied, vec![1, 2]);
        assert_eq!(
            current_version(&c).unwrap(),
            Database::Corpus.latest_version()
        );
    }

    #[test]
    fn a_fresh_user_database_reaches_the_latest_version() {
        let c = mem();
        assert_eq!(migrate(&c, Database::User).unwrap(), vec![1]);
        assert_eq!(current_version(&c).unwrap(), 1);
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let c = mem();
        migrate(&c, Database::Corpus).unwrap();
        assert_eq!(migrate(&c, Database::Corpus).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn editing_a_released_migration_is_detected() {
        let c = mem();
        migrate(&c, Database::Corpus).unwrap();
        // Simulate someone editing 001 instead of adding 003.
        c.execute(
            "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 1",
            [],
        )
        .unwrap();
        assert!(matches!(
            migrate(&c, Database::Corpus),
            Err(MigrationError::ChecksumMismatch { version: 1, .. })
        ));
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused_rather_than_downgraded() {
        let c = mem();
        migrate(&c, Database::Corpus).unwrap();
        c.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at)
             VALUES (99, 'future', 'x', datetime('now'))",
            [],
        )
        .unwrap();
        assert!(matches!(
            migrate(&c, Database::Corpus),
            Err(MigrationError::FromTheFuture {
                database_version: 99,
                ..
            })
        ));
    }

    #[test]
    fn checksums_differ_for_different_text() {
        assert_ne!(checksum("a"), checksum("b"));
        assert_eq!(checksum("a"), checksum("a"));
        assert_eq!(checksum("").len(), 16);
    }

    /// The §4.6 requirement: a database created at v1 and walked forward must
    /// end up identical to one created fresh at HEAD.
    #[test]
    fn stepwise_upgrade_matches_a_fresh_install() {
        fn schema_of(c: &Connection) -> Vec<(String, String)> {
            let mut stmt = c
                .prepare(
                    "SELECT type || ' ' || name, COALESCE(sql, '')
                     FROM sqlite_master
                     WHERE name NOT LIKE 'sqlite_%' AND name != 'schema_migrations'
                     ORDER BY 1",
                )
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows
        }

        for db in [Database::Corpus, Database::User] {
            // Stepwise: apply one migration at a time, as an upgrading user does.
            let stepwise = mem();
            ensure_bookkeeping(&stepwise).unwrap();
            for m in db.migrations() {
                stepwise.execute_batch(m.sql).unwrap();
                stepwise
                    .execute(
                        "INSERT INTO schema_migrations (version, name, checksum, applied_at)
                         VALUES (?1, ?2, ?3, datetime('now'))",
                        rusqlite::params![m.version, m.name, checksum(m.sql)],
                    )
                    .unwrap();
            }

            // Fresh: the normal path.
            let fresh = mem();
            migrate(&fresh, db).unwrap();

            assert_eq!(
                schema_of(&stepwise),
                schema_of(&fresh),
                "{db:?}: stepwise upgrade diverged from a fresh install"
            );
            assert_eq!(
                current_version(&stepwise).unwrap(),
                current_version(&fresh).unwrap()
            );
        }
    }

    #[test]
    fn fts5_is_available_in_the_bundled_sqlite() {
        // The whole search design (§4.4) rests on this. If the bundled build
        // ever loses FTS5, fail here rather than at a user's first search.
        let c = mem();
        migrate(&c, Database::Corpus).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'verse_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n, 1,
            "verse_fts must exist; FTS5 missing from the bundled SQLite"
        );
    }

    #[test]
    fn the_licence_gate_is_enforced_by_the_schema_too() {
        let c = mem();
        migrate(&c, Database::Corpus).unwrap();
        c.execute_batch(
            "INSERT INTO versification_scheme (id, name, source_sha256) VALUES ('org','Original','x');",
        )
        .unwrap();
        // Empty attribution must be rejected by the CHECK constraint, not only
        // by build-corpus.
        let r = c.execute(
            "INSERT INTO translation (id, abbrev, name, language_tag, text_direction, scheme_id,
                 canon, license_spdx, redistributable, commercial_use, attribution,
                 source_url, source_sha256, imported_at, importer_version)
             VALUES ('x','X','X','en','ltr','org','protestant',NULL,1,1,'   ','u','s','now','1')",
            [],
        );
        assert!(
            r.is_err(),
            "blank attribution must violate the CHECK constraint"
        );
    }
}
