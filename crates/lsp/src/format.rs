//! Canonical schema formatter for `schema.ruprizzle`.

use tower_lsp::lsp_types::{Position, Range, TextEdit};

/// Formats the whole document and returns the `TextEdit`s.
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn format_document(text: &str) -> Vec<TextEdit> {
    let formatted = format_schema(text);
    if formatted == text {
        return Vec::new();
    }

    let line_count = text.lines().count() as u32;
    let last_line_len = text.lines().last().map_or(0, str::len) as u32;

    vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: line_count.saturating_sub(1),
                character: last_line_len,
            },
        },
        new_text: formatted,
    }]
}

/// Formats raw schema text into canonical, beautifully aligned form.
#[must_use]
pub fn format_schema(text: &str) -> String {
    let mut out = String::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut in_block = false;
    let mut block_lines: Vec<&str> = Vec::new();

    for line in &lines {
        let trimmed = line.trim();

        if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.is_empty() {
            if in_block {
                block_lines.push(line);
            } else if !out.is_empty() && !out.ends_with("\n\n") && trimmed.is_empty() {
                out.push('\n');
            } else if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            }
            continue;
        }

        if trimmed.ends_with('{') {
            in_block = true;
            block_lines.clear();
            block_lines.push(line);
            continue;
        }

        if trimmed == "}" {
            if in_block {
                block_lines.push(line);
                let formatted_block = format_block(&block_lines);
                if !out.is_empty() && !out.ends_with("\n\n") {
                    out.push('\n');
                }
                out.push_str(&formatted_block);
                out.push('\n');
                in_block = false;
                block_lines.clear();
            } else {
                out.push_str("}\n");
            }
            continue;
        }

        if in_block {
            block_lines.push(line);
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    if in_block && !block_lines.is_empty() {
        let formatted_block = format_block(&block_lines);
        out.push_str(&formatted_block);
    }

    out
}

fn format_block(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let header = lines[0].trim();
    let mut out = format!("{header}\n");

    // Parse model fields to align them nicely in columns.
    let is_model = header.starts_with("model ");
    let mut entries = Vec::new();

    for line in &lines[1..lines.len().saturating_sub(1)] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            entries.push(Entry::Blank);
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            entries.push(Entry::Comment(trimmed.to_owned()));
            continue;
        }
        if trimmed.starts_with("@@") {
            entries.push(Entry::BlockAttr(trimmed.to_owned()));
            continue;
        }

        if is_model {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[0];
                let type_name = parts[1];
                let attrs = if parts.len() > 2 {
                    parts[2..].join(" ")
                } else {
                    String::new()
                };
                entries.push(Entry::Field {
                    name: name.to_owned(),
                    type_name: type_name.to_owned(),
                    attrs,
                });
                continue;
            }
        }

        // Generic config or enum entry
        entries.push(Entry::Raw(trimmed.to_owned()));
    }

    // Determine column widths for model fields
    let mut max_name_len = 0;
    let mut max_type_len = 0;

    for entry in &entries {
        if let Entry::Field {
            name, type_name, ..
        } = entry
        {
            max_name_len = max_name_len.max(name.len());
            max_type_len = max_type_len.max(type_name.len());
        }
    }

    for entry in entries {
        match entry {
            Entry::Blank => out.push('\n'),
            Entry::Comment(c) => {
                out.push_str("  ");
                out.push_str(&c);
                out.push('\n');
            }
            Entry::BlockAttr(attr) => {
                out.push_str("  ");
                out.push_str(&attr);
                out.push('\n');
            }
            Entry::Field {
                name,
                type_name,
                attrs,
            } => {
                out.push_str("  ");
                out.push_str(&name);
                if max_name_len > name.len() {
                    out.push_str(&" ".repeat(max_name_len - name.len()));
                }
                out.push(' ');
                out.push_str(&type_name);
                if !attrs.is_empty() {
                    if max_type_len > type_name.len() {
                        out.push_str(&" ".repeat(max_type_len - type_name.len()));
                    }
                    out.push(' ');
                    out.push_str(&attrs);
                }
                out.push('\n');
            }
            Entry::Raw(r) => {
                out.push_str("  ");
                out.push_str(&r);
                out.push('\n');
            }
        }
    }

    out.push('}');
    out
}

enum Entry {
    Blank,
    Comment(String),
    BlockAttr(String),
    Field {
        name: String,
        type_name: String,
        attrs: String,
    },
    Raw(String),
}
