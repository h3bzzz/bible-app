//! Translations: a body of text in a language, using one versification scheme,
//! under one licence.

use crate::book::Canon;
use crate::provenance::License;
use crate::reference::SchemeId;
use core::fmt;

/// Stable translation identifier, e.g. `eng-web`, `eng-kjv`.
///
/// Follows the eBible/ISO-639-3 convention of `<lang>-<abbrev>`. Owned rather
/// than inline because it appears in manifests and URLs and has no useful
/// length bound.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranslationId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationIdError {
    Empty,
    /// Only lowercase ASCII letters, digits and `-` are permitted, so an id is
    /// always safe as a filename, a URL segment and a SQL literal.
    IllegalCharacter(char),
}

impl fmt::Display for TranslationIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranslationIdError::Empty => write!(f, "translation id is empty"),
            TranslationIdError::IllegalCharacter(c) => {
                write!(f, "translation id contains illegal character {c:?}")
            }
        }
    }
}

impl std::error::Error for TranslationIdError {}

impl TranslationId {
    pub fn new(s: impl Into<String>) -> Result<TranslationId, TranslationIdError> {
        let s = s.into();
        if s.is_empty() {
            return Err(TranslationIdError::Empty);
        }
        if let Some(c) = s
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
        {
            return Err(TranslationIdError::IllegalCharacter(c));
        }
        Ok(TranslationId(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TranslationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Base writing direction, derived from the language at import time and stored,
/// never inferred at render time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    Ltr,
    Rtl,
}

impl TextDirection {
    /// The value for an HTML `dir` attribute.
    pub const fn as_html(self) -> &'static str {
        match self {
            TextDirection::Ltr => "ltr",
            TextDirection::Rtl => "rtl",
        }
    }
}

impl fmt::Display for TextDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_html())
    }
}

/// A BCP-47 language tag, e.g. `en`, `he`, `arb`.
///
/// Not parsed into subtags: the only thing this application does with it is
/// emit it as an HTML `lang` attribute and pick a font stack. Parsing it would
/// be machinery without a consumer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LangTag(String);

impl LangTag {
    pub fn new(s: impl Into<String>) -> LangTag {
        LangTag(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LangTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A translation's identity and terms. Not its text — that is [`crate::text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub id: TranslationId,
    /// Short form shown in the UI, e.g. `WEB`.
    pub abbrev: String,
    pub name: String,
    pub language: LangTag,
    pub direction: TextDirection,
    pub scheme: SchemeId,
    pub canon: Canon,
    pub license: License,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_accept_the_ebible_convention() {
        assert_eq!(TranslationId::new("eng-web").unwrap().as_str(), "eng-web");
        assert_eq!(
            TranslationId::new("eng-kjv2006").unwrap().as_str(),
            "eng-kjv2006"
        );
    }

    #[test]
    fn ids_reject_anything_unsafe_as_a_filename_or_url_segment() {
        assert_eq!(TranslationId::new(""), Err(TranslationIdError::Empty));
        assert_eq!(
            TranslationId::new("eng_web"),
            Err(TranslationIdError::IllegalCharacter('_'))
        );
        assert_eq!(
            TranslationId::new("ENG-WEB"),
            Err(TranslationIdError::IllegalCharacter('E'))
        );
        assert_eq!(
            TranslationId::new("../etc/passwd"),
            Err(TranslationIdError::IllegalCharacter('.'))
        );
    }

    #[test]
    fn direction_maps_to_the_html_attribute() {
        assert_eq!(TextDirection::Ltr.as_html(), "ltr");
        assert_eq!(TextDirection::Rtl.as_html(), "rtl");
    }
}
