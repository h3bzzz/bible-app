//! Canonical book identity.
//!
//! `BookId` is a closed enum, not a string. An unrecognised book code is a
//! parse error at the boundary rather than a silently empty result set three
//! layers down. The full Paratext/USFM book set is represented — 98 books,
//! including Septuagint alternates and the wider Eastern canons — because the
//! versification data in `data/versification/` references all of them and a
//! partial enum would fail to load it.

use core::fmt;

/// Defines the book table exactly once: variant, USFM code, English name.
///
/// Every other property (ordering, canon membership, parsing) is derived from
/// this single table, so adding a book cannot leave a parallel `match` stale.
macro_rules! book_table {
    ($( $variant:ident => $code:literal, $name:literal ; )*) => {
        /// A book of one of the biblical canons, identified by its USFM code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum BookId {
            $( $variant, )*
        }

        impl BookId {
            /// Every book, in canonical order.
            pub const ALL: &'static [BookId] = &[ $( BookId::$variant, )* ];

            /// The three-character USFM identifier, e.g. `"GEN"`, `"1SA"`.
            pub const fn usfm_code(self) -> &'static str {
                match self { $( BookId::$variant => $code, )* }
            }

            /// English display name.
            pub const fn english_name(self) -> &'static str {
                match self { $( BookId::$variant => $name, )* }
            }

            /// Parse a USFM code. Case-sensitive: USFM codes are uppercase and
            /// accepting lowercase would mask importer bugs.
            pub fn from_usfm_code(code: &str) -> Option<BookId> {
                match code {
                    $( $code => Some(BookId::$variant), )*
                    _ => None,
                }
            }
        }
    };
}

book_table! {
    // ── Hebrew Bible / Old Testament (39) ──────────────────────────────
    Gen => "GEN", "Genesis";
    Exo => "EXO", "Exodus";
    Lev => "LEV", "Leviticus";
    Num => "NUM", "Numbers";
    Deu => "DEU", "Deuteronomy";
    Jos => "JOS", "Joshua";
    Jdg => "JDG", "Judges";
    Rut => "RUT", "Ruth";
    Sa1 => "1SA", "1 Samuel";
    Sa2 => "2SA", "2 Samuel";
    Ki1 => "1KI", "1 Kings";
    Ki2 => "2KI", "2 Kings";
    Ch1 => "1CH", "1 Chronicles";
    Ch2 => "2CH", "2 Chronicles";
    Ezr => "EZR", "Ezra";
    Neh => "NEH", "Nehemiah";
    Est => "EST", "Esther";
    Job => "JOB", "Job";
    Psa => "PSA", "Psalms";
    Pro => "PRO", "Proverbs";
    Ecc => "ECC", "Ecclesiastes";
    Sng => "SNG", "Song of Solomon";
    Isa => "ISA", "Isaiah";
    Jer => "JER", "Jeremiah";
    Lam => "LAM", "Lamentations";
    Ezk => "EZK", "Ezekiel";
    Dan => "DAN", "Daniel";
    Hos => "HOS", "Hosea";
    Jol => "JOL", "Joel";
    Amo => "AMO", "Amos";
    Oba => "OBA", "Obadiah";
    Jon => "JON", "Jonah";
    Mic => "MIC", "Micah";
    Nam => "NAM", "Nahum";
    Hab => "HAB", "Habakkuk";
    Zep => "ZEP", "Zephaniah";
    Hag => "HAG", "Haggai";
    Zec => "ZEC", "Zechariah";
    Mal => "MAL", "Malachi";

    // ── New Testament (27) ─────────────────────────────────────────────
    Mat => "MAT", "Matthew";
    Mrk => "MRK", "Mark";
    Luk => "LUK", "Luke";
    Jhn => "JHN", "John";
    Act => "ACT", "Acts";
    Rom => "ROM", "Romans";
    Co1 => "1CO", "1 Corinthians";
    Co2 => "2CO", "2 Corinthians";
    Gal => "GAL", "Galatians";
    Eph => "EPH", "Ephesians";
    Php => "PHP", "Philippians";
    Col => "COL", "Colossians";
    Th1 => "1TH", "1 Thessalonians";
    Th2 => "2TH", "2 Thessalonians";
    Ti1 => "1TI", "1 Timothy";
    Ti2 => "2TI", "2 Timothy";
    Tit => "TIT", "Titus";
    Phm => "PHM", "Philemon";
    Heb => "HEB", "Hebrews";
    Jas => "JAS", "James";
    Pe1 => "1PE", "1 Peter";
    Pe2 => "2PE", "2 Peter";
    Jn1 => "1JN", "1 John";
    Jn2 => "2JN", "2 John";
    Jn3 => "3JN", "3 John";
    Jud => "JUD", "Jude";
    Rev => "REV", "Revelation";

    // ── Deuterocanon and wider canons (32) ─────────────────────────────
    Tob => "TOB", "Tobit";
    Jdt => "JDT", "Judith";
    Esg => "ESG", "Esther (Greek)";
    Wis => "WIS", "Wisdom";
    Sir => "SIR", "Sirach";
    Bar => "BAR", "Baruch";
    Lje => "LJE", "Letter of Jeremiah";
    S3y => "S3Y", "Song of the Three Young Men";
    Sus => "SUS", "Susanna";
    Bel => "BEL", "Bel and the Dragon";
    Ma1 => "1MA", "1 Maccabees";
    Ma2 => "2MA", "2 Maccabees";
    Ma3 => "3MA", "3 Maccabees";
    Ma4 => "4MA", "4 Maccabees";
    Es1 => "1ES", "1 Esdras";
    Es2 => "2ES", "2 Esdras";
    Man => "MAN", "Prayer of Manasseh";
    Ps2 => "PS2", "Psalm 151";
    Pss => "PSS", "Psalms of Solomon";
    Oda => "ODA", "Odes";
    Eza => "EZA", "Apocalypse of Ezra";
    Jub => "JUB", "Jubilees";
    Eno => "ENO", "Enoch";
    Ez6 => "6EZ", "6 Ezra";
    Lao => "LAO", "Laodiceans";
    Dag => "DAG", "Daniel (Greek)";
    Jsa => "JSA", "Joshua (alternate)";
    Jdb => "JDB", "Judges (alternate)";
    Tbs => "TBS", "Tobit (Sinaiticus)";
    Sst => "SST", "Susanna (Theodotion)";
    Dnt => "DNT", "Daniel (Theodotion)";
    Blt => "BLT", "Bel and the Dragon (Theodotion)";
}

