//! JSON laid out for people: one field to a line, but lists of numbers
//! kept together on as few lines as fit.

use serde::Serialize;
use serde_json::Value;

const WIDTH: usize = 100;
const INDENT: &str = "  ";

pub fn to_string<T: Serialize>(value: &T) -> String {
    let pretty = serde_json::to_string_pretty(value).expect("content serializes");
    let mut out = String::new();
    let mut rest = pretty.as_str();
    let mut in_string = false;
    while let Some(c) = rest.chars().next() {
        if c == '[' && !in_string {
            if let Some(end) = numeric_array_end(rest) {
                let items: Vec<Value> = serde_json::from_str(&rest[..end]).expect("a JSON array");
                write_packed(&mut out, &items);
                rest = &rest[end..];
                continue;
            }
        }
        if c == '"' && !escaped(&out) {
            in_string = !in_string;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out.push('\n');
    out
}

/// Where the array starting `text` ends, if it holds only numbers (or
/// lists of them: deeper lists are laid out a row to a line).
fn numeric_array_end(text: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '[' if depth == 2 => return None,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            '0'..='9' | '-' | '+' | '.' | 'e' | 'E' | ',' => {}
            c if c.is_whitespace() => {}
            _ => return None,
        }
    }
    None
}

/// Whether a quote written now would be escaped.
fn escaped(out: &str) -> bool {
    out.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1
}

/// `[a, b, c]` on one line if it fits, otherwise wrapped, indented one
/// step more than the line it starts on.
fn write_packed(out: &mut String, items: &[Value]) {
    let line_start = out.rfind('\n').map_or(0, |n| n + 1);
    let current = &out[line_start..];
    let indent = current.len() - current.trim_start().len();
    let parts: Vec<String> = items.iter().map(compact).collect();
    let one_line = format!("[{}]", parts.join(", "));
    if current.len() + one_line.len() <= WIDTH {
        out.push_str(&one_line);
        return;
    }
    let inner = " ".repeat(indent + INDENT.len());
    out.push('[');
    let mut line = String::new();
    for (i, part) in parts.iter().enumerate() {
        let comma = if i + 1 < parts.len() { "," } else { "" };
        if !line.is_empty() && inner.len() + line.len() + 1 + part.len() + comma.len() > WIDTH {
            out.push('\n');
            out.push_str(&inner);
            out.push_str(&line);
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(part);
        line.push_str(comma);
    }
    out.push('\n');
    out.push_str(&inner);
    out.push_str(&line);
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push(']');
}

/// A value as compact JSON, with spaces after commas.
fn compact(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(compact).collect();
            format!("[{}]", parts.join(", "))
        }
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_pack_and_everything_reads_back() {
        let value = serde_json::json!({
            "name": "{1,2}A",
            "pairs": [[1, -2], [3, 4]],
            "grid": [[[1, 2], [3, 4]], [[5, 6], [7, 8]]],
            "many": (0..60).collect::<Vec<_>>(),
            "lines": ["a", "b"],
            "empty": [],
            "quoted": "a \"[1, 2]\" b \\",
        });
        let text = to_string(&value);
        assert!(text.contains("\"pairs\": [[1, -2], [3, 4]]"), "{text}");
        assert!(text.contains("\"lines\": [\n"), "{text}");
        assert!(
            text.contains("\"grid\": [\n    [[1, 2], [3, 4]],"),
            "{text}"
        );
        assert!(text.contains("\"many\": [\n    0, 1,"), "{text}");
        assert!(text.lines().all(|l| l.len() <= WIDTH), "{text}");
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back, value);
    }
}
