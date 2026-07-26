//! Licensing and provenance.
//!
//! Shared by translations and annotation layers, because both are corpora with
//! their own copyright independent of this application's. The corpus is the
//! product; its licences are not ours to assume.

use core::fmt;

/// Where a dataset came from, pinned so it can be re-fetched and verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub url: String,
    /// SHA-256 of the archive as vendored under `data/vendor/`.
    pub sha256: String,
    /// Free-text description of the upstream publisher.
    pub publisher: String,
}

/// Redistribution terms for a bundled dataset.
///
/// `redistributable` is the build gate: `build-corpus` refuses to bundle a
/// dataset whose manifest sets it false. That is the mechanism by which a
/// licensing mistake becomes a build failure instead of a legal problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct License {
    /// SPDX identifier where one applies, e.g. `CC0-1.0`, `CC-BY-SA-4.0`.
    /// `None` for public domain by age or for bespoke terms.
    pub spdx: Option<String>,
    pub redistributable: bool,
    pub commercial_use: bool,
    /// Attribution that must be surfaced in the UI wherever this dataset is
    /// displayed. Required even for public-domain works, because several of
    /// the permissively-licensed collections require it.
    pub attribution: String,
    pub source: SourceRef,
}

impl License {
    /// Public domain, no restrictions, attribution retained as courtesy.
    pub fn public_domain(attribution: impl Into<String>, source: SourceRef) -> License {
        License {
            spdx: None,
            redistributable: true,
            commercial_use: true,
            attribution: attribution.into(),
            source,
        }
    }
}

/// Why a dataset may not be bundled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleRefusal {
    NotRedistributable { dataset: String, terms: String },
    MissingAttribution { dataset: String },
}

impl fmt::Display for BundleRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundleRefusal::NotRedistributable { dataset, terms } => write!(
                f,
                "refusing to bundle {dataset}: its manifest forbids redistribution ({terms})"
            ),
            BundleRefusal::MissingAttribution { dataset } => {
                write!(f, "refusing to bundle {dataset}: attribution text is empty")
            }
        }
    }
}

impl std::error::Error for BundleRefusal {}

impl License {
    /// The §4.3 gate. Called by `build-corpus` before any dataset is written.
    pub fn check_bundlable(&self, dataset: &str) -> Result<(), BundleRefusal> {
        if !self.redistributable {
            return Err(BundleRefusal::NotRedistributable {
                dataset: dataset.to_string(),
                terms: self.spdx.clone().unwrap_or_else(|| "bespoke terms".into()),
            });
        }
        if self.attribution.trim().is_empty() {
            return Err(BundleRefusal::MissingAttribution {
                dataset: dataset.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> SourceRef {
        SourceRef {
            url: "https://ebible.org/Scriptures/eng-web_usfm.zip".into(),
            sha256: "102fbf7587c2a60566ea4ebd9c82fd66dc1418d64248ffafd8b14bbaa11c89f9".into(),
            publisher: "eBible.org".into(),
        }
    }

    #[test]
    fn public_domain_datasets_are_bundlable() {
        let l = License::public_domain("World English Bible, public domain", source());
        assert!(l.check_bundlable("eng-web").is_ok());
    }

    #[test]
    fn non_redistributable_datasets_are_refused() {
        let l = License {
            spdx: None,
            redistributable: false,
            commercial_use: false,
            attribution: "All rights reserved".into(),
            source: source(),
        };
        let err = l.check_bundlable("eng-niv").unwrap_err();
        assert!(matches!(err, BundleRefusal::NotRedistributable { .. }));
        assert!(err.to_string().contains("eng-niv"));
    }

    #[test]
    fn empty_attribution_is_refused_even_when_redistributable() {
        let l = License {
            spdx: Some("CC-BY-SA-4.0".into()),
            redistributable: true,
            commercial_use: false,
            attribution: "   ".into(),
            source: source(),
        };
        assert!(matches!(
            l.check_bundlable("x").unwrap_err(),
            BundleRefusal::MissingAttribution { .. }
        ));
    }
}
