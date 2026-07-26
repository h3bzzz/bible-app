//! Loading annotation layers (ADR-0009) and activating them for display.
//!
//! A layer dataset is JSON authored in `org`. Loading validates every
//! annotation against the domain rules; activation maps the ranges into the
//! display translation's scheme — the step [`ActiveLayers`] documents as the
//! caller's job. Both live here so shells stay wiring (§5).

use crate::get_chapter::ActiveLayers;
use bible_domain::{
    Annotation, AnnotationLayer, AnnotationPayload, BookId, LayerId, LayerKind, License,
    SchemeId, SourceRef, VerseRange, VerseRef,
};
use bible_versification::Versifier;
use std::fmt;

/// The datasets compiled into the application. Embedded for the same reason
/// the versification tables are: the reader must work with zero network
/// access and no external data directory (§1).
const EMBEDDED: &[&str] = &[include_str!(
    "../../../data/annotations/messianic-prophecy.json"
)];

#[derive(Debug)]
pub enum LayerError {
    Json(String),
    Field { field: &'static str, reason: String },
    Reference(String),
    Invalid(String),
}

impl fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerError::Json(e) => write!(f, "layer dataset is not valid JSON: {e}"),
            LayerError::Field { field, reason } => write!(f, "layer field {field:?}: {reason}"),
            LayerError::Reference(e) => write!(f, "layer contains a bad reference: {e}"),
            LayerError::Invalid(e) => write!(f, "layer failed validation: {e}"),
        }
    }
}

impl std::error::Error for LayerError {}

/// A parsed, validated layer: the metadata and its annotations, still in
/// `org` exactly as authored.
#[derive(Debug, Clone)]
pub struct LoadedLayer {
    pub layer: AnnotationLayer,
    pub annotations: Vec<Annotation>,
}

/// Parses `"GEN 3:15"`, `"GEN 9:26-27"` or `"ISA 52:13-53:12"` in `scheme`.
fn parse_range(s: &str, scheme: SchemeId) -> Result<VerseRange, LayerError> {
    let bad = |why: &str| LayerError::Reference(format!("{s:?}: {why}"));
    let (book_s, rest) = s.rsplit_once(' ').ok_or_else(|| bad("no book code"))?;
    let book = BookId::from_usfm_code(book_s).ok_or_else(|| bad("unknown book code"))?;
    let cv = |part: &str| -> Result<(u16, u16), LayerError> {
        let (c, v) = part.split_once(':').ok_or_else(|| bad("missing colon"))?;
        Ok((
            c.parse().map_err(|_| bad("bad chapter"))?,
            v.parse().map_err(|_| bad("bad verse"))?,
        ))
    };
    let (start, end) = match rest.split_once('-') {
        None => {
            let (c, v) = cv(rest)?;
            ((c, v), (c, v))
        }
        Some((first, second)) => {
            let (c1, v1) = cv(first)?;
            match second.split_once(':') {
                Some(_) => {
                    let (c2, v2) = cv(second)?;
                    ((c1, v1), (c2, v2))
                }
                None => {
                    let v2 = second.parse().map_err(|_| bad("bad verse"))?;
                    ((c1, v1), (c1, v2))
                }
            }
        }
    };
    VerseRange::new(
        VerseRef::new(book, start.0, start.1, scheme),
        VerseRef::new(book, end.0, end.1, scheme),
    )
    .map_err(|e| bad(&e.to_string()))
}

