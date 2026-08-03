//! Parsing helpers for JSON that came back from an LLM.
//!
//! Models are not strict JSON producers. Depending on the provider the payload
//! can arrive wrapped in prose or markdown fences, and models routinely slip in
//! a trailing comma before a closing brace, a `//` comment, or a raw newline
//! inside a string. Only the OpenAI and Gemini paths in `ai_service` can ask for
//! a JSON-mode response; the Anthropic path cannot, so its output is plain text
//! that merely *looks* like JSON.
//!
//! Every place that reads a model response goes through here so one sloppy
//! character can't throw away a whole (already paid for) pipeline run. The
//! repairs are structural only — nothing inside a string literal is rewritten
//! except to escape characters JSON forbids there — so a repaired parse still
//! reflects exactly what the model said.

use serde::de::DeserializeOwned;

/// Best-effort extraction of a JSON object from a model response that may be
/// wrapped in markdown fences or prose.
pub fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end >= start {
            return &trimmed[start..=end];
        }
    }
    trimmed
}

/// Parse a model response into a `serde_json::Value`, retrying once through
/// [`repair`] when the strict parse fails.
pub fn parse_value(raw: &str) -> Result<serde_json::Value, String> {
    parse(raw)
}

/// Parse a model response into any deserializable type, retrying once through
/// [`repair`] when the strict parse fails. The error is the *strict* parse error
/// (with its line/column), which is the one that describes what the model got
/// wrong.
pub fn parse<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    let json = extract_json(raw);
    match serde_json::from_str::<T>(json) {
        Ok(v) => Ok(v),
        Err(strict_err) => {
            let repaired = repair(json);
            serde_json::from_str::<T>(&repaired).map_err(|_| strict_err.to_string())
        }
    }
}

/// Fix up the JSON sloppiness models commonly emit: `//` and `/* */` comments,
/// trailing commas before `}`/`]`, and raw control characters inside strings.
/// A document that is already valid comes back unchanged.
pub fn repair(src: &str) -> String {
    normalize(&strip_comments(src))
}

/// Remove `//` line comments and `/* */` block comments that sit outside string
/// literals. JSON has no comments, but models add them anyway.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut in_string = false;

    while i < b.len() {
        let c = b[i];
        if in_string {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

/// Drop commas that are immediately followed by `}` or `]`, and escape the raw
/// control characters (newline/tab/…) that models leave unescaped inside
/// strings. Assumes comments are already gone.
fn normalize(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut in_string = false;

    while i < b.len() {
        let c = b[i];
        if in_string {
            if c == b'\\' && i + 1 < b.len() {
                out.push(c);
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            match c {
                b'"' => {
                    in_string = false;
                    out.push(c);
                }
                b'\n' => out.extend_from_slice(b"\\n"),
                b'\r' => out.extend_from_slice(b"\\r"),
                b'\t' => out.extend_from_slice(b"\\t"),
                // Any other control character JSON forbids inside a string.
                0x00..=0x1f => out.extend_from_slice(format!("\\u{:04x}", c).as_bytes()),
                _ => out.push(c),
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                in_string = true;
                out.push(c);
                i += 1;
            }
            b',' => {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && (b[j] == b'}' || b[j] == b']') {
                    i += 1; // trailing comma — drop it
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }

    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_fenced_prose() {
        let raw = "Sure! Here's the plan:\n```json\n{\"a\": 1}\n```\nHope that helps.";
        assert_eq!(extract_json(raw), "{\"a\": 1}");
    }

    #[test]
    fn valid_json_is_unchanged_by_repair() {
        let src = "{\"a\": [1, 2], \"b\": \"x, }\"}";
        assert_eq!(repair(src), src);
    }

    #[test]
    fn drops_trailing_comma_in_object_and_array() {
        let v = parse_value("{\"scenes\": [{\"n\": 1, \"clips\": [1, 2,],},]}").unwrap();
        assert_eq!(v["scenes"][0]["n"], 1);
        assert_eq!(v["scenes"][0]["clips"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn keeps_commas_that_are_not_trailing() {
        let v = parse_value("{\"a\": 1, \"b\": 2}").unwrap();
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn does_not_touch_commas_inside_strings() {
        let v = parse_value("{\"reason\": \"trimmed, ] and , } stay\", \"n\": 1,}").unwrap();
        assert_eq!(v["reason"], "trimmed, ] and , } stay");
        assert_eq!(v["n"], 1);
    }

    #[test]
    fn strips_comments() {
        let v = parse_value("{\n // pick the clean take\n \"a\": 1, /* and this */ \"b\": 2\n}").unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn leaves_comment_markers_inside_strings_alone() {
        let v = parse_value("{\"url\": \"https://x.dev/a\", \"note\": \"a /* b */ c\",}").unwrap();
        assert_eq!(v["url"], "https://x.dev/a");
        assert_eq!(v["note"], "a /* b */ c");
    }

    #[test]
    fn escapes_raw_newlines_inside_strings() {
        let v = parse_value("{\"reason\": \"line one\nline two\"}").unwrap();
        assert_eq!(v["reason"], "line one\nline two");
    }

    #[test]
    fn keeps_escaped_quotes_intact() {
        let v = parse_value("{\"reason\": \"he said \\\"go\\\", then left\",}").unwrap();
        assert_eq!(v["reason"], "he said \"go\", then left");
    }

    #[test]
    fn truncated_json_still_fails() {
        // A cut-off response is a real failure — repairing it would silently
        // render half an edit.
        assert!(parse_value("{\"scenes\": [{\"a\": 1}").is_err());
    }

    #[test]
    fn reports_the_strict_error_when_unrepairable() {
        let err = parse_value("not json at all {").unwrap_err();
        assert!(err.contains("line"), "expected a serde error, got: {}", err);
    }
}
