//! Minimal reader/writer for Java `.properties` files.
//!
//! Observo keeps its settings in the same file as the previous Java version,
//! so the parser intentionally follows `java.util.Properties` semantics:
//! `#`/`!` comments, `=`/`:`/whitespace separators, backslash continuations
//! and `\uXXXX` escapes.
//!
//! Writing uses ASCII only (`\uXXXX` for non-ASCII) so the file remains readable
//! by the old Java application as well.

use std::collections::BTreeMap;

/// Parses a `.properties` document.
pub fn parse(input: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut logical_lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for raw_line in input.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if current.is_empty() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                continue;
            }
            current.push_str(trimmed);
        } else {
            current.push_str(line.trim_start());
        }

        if has_continuation(&current) {
            // A single trailing backslash continues the logical line.
            current.pop();
            continue;
        }

        logical_lines.push(std::mem::take(&mut current));
    }
    if !current.is_empty() {
        logical_lines.push(current);
    }

    for line in logical_lines {
        if let Some((key, value)) = split_key_value(&line) {
            result.insert(unescape(&key), unescape(&value));
        }
    }
    result
}

fn has_continuation(line: &str) -> bool {
    let trailing = line.chars().rev().take_while(|c| *c == '\\').count();
    trailing % 2 == 1
}

fn split_key_value(line: &str) -> Option<(String, String)> {
    let mut escaped = false;
    let mut separator = None;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '=' | ':' => {
                separator = Some((index, ch.len_utf8()));
                break;
            }
            c if c.is_whitespace() => {
                separator = Some((index, 0));
                break;
            }
            _ => {}
        }
    }

    let (index, separator_len) = separator?;
    let key = &line[..index];
    let mut value_start = index + separator_len;
    // Whitespace before and after the separator is not part of the value.
    while value_start < line.len() {
        let ch = line[value_start..].chars().next()?;
        if ch.is_whitespace() {
            value_start += ch.len_utf8();
        } else {
            break;
        }
    }
    Some((key.to_string(), line[value_start..].to_string()))
}

fn read_hex4(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<u32> {
    let mut code = 0u32;
    for _ in 0..4 {
        let digit = chars.next()?.to_digit(16)?;
        code = code * 16 + digit;
    }
    Some(code)
}

fn unescape(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('f') => output.push('\u{000C}'),
            Some('u') => {
                let Some(code) = read_hex4(&mut chars) else {
                    output.push_str("\\u");
                    continue;
                };
                if (0xD800..=0xDBFF).contains(&code) {
                    // Java writes supplementary characters as a UTF-16 surrogate pair.
                    let mut lookahead = chars.clone();
                    if lookahead.next() == Some('\\') && lookahead.next() == Some('u') {
                        chars.next();
                        chars.next();
                        if let Some(low) = read_hex4(&mut chars)
                            && (0xDC00..=0xDFFF).contains(&low)
                        {
                            let combined = 0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            if let Some(c) = char::from_u32(combined) {
                                output.push(c);
                            }
                            continue;
                        }
                    }
                } else if let Some(c) = char::from_u32(code) {
                    output.push(c);
                }
            }
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }
    output
}

/// Serialises properties in a deterministic, Java-compatible form.
pub fn write(values: &BTreeMap<String, String>, comment: &str) -> String {
    let mut output = String::new();
    for line in comment.lines() {
        output.push_str("# ");
        output.push_str(line);
        output.push('\n');
    }
    for (key, value) in values {
        output.push_str(&escape(key, true));
        output.push('=');
        output.push_str(&escape(value, false));
        output.push('\n');
    }
    output
}

fn escape(input: &str, is_key: bool) -> String {
    let mut output = String::with_capacity(input.len());
    for (index, ch) in input.chars().enumerate() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{000C}' => output.push_str("\\f"),
            ' ' if index == 0 => output.push_str("\\ "),
            '=' if is_key => output.push_str("\\="),
            ':' if is_key => output.push_str("\\:"),
            '#' | '!' if is_key && index == 0 => {
                output.push('\\');
                output.push(ch);
            }
            c if c.is_ascii() => output.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    output.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_separators_and_escapes() {
        let input = "\
# a comment
! another comment
language=de
window.x: 120
window.y 240
password=pa\\=ss\\:word
unicode=Gr\\u00FC\\u00DFe
continued=first\\
  second
empty=
";
        let parsed = parse(input);
        assert_eq!(parsed.get("language").unwrap(), "de");
        assert_eq!(parsed.get("window.x").unwrap(), "120");
        assert_eq!(parsed.get("window.y").unwrap(), "240");
        assert_eq!(parsed.get("password").unwrap(), "pa=ss:word");
        assert_eq!(parsed.get("unicode").unwrap(), "Grüße");
        assert_eq!(parsed.get("continued").unwrap(), "firstsecond");
        assert_eq!(parsed.get("empty").unwrap(), "");
    }

    #[test]
    fn roundtrips_special_characters() {
        let mut values = BTreeMap::new();
        values.insert("key".to_string(), "a=b:c#d!e".to_string());
        values.insert("unicode".to_string(), "Grüße 🙂".to_string());
        values.insert("multi".to_string(), "line1\nline2\ttab".to_string());
        values.insert(" leading".to_string(), " leading value".to_string());

        let encoded = write(&values, "test");
        assert!(encoded.is_ascii(), "writer must stay ASCII: {encoded}");
        assert_eq!(parse(&encoded), values);
    }

    #[test]
    fn odd_and_even_backslashes() {
        let mut values = BTreeMap::new();
        values.insert("path".to_string(), "C:\\temp\\\\".to_string());
        let encoded = write(&values, "");
        assert_eq!(parse(&encoded), values);
    }
}