/// Parses and validates one layer dataset.
pub fn parse_layer(json: &str) -> Result<LoadedLayer, LayerError> {
    let doc: serde_json::Value =
        serde_json::from_str(json).map_err(|e| LayerError::Json(e.to_string()))?;
    let str_field = |field: &'static str| -> Result<String, LayerError> {
        doc.get(field)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or(LayerError::Field {
                field,
                reason: "missing or not a string".into(),
            })
    };

    let id = LayerId::new(str_field("id")?).map_err(|e| LayerError::Field {
        field: "id",
        reason: e.to_string(),
    })?;
    let kind = match str_field("kind")?.as_str() {
        "messianicProphecy" => LayerKind::MessianicProphecy,
        "theme" => LayerKind::Theme,
        "crossReference" => LayerKind::CrossReference,
        "commentary" => LayerKind::Commentary,
        other => {
            return Err(LayerError::Field {
                field: "kind",
                reason: format!("unknown kind {other:?}"),
            })
        }
    };
    let scheme = str_field("scheme")?;
    if scheme != "org" {
        // ADR-0009: layers are authored in org, full stop. Refusing here beats
        // every annotation failing validation one by one below.
        return Err(LayerError::Field {
            field: "scheme",
            reason: format!("layers must be authored in org, found {scheme:?}"),
        });
    }

    let lic = doc
        .get("license")
        .ok_or(LayerError::Field {
            field: "license",
            reason: "missing".into(),
        })?
        .clone();
    let lic_str = |field: &'static str, v: &serde_json::Value| -> Result<String, LayerError> {
        v.as_str().map(str::to_string).ok_or(LayerError::Field {
            field,
            reason: "missing or not a string".into(),
        })
    };
    let source = lic.get("source").cloned().unwrap_or_default();
    let license = License {
        spdx: None,
        redistributable: lic
            .get("redistributable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        commercial_use: lic
            .get("commercialUse")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        attribution: lic_str("license.attribution", lic.get("attribution").unwrap_or(&serde_json::Value::Null))?,
        source: SourceRef {
            url: source
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            sha256: source
                .get("sha256")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            publisher: source
                .get("publisher")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        },
    };

    let layer = AnnotationLayer {
        id: id.clone(),
        kind,
        name: str_field("name")?,
        spoken_label: str_field("spokenLabel")?,
        license,
        enabled_by_default: doc
            .get("enabledByDefault")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };

    let raw = doc
        .get("annotations")
        .and_then(|v| v.as_array())
        .ok_or(LayerError::Field {
            field: "annotations",
            reason: "missing or not an array".into(),
        })?;

    let mut annotations = Vec::with_capacity(raw.len());
    for a in raw {
        let range_s = a.get("range").and_then(|v| v.as_str()).ok_or(LayerError::Field {
            field: "annotations[].range",
            reason: "missing or not a string".into(),
        })?;
        let range = parse_range(range_s, SchemeId::ORG)?;
        let payload = match kind {
            LayerKind::MessianicProphecy => {
                let fulfilled = a
                    .get("fulfilledIn")
                    .and_then(|v| v.as_array())
                    .ok_or(LayerError::Field {
                        field: "annotations[].fulfilledIn",
                        reason: "missing or not an array".into(),
                    })?
                    .iter()
                    .map(|f| {
                        f.as_str()
                            .ok_or(LayerError::Field {
                                field: "annotations[].fulfilledIn[]",
                                reason: "not a string".into(),
                            })
                            .and_then(|s| parse_range(s, SchemeId::ORG))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                AnnotationPayload::MessianicProphecy {
                    fulfilled_in: fulfilled,
                    note: a
                        .get("note")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                }
            }
            // The other kinds gain loaders when their first dataset ships;
            // refusing now beats silently mis-parsing later.
            other => {
                return Err(LayerError::Field {
                    field: "kind",
                    reason: format!("no loader for kind {other:?} yet"),
                })
            }
        };
        let annotation = Annotation {
            layer: id.clone(),
            range,
            payload,
        };
        annotation
            .validate(&layer)
            .map_err(|e| LayerError::Invalid(e.to_string()))?;
        annotations.push(annotation);
    }

    Ok(LoadedLayer { layer, annotations })
}

/// Loads every dataset compiled into the binary.
///
/// A compile-time dataset that fails to parse is a build defect, not a user
/// condition, so this returns `Result` and the shell treats `Err` as fatal
/// at startup — the same posture `build-corpus` takes toward bad manifests.
pub fn builtin_layers() -> Result<Vec<LoadedLayer>, LayerError> {
    EMBEDDED.iter().map(|json| parse_layer(json)).collect()
}

/// Maps one org range into `scheme`. `None` when nothing maps — absence is
/// an answer (a scheme that lacks the verse should highlight nothing).
fn map_range(
    range: &VerseRange,
    scheme: SchemeId,
    versifier: &impl Versifier,
) -> Option<VerseRange> {
    let starts = versifier.map(range.start(), scheme).ok()?;
    let ends = versifier.map(range.end(), scheme).ok()?;
    let start = starts.into_iter().min()?;
    let end = ends.into_iter().max()?;
    VerseRange::new(start, end).ok()
}

/// Builds the [`ActiveLayers`] for one translation: the enabled layers, with
/// every range expressed in the translation's scheme.
pub fn activate(
    loaded: &[LoadedLayer],
    enabled_ids: &[String],
    scheme: SchemeId,
    versifier: &impl Versifier,
) -> ActiveLayers {
    let mut layers = Vec::new();
    let mut annotations = Vec::new();
    for l in loaded {
        if !enabled_ids.iter().any(|id| id == l.layer.id.as_str()) {
            continue;
        }
        layers.push(l.layer.clone());
        for a in &l.annotations {
            let Some(range) = map_range(&a.range, scheme, versifier) else {
                continue;
            };
            let payload = match &a.payload {
                AnnotationPayload::MessianicProphecy { fulfilled_in, note } => {
                    AnnotationPayload::MessianicProphecy {
                        fulfilled_in: fulfilled_in
                            .iter()
                            .filter_map(|f| map_range(f, scheme, versifier))
                            .collect(),
                        note: note.clone(),
                    }
                }
                other => other.clone(),
            };
            annotations.push(Annotation {
                layer: a.layer.clone(),
                range,
                payload,
            });
        }
    }
    ActiveLayers {
        layers,
        annotations,
    }
}

/// The layers as the UI's picker sees them.
pub fn layer_views(loaded: &[LoadedLayer]) -> Vec<crate::view::LayerView> {
    loaded
        .iter()
        .map(|l| crate::view::LayerView {
            id: l.layer.id.to_string(),
            name: l.layer.name.clone(),
            spoken_label: l.layer.spoken_label.clone(),
            enabled_by_default: l.layer.enabled_by_default,
            attribution: l.layer.license.attribution.clone(),
        })
        .collect()
}

/// Activates layers for the translation the reader is in: looks up its
/// scheme and maps every enabled annotation into it. The one call a shell
/// makes before `get_chapter`.
pub fn activate_for_translation(
    db: &bible_storage::CorpusDb,
    translation_id: &str,
    loaded: &[LoadedLayer],
    enabled_ids: &[String],
    versifier: &impl Versifier,
) -> Result<ActiveLayers, crate::get_chapter::AppError> {
    use crate::get_chapter::AppError;
    if enabled_ids.is_empty() {
        return Ok(ActiveLayers::default());
    }
    let tid = bible_domain::TranslationId::new(translation_id)
        .map_err(|_| AppError::UnknownTranslation(translation_id.to_string()))?;
    let translation = db
        .translation(&tid)?
        .ok_or_else(|| AppError::UnknownTranslation(translation_id.to_string()))?;
    Ok(activate(loaded, enabled_ids, translation.scheme, versifier))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bible_versification::SchemeRegistry;

    fn reg() -> SchemeRegistry {
        SchemeRegistry::embedded().expect("embedded schemes load")
    }

    #[test]
    fn the_bundled_prophecy_dataset_loads_and_validates() {
        let layers = builtin_layers().expect("bundled dataset must load");
        assert_eq!(layers.len(), 1);
        let prophecy = &layers[0];
        assert_eq!(prophecy.layer.id.as_str(), "messianic-prophecy");
        assert!(!prophecy.layer.enabled_by_default, "ADR-0009: default off");
        assert!(prophecy.annotations.len() > 200);
        assert!(prophecy.layer.license.redistributable);
    }

    #[test]
    fn activation_maps_hebrew_numbering_back_to_english() {
        // org Malachi 3:23 must come out as English Malachi 4:5 for the
        // bundled translations, or the highlight lands on the wrong page.
        let layers = builtin_layers().unwrap();
        let active = activate(
            &layers,
            &["messianic-prophecy".to_string()],
            SchemeId::ENG,
            &reg(),
        );
        assert!(!active.annotations.is_empty());
        assert!(active.annotations.iter().any(|a| {
            a.range.start().book == BookId::Mal
                && a.range.start().chapter == 4
                && a.range.start().verse == 5
        }));
        assert!(active.annotations.iter().all(|a| a.range.scheme() == SchemeId::ENG));
    }

    #[test]
    fn a_disabled_layer_contributes_nothing() {
        let layers = builtin_layers().unwrap();
        let active = activate(&layers, &[], SchemeId::ENG, &reg());
        assert!(active.layers.is_empty());
        assert!(active.annotations.is_empty());
    }

    #[test]
    fn genesis_3_15_is_annotated_with_its_fulfilments() {
        let layers = builtin_layers().unwrap();
        let active = activate(
            &layers,
            &["messianic-prophecy".to_string()],
            SchemeId::ENG,
            &reg(),
        );
        let gen = active
            .annotations
            .iter()
            .find(|a| a.range.start().book == BookId::Gen && a.range.start().chapter == 3)
            .expect("Genesis 3:15 must be present");
        let AnnotationPayload::MessianicProphecy { fulfilled_in, .. } = &gen.payload else {
            panic!("wrong payload");
        };
        assert!(fulfilled_in.len() >= 4, "merged fulfilments expected");
    }

    #[test]
    fn a_layer_authored_outside_org_is_refused() {
        let json = r#"{"id":"x","kind":"messianicProphecy","name":"X",
            "spokenLabel":"x","scheme":"eng",
            "license":{"redistributable":true,"commercialUse":true,"attribution":"x"},
            "annotations":[]}"#;
        let e = parse_layer(json).unwrap_err();
        assert!(e.to_string().contains("org"), "{e}");
    }
}