impl BookId {
    /// Canonical sort position. Derived from `ALL`, so it can never disagree
    /// with the table above.
    pub fn order(self) -> usize {
        self as usize
    }

    /// True for the 39 books of the Hebrew Bible / Protestant Old Testament.
    pub fn is_old_testament(self) -> bool {
        self.order() <= BookId::Mal.order()
    }

    /// True for the 27 books of the New Testament.
    pub fn is_new_testament(self) -> bool {
        self.order() >= BookId::Mat.order() && self.order() <= BookId::Rev.order()
    }
}

impl fmt::Display for BookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.usfm_code())
    }
}

/// A canon: which books a tradition receives, and in what order.
///
/// `Ethiopian` is deliberately absent. Representing it correctly requires book
/// divisions this project has not yet sourced (the Ethiopian Orthodox canon
/// splits and combines books in ways the Paratext set does not model cleanly),
/// and shipping a wrong canon is worse than shipping none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Canon {
    Protestant,
    Catholic,
    EasternOrthodox,
}

const PROTESTANT_EXTRA: &[BookId] = &[];

/// Trent's deuterocanon: Tobit, Judith, Greek Esther, Wisdom, Sirach, Baruch
/// (with the Letter of Jeremiah as its sixth chapter), 1–2 Maccabees, and the
/// Greek additions to Daniel.
const CATHOLIC_EXTRA: &[BookId] = &[
    BookId::Tob,
    BookId::Jdt,
    BookId::Esg,
    BookId::Wis,
    BookId::Sir,
    BookId::Bar,
    BookId::Lje,
    BookId::S3y,
    BookId::Sus,
    BookId::Bel,
    BookId::Ma1,
    BookId::Ma2,
];

