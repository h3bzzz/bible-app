//! Per-translation manifests.
//!
//! §4.3: the corpus is the product and it carries its own licences,
//! independent of the application's. A manifest is machine-readable so the
//! build can *refuse* — a licensing mistake becomes a failed build rather than
//! a legal problem.

use bible_domain::{
    Canon, LangTag, License, SchemeId, SourceRef, TextDirection, Translation, TranslationId,
};
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub abbrev: String,
    pub name: String,
    pub language: String,
    pub direction: String,
    pub scheme: String,
    pub canon: String,
    pub license: LicenseSection,
    pub source: SourceSection,
}

#[derive(Debug, Deserialize)]
pub struct LicenseSection {
    #[serde(default)]
    pub spdx: Option<String>,
    pub redistributable: bool,
    pub commercial_use: bool,
    pub attribution: String,
}

#[derive(Debug, Deserialize)]
pub struct SourceSection {
    pub url: String,
    pub sha256: String,
    pub publisher: String,
    pub archive: String,
}

#[derive(Debug)]
pub enum ManifestError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            ManifestError::Toml { path, source } => {
                write!(f, "malformed manifest {}: {source}", path.display())
            }
            ManifestError::Invalid { path, reason } => {
                write!(f, "invalid manifest {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ManifestError::Toml {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Loads every `*.toml` in a directory, sorted by filename for a
    /// reproducible build.
    pub fn load_dir(dir: &Path) -> Result<Vec<(PathBuf, Manifest)>, ManifestError> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|source| ManifestError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect();
        paths.sort();
        paths
            .into_iter()
            .map(|p| Manifest::load(&p).map(|m| (p, m)))
            .collect()
    }

    /// Converts to the domain type, validating every field.
    pub fn to_translation(&self, path: &Path) -> Result<Translation, ManifestError> {
        let invalid = |reason: String| ManifestError::Invalid {
            path: path.to_path_buf(),
            reason,
        };

        let id = TranslationId::new(self.id.clone())
            .map_err(|e| invalid(format!("id {:?}: {e}", self.id)))?;
        let scheme = SchemeId::new(&self.scheme)
            .map_err(|e| invalid(format!("scheme {:?}: {e}", self.scheme)))?;
        let direction = match self.direction.as_str() {
            "ltr" => TextDirection::Ltr,
            "rtl" => TextDirection::Rtl,
            other => {
                return Err(invalid(format!(
                    "direction must be ltr or rtl, got {other:?}"
                )))
            }
        };
        let canon = match self.canon.as_str() {
            "protestant" => Canon::Protestant,
            "catholic" => Canon::Catholic,
            "eastern_orthodox" => Canon::EasternOrthodox,
            other => return Err(invalid(format!("unknown canon {other:?}"))),
        };
        if self.source.sha256.len() != 64
            || !self.source.sha256.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(invalid(format!(
                "source.sha256 must be 64 hex digits, got {:?}",
                self.source.sha256
            )));
        }

        Ok(Translation {
            id,
            abbrev: self.abbrev.clone(),
            name: self.name.clone(),
            language: LangTag::new(self.language.clone()),
            direction,
            scheme,
            canon,
            license: License {
                // An empty `spdx = ""` means "public domain, no instrument",
                // which is not the same as an SPDX id of "".
                spdx: self.license.spdx.clone().filter(|s| !s.trim().is_empty()),
                redistributable: self.license.redistributable,
                commercial_use: self.license.commercial_use,
                attribution: self.license.attribution.trim().to_string(),
                source: SourceRef {
                    url: self.source.url.clone(),
                    sha256: self.source.sha256.clone(),
                    publisher: self.source.publisher.clone(),
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn the_bundled_web_manifest_is_valid_and_redistributable() {
        let path = repo_root().join("data/manifests/eng-web.toml");
        let m = Manifest::load(&path).expect("manifest must load");
        let t = m.to_translation(&path).expect("manifest must validate");
        assert_eq!(t.id.as_str(), "eng-web");
        assert_eq!(t.abbrev, "WEB");
        assert_eq!(t.scheme, SchemeId::ENG);
        assert_eq!(t.canon, Canon::Protestant);
        assert_eq!(t.direction, TextDirection::Ltr);
        assert!(t.license.redistributable);
        assert!(t.license.check_bundlable("eng-web").is_ok());
        // Public domain, so no SPDX instrument is claimed.
        assert_eq!(t.license.spdx, None);
        assert!(t.license.attribution.contains("public domain"));
    }

    #[test]
    fn the_niv_fixture_validates_but_refuses_to_bundle() {
        // The manifest is well-formed. It is the *licence*, not the syntax,
        // that stops it — which is the distinction the gate has to get right.
        let path = repo_root().join("tests/fixtures/manifests/eng-niv.toml");
        let m = Manifest::load(&path).expect("fixture must load");
        let t = m
            .to_translation(&path)
            .expect("fixture must be well-formed");
        assert!(!t.license.redistributable);
        let err = t.license.check_bundlable("eng-niv").unwrap_err();
        assert!(err.to_string().contains("forbids redistribution"), "{err}");
    }

    #[test]
    fn a_bad_checksum_field_is_rejected() {
        let toml = r#"
            id = "x"
            abbrev = "X"
            name = "X"
            language = "en"
            direction = "ltr"
            scheme = "eng"
            canon = "protestant"
            [license]
            redistributable = true
            commercial_use = true
            attribution = "x"
            [source]
            url = "u"
            sha256 = "tooshort"
            publisher = "p"
            archive = "a.zip"
        "#;
        let m: Manifest = toml::from_str(toml).unwrap();
        let e = m.to_translation(Path::new("test.toml")).unwrap_err();
        assert!(e.to_string().contains("64 hex digits"), "{e}");
    }

    #[test]
    fn an_unknown_canon_or_direction_is_rejected() {
        let base = |canon: &str, dir: &str| {
            format!(
                r#"id="x"
                   abbrev="X"
                   name="X"
                   language="en"
                   direction="{dir}"
                   scheme="eng"
                   canon="{canon}"
                   [license]
                   redistributable=true
                   commercial_use=true
                   attribution="x"
                   [source]
                   url="u"
                   sha256="{}"
                   publisher="p"
                   archive="a.zip""#,
                "0".repeat(64)
            )
        };
        let m: Manifest = toml::from_str(&base("ethiopian", "ltr")).unwrap();
        assert!(m.to_translation(Path::new("t.toml")).is_err());
        let m: Manifest = toml::from_str(&base("protestant", "sideways")).unwrap();
        assert!(m.to_translation(Path::new("t.toml")).is_err());
    }
}
