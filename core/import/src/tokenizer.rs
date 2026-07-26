//! USFM lexer.
//!
//! USFM is a flat stream of backslash markers and text. The grammar that
//! matters here is small:
//!
//! ```text
//! \p                 paragraph marker
//! \v 1 In the ...    marker with a numeric argument
//! \wj ... \wj*       a character span, opened and closed
//! \+w x|strong="G1"\+w*   a character span nested inside another
//! ```
//!
//! The `+` prefix marks nesting. The `*` suffix closes. One space after a
//! marker is a separator and not part of the text — a rule that matters,
//! because the vendored corpus pads most lines with trailing spaces.

/// A lexical unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    Marker(Marker<'a>),
    Text(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marker<'a> {
    /// Marker name without the backslash, `+` prefix or `*` suffix: `p`, `q1`,
    /// `wj`, `w`.
    pub name: &'a str,
    /// True for a `\+x` form, i.e. explicitly nested inside another span.
    pub nested: bool,
    /// True for a `\x*` form, i.e. closing a span.
    pub closing: bool,
}

/// Splits USFM source into markers and text runs.
pub fn tokenize(src: &str) -> Vec<Token<'_>> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut text_start = 0;

    while i < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }

        if text_start < i {
            out.push(Token::Text(&src[text_start..i]));
        }

        let mut j = i + 1;
        let nested = j < bytes.len() && bytes[j] == b'+';
        if nested {
            j += 1;
        }
        let name_start = j;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric()) {
            j += 1;
        }
        let name = &src[name_start..j];
        let closing = j < bytes.len() && bytes[j] == b'*';
        if closing {
            j += 1;
        }

        if name.is_empty() {
            // A lone backslash. Not valid USFM; treat it as text so nothing is
            // lost, and let the parser's unknown-marker reporting surface it.
            i += 1;
            text_start = i - 1;
            continue;
        }

        out.push(Token::Marker(Marker {
            name,
            nested,
            closing,
        }));

        // Consume exactly one separator space after an *opening* marker. Any
        // further whitespace is real content (or trailing padding, normalised
        // later).
        //
        // After a *closing* marker the space is content, not a separator:
        // in `\w Yahweh|strong="H3068"\w* is my shepherd` the space before
        // "is" is an ordinary word space, and eating it welds two words
        // together.
        if !closing && j < bytes.len() && bytes[j] == b' ' {
            j += 1;
        }
        i = j;
        text_start = j;
    }

    if text_start < bytes.len() {
        out.push(Token::Text(&src[text_start..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker<'a>(name: &'a str, nested: bool, closing: bool) -> Token<'a> {
        Token::Marker(Marker {
            name,
            nested,
            closing,
        })
    }

    #[test]
    fn splits_a_paragraph_marker_from_its_text() {
        assert_eq!(
            tokenize("\\p Hello"),
            vec![marker("p", false, false), Token::Text("Hello")]
        );
    }

    #[test]
    fn consumes_exactly_one_separator_space() {
        // The second space is content, not separator.
        assert_eq!(
            tokenize("\\p  Hello"),
            vec![marker("p", false, false), Token::Text(" Hello")]
        );
    }

    #[test]
    fn keeps_the_word_space_after_a_closing_marker() {
        assert_eq!(
            tokenize("\\w Yahweh\\w* is my shepherd"),
            vec![
                marker("w", false, false),
                Token::Text("Yahweh"),
                marker("w", false, true),
                Token::Text(" is my shepherd")
            ]
        );
    }

    #[test]
    fn recognises_closing_markers() {
        assert_eq!(
            tokenize("\\wj Come\\wj*"),
            vec![
                marker("wj", false, false),
                Token::Text("Come"),
                marker("wj", false, true)
            ]
        );
    }

    #[test]
    fn recognises_nested_markers() {
        assert_eq!(
            tokenize("\\+w a\\+w*"),
            vec![
                marker("w", true, false),
                Token::Text("a"),
                marker("w", true, true)
            ]
        );
    }

    #[test]
    fn handles_alphanumeric_marker_names() {
        assert_eq!(tokenize("\\q1")[0], marker("q1", false, false));
        assert_eq!(tokenize("\\toc3")[0], marker("toc3", false, false));
        assert_eq!(tokenize("\\1x")[0], marker("1x", false, false));
    }

    #[test]
    fn keeps_text_before_the_first_marker() {
        assert_eq!(
            tokenize("lead \\p x"),
            vec![
                Token::Text("lead "),
                marker("p", false, false),
                Token::Text("x")
            ]
        );
    }

    #[test]
    fn tokenises_the_real_shape_of_a_tagged_word() {
        assert_eq!(
            tokenize("\\w Yahweh|strong=\"H3068\"\\w*"),
            vec![
                marker("w", false, false),
                Token::Text("Yahweh|strong=\"H3068\""),
                marker("w", false, true)
            ]
        );
    }

    #[test]
    fn tokenises_a_verse_marker_with_its_number_as_text() {
        assert_eq!(
            tokenize("\\v 1 In the beginning"),
            vec![marker("v", false, false), Token::Text("1 In the beginning")]
        );
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert_eq!(tokenize(""), vec![]);
    }

    #[test]
    fn a_lone_backslash_is_preserved_as_text() {
        let t = tokenize("a \\ b");
        let joined: String = t
            .iter()
            .map(|x| match x {
                Token::Text(s) => *s,
                Token::Marker(_) => "",
            })
            .collect();
        assert!(joined.contains('\\'), "backslash must not vanish: {t:?}");
    }
}
