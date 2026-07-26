//! `build-annotations` — reference-pair TSV in, annotation layer JSON out.

use build_annotations::{build_annotations, fmt_range, prophecy_layer};
use bible_domain::AnnotationPayload;
use bible_versification::SchemeRegistry;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
build-annotations — build the messianic-prophecy layer from extracted refs

USAGE:
    build-annotations [--refs <TSV>] [--out <JSON>]

OPTIONS:
    --refs <TSV>   Reference pairs   [default: data/annotations/sources/messianic-prophecy-refs.tsv]
    --out <JSON>   Output dataset    [default: data/annotations/messianic-prophecy.json]
    -h, --help     Show this message
";

fn main() -> ExitCode {
    let mut refs = PathBuf::from("data/annotations/sources/messianic-prophecy-refs.tsv");
    let mut out = PathBuf::from("data/annotations/messianic-prophecy.json");

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--refs" => match args.next() {
                Some(v) => refs = PathBuf::from(v),
                None => return usage_err("--refs requires a value"),
            },
            "--out" => match args.next() {
                Some(v) => out = PathBuf::from(v),
                None => return usage_err("--out requires a value"),
            },
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => return usage_err(&format!("unknown argument {other:?}")),
        }
    }

    let tsv = match std::fs::read_to_string(&refs) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("build-annotations: {}: {e}", refs.display());
            return ExitCode::FAILURE;
        }
    };

    let reg = match SchemeRegistry::embedded() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("build-annotations: {e}");
            return ExitCode::FAILURE;
        }
    };

    let annotations = match build_annotations(&tsv, &reg) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("build-annotations: {e}");
            return ExitCode::FAILURE;
        }
    };

    let sha = format!("{:x}", Sha256::new_with_prefix(tsv.as_bytes()).finalize());
    let layer = prophecy_layer(sha);

    let ann_json: Vec<_> = annotations
        .iter()
        .map(|a| {
            let AnnotationPayload::MessianicProphecy { fulfilled_in, .. } = &a.payload else {
                unreachable!("build_annotations only emits prophecy payloads");
            };
            json!({
                "range": fmt_range(&a.range),
                "fulfilledIn": fulfilled_in.iter().map(fmt_range).collect::<Vec<_>>(),
            })
        })
        .collect();

    let doc = json!({
        "id": layer.id.as_str(),
        "kind": "messianicProphecy",
        "name": layer.name,
        "spokenLabel": layer.spoken_label,
        "enabledByDefault": layer.enabled_by_default,
        "scheme": "org",
        "license": {
            "redistributable": layer.license.redistributable,
            "commercialUse": layer.license.commercial_use,
            "attribution": layer.license.attribution,
            "source": {
                "url": layer.license.source.url,
                "sha256": layer.license.source.sha256,
                "publisher": layer.license.source.publisher,
            },
        },
        "annotations": ann_json,
    });

    let pretty = serde_json::to_string_pretty(&doc).expect("layer document serialises");
    if let Err(e) = std::fs::write(&out, pretty + "\n") {
        eprintln!("build-annotations: {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "built {} ({} annotation(s) from {} line(s))",
        out.display(),
        annotations.len(),
        tsv.lines().filter(|l| !l.trim().is_empty()).count()
    );
    ExitCode::SUCCESS
}

fn usage_err(msg: &str) -> ExitCode {
    eprintln!("build-annotations: {msg}\n\n{USAGE}");
    ExitCode::FAILURE
}