/// Catholic deuterocanon plus the books received or appended in the Greek and
/// Slavonic Orthodox traditions.
const ORTHODOX_EXTRA: &[BookId] = &[
    BookId::Tob,
    BookId::Jdt,
    BookId::Esg,
    BookId::Wis,
    BookId::Sir,
    BookId::Bar,
    BookId::Lje,
    BookId::S3y,
    BookId::Sus,
    BookId::Bel,
    BookId::Ma1,
    BookId::Ma2,
    BookId::Ma3,
    BookId::Ma4,
    BookId::Es1,
    BookId::Es2,
    BookId::Man,
    BookId::Ps2,
    BookId::Oda,
];

impl Canon {
    /// Books received by this canon, in canonical order.
    pub fn books(self) -> Vec<BookId> {
        let extra = match self {
            Canon::Protestant => PROTESTANT_EXTRA,
            Canon::Catholic => CATHOLIC_EXTRA,
            Canon::EasternOrthodox => ORTHODOX_EXTRA,
        };
        let mut books: Vec<BookId> = BookId::ALL
            .iter()
            .copied()
            .filter(|b| b.is_old_testament() || b.is_new_testament() || extra.contains(b))
            .collect();
        books.sort_by_key(|b| b.order());
        books
    }

    pub fn contains(self, book: BookId) -> bool {
        if book.is_old_testament() || book.is_new_testament() {
            return true;
        }
        match self {
            Canon::Protestant => PROTESTANT_EXTRA.contains(&book),
            Canon::Catholic => CATHOLIC_EXTRA.contains(&book),
            Canon::EasternOrthodox => ORTHODOX_EXTRA.contains(&book),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_the_full_paratext_set() {
        // The versification data in data/versification/ references 98 distinct
        // book codes. A short table would fail to load it.
        assert_eq!(BookId::ALL.len(), 98);
    }

    #[test]
    fn codes_round_trip() {
        for &b in BookId::ALL {
            assert_eq!(BookId::from_usfm_code(b.usfm_code()), Some(b), "{b}");
        }
    }

    #[test]
    fn codes_are_unique_and_three_chars() {
        let mut seen = std::collections::HashSet::new();
        for &b in BookId::ALL {
            assert_eq!(b.usfm_code().len(), 3, "{b} code is not 3 chars");
            assert!(
                seen.insert(b.usfm_code()),
                "duplicate code {}",
                b.usfm_code()
            );
        }
    }

    #[test]
    fn parsing_is_case_sensitive() {
        assert_eq!(BookId::from_usfm_code("GEN"), Some(BookId::Gen));
        assert_eq!(BookId::from_usfm_code("gen"), None);
        assert_eq!(BookId::from_usfm_code("XYZ"), None);
    }

    #[test]
    fn testament_partitions_are_exact() {
        assert_eq!(
            BookId::ALL.iter().filter(|b| b.is_old_testament()).count(),
            39
        );
        assert_eq!(
            BookId::ALL.iter().filter(|b| b.is_new_testament()).count(),
            27
        );
        assert!(BookId::Gen.is_old_testament());
        assert!(BookId::Mal.is_old_testament());
        assert!(!BookId::Mat.is_old_testament());
        assert!(BookId::Rev.is_new_testament());
        assert!(!BookId::Tob.is_new_testament());
    }

    #[test]
    fn canons_have_the_expected_sizes() {
        assert_eq!(Canon::Protestant.books().len(), 66);
        assert_eq!(Canon::Catholic.books().len(), 66 + 12);
        assert!(Canon::Catholic.contains(BookId::Tob));
        assert!(!Canon::Protestant.contains(BookId::Tob));
        assert!(Canon::EasternOrthodox.contains(BookId::Ma3));
        assert!(!Canon::Catholic.contains(BookId::Ma3));
        // Every canon receives every protocanonical book.
        for &b in BookId::ALL {
            if b.is_old_testament() || b.is_new_testament() {
                assert!(Canon::Protestant.contains(b), "{b}");
            }
        }
    }

    #[test]
    fn canonical_order_is_genesis_first_revelation_after_malachi() {
        assert!(BookId::Gen.order() < BookId::Mal.order());
        assert!(BookId::Mal.order() < BookId::Mat.order());
        assert!(BookId::Mat.order() < BookId::Rev.order());
    }
}
