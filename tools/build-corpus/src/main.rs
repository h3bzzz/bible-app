//! `build-corpus` — turns vendored archives plus manifests into `corpus.db`.

use build_corpus::{build, BuildOptions};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
build-corpus — build corpus.db from vendored archives and manifests

USAGE:
    build-corpus [OPTIONS]

OPTIONS:
    --manifests <DIR>   Manifest directory        [default: data/manifests]
    --vendor <DIR>      Vendored archives         [default: data/vendor]
    --out <FILE>        Output database           [default: build/corpus.db]
    --strict            Refuse on unmodelled markers or scheme defects
    --trigram           Also build the substring search index
    -h, --help          Show this message
";

fn main() -> ExitCode {
    let mut opts = BuildOptions {
        manifests: PathBuf::from("data/manifests"),
        vendor: PathBuf::from("data/vendor"),
        output: PathBuf::from("build/corpus.db"),
        strict: false,
        trigram: false,
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| -> Result<String, String> {
            args.next()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        let result = match arg.as_str() {
            "--manifests" => value("--manifests").map(|v| opts.manifests = PathBuf::from(v)),
            "--vendor" => value("--vendor").map(|v| opts.vendor = PathBuf::from(v)),
            "--out" => value("--out").map(|v| opts.output = PathBuf::from(v)),
            "--strict" => {
                opts.strict = true;
                Ok(())
            }
            "--trigram" => {
                opts.trigram = true;
                Ok(())
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => Err(format!("unknown argument {other:?}")),
        };
        if let Err(e) = result {
            eprintln!("build-corpus: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    }

    match build(&opts) {
        Ok(report) => {
            println!(
                "built {} ({} translation(s), {} book(s), {} chapter(s), {} verse(s))",
                opts.output.display(),
                report.translations,
                report.books,
                report.chapters,
                report.verses
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("build-corpus: {e}");
            ExitCode::FAILURE
        }
    }
}
