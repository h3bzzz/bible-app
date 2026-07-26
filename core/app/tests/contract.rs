//! Pins the JSON shape crossing the boundary.
//!
//! The renderer in `apps/ui/src/lib/render.js` reads these exact key names.
//! Nothing in the Rust type system connects the two, so the contract is
//! asserted here instead — a mismatch is otherwise invisible until a verse
//! silently loses its annotation cue at runtime.
//!
//! This test exists because that bug actually happened: `rename_all` on an
//! enum renames its *variants*, not their fields, so `spokenLayers` shipped as
//! `spoken_layers` and the hand-written JavaScript fixtures did not notice.

use bible_app_core::{view_of, ActiveLayers, TranslationView};
use bible_domain::{
    Annotation, AnnotationLayer, AnnotationPayload, Block, BookId, ChapterDoc, ChapterOverlay,
    CharStyle, Inline, LayerId, LayerKind, License, NoteId, ParaStyle, SchemeId, SourceRef,
    TranslationId, VerseRange, VerseRef, VerseSpan,
};
use serde_json::Value;

fn translation_view() -> TranslationView {
    TranslationView {
        id: "eng-web".into(),
        abbrev: "WEB".into(),
        name: "World English Bible".into(),
        language: "en".into(),
        direction: "ltr".into(),
        attribution: "public domain".into(),
    }
}

fn doc() -> ChapterDoc {
    ChapterDoc {
        translation: TranslationId::new("eng-web").unwrap(),
        book: BookId::Isa,
        chapter: 53,
        notes: vec![],
        blocks: vec![Block::Paragraph {
            style: ParaStyle::Poetry(1),
            inlines: vec![
                Inline::VerseStart(VerseSpan::single(4)),
                Inline::Char {
                    style: CharStyle::WordsOfJesus,
                    inner: vec![Inline::Text("borne".into())],
                },
                Inline::Text(" our griefs".into()),
                Inline::NoteRef(NoteId(0)),
                Inline::VerseEnd(VerseSpan::single(4)),
            ],
        }],
    }
}

fn prophecy_layer() -> (AnnotationLayer, Annotation) {
    let id = LayerId::new("messianic-prophecy").unwrap();
    let layer = AnnotationLayer {
        id: id.clone(),
        kind: LayerKind::MessianicProphecy,
        name: "Messianic prophecy".into(),
        spoken_label: "messianic prophecy".into(),
        license: License::public_domain(
            "Derived from New Testament citations",
            SourceRef {
                url: "https://example.invalid".into(),
                sha256: "0".repeat(64),
                publisher: "public domain".into(),
            },
        ),
        enabled_by_default: false,
    };
    let range = VerseRange::new(
        VerseRef::new(BookId::Isa, 53, 4, SchemeId::ORG),
        VerseRef::new(BookId::Isa, 53, 6, SchemeId::ORG),
    )
    .unwrap();
    let annotation = Annotation {
        layer: id,
        range,
        payload: AnnotationPayload::MessianicProphecy {
            fulfilled_in: vec![VerseRange::single(VerseRef::new(
                BookId::Mat,
                8,
                17,
                SchemeId::ORG,
            ))],
            note: None,
        },
    };
    (layer, annotation)
}

fn render_json(active: &ActiveLayers) -> Value {
    let doc = doc();
    let overlay = ChapterOverlay::build(
        doc.book,
        doc.chapter,
        &doc.verse_numbers(),
        active.annotations.iter(),
    );
    let view = view_of(&doc, translation_view(), &overlay, active);
    serde_json::to_value(&view).expect("view must serialise")
}

/// Every key the JavaScript renderer reads.
#[test]
fn the_json_uses_the_key_names_the_renderer_reads() {
    let (layer, annotation) = prophecy_layer();
    let active = ActiveLayers {
        layers: vec![layer],
        annotations: vec![annotation],
    };
    let json = render_json(&active);

    for key in [
        "translation",
        "book",
        "chapter",
        "title",
        "blocks",
        "notes",
        "verses",
        "nav",
    ] {
        assert!(json.get(key).is_some(), "ChapterView is missing {key:?}");
    }
    for key in [
        "id",
        "abbrev",
        "name",
        "language",
        "direction",
        "attribution",
    ] {
        assert!(
            json["translation"].get(key).is_some(),
            "translation is missing {key:?}"
        );
    }

    let block = &json["blocks"][0];
    assert_eq!(block["kind"], "paragraph");
    assert_eq!(block["style"], "q1");

    let verse = &block["inlines"][0];
    assert_eq!(verse["kind"], "verseStart");
    for key in ["start", "end", "number", "label", "layers", "spokenLayers"] {
        assert!(
            verse.get(key).is_some(),
            "verseStart is missing {key:?}; keys present: {:?}",
            verse.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }
    // The snake_case form must be gone, not merely accompanied.
    assert!(
        verse.get("spoken_layers").is_none(),
        "snake_case key leaked into the contract"
    );

    assert_eq!(verse["label"], "Isaiah 53 verse 4");
    assert_eq!(verse["number"], "4");
    assert_eq!(verse["layers"][0], "messianic-prophecy");
    assert_eq!(verse["spokenLayers"][0], "messianic prophecy");

    let span = &block["inlines"][1];
    assert_eq!(span["kind"], "span");
    assert_eq!(span["style"], "wj");

    let note = &block["inlines"][3];
    assert_eq!(note["kind"], "note");
    assert!(note.get("id").is_some());
}

#[test]
fn every_variant_tag_is_camel_case() {
    let json = render_json(&ActiveLayers::default());
    let block = &json["blocks"][0];
    let kinds: Vec<&str> = block["inlines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec!["verseStart", "span", "text", "note", "verseEnd"]
    );
    for k in kinds {
        assert!(!k.contains('_'), "variant tag {k:?} is not camelCase");
    }
}

#[test]
fn a_verse_with_no_active_layers_reports_empty_arrays_not_null() {
    // The renderer does `inline.spokenLayers?.length`; null would work, but an
    // empty array is the contract and keeps the JS branch-free.
    let json = render_json(&ActiveLayers::default());
    let verse = &json["blocks"][0]["inlines"][0];
    assert_eq!(verse["layers"], serde_json::json!([]));
    assert_eq!(verse["spokenLayers"], serde_json::json!([]));
}

#[test]
fn strong_is_omitted_rather_than_null_when_absent() {
    let json = render_json(&ActiveLayers::default());
    let span = &json["blocks"][0]["inlines"][1];
    assert!(
        span.get("strong").is_none(),
        "absent Strong's must be omitted, not null"
    );
}
