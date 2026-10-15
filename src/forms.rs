use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopLevelForm {
    pub text: String,
    pub range: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct FormNode {
    pub range: Range<usize>,
    pub is_collection: bool,
    pub depth: usize,
}

struct OpenDelimiter {
    start: usize,
    open_char: u8,
    depth: usize,
}

fn is_whitespace_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',')
}

fn is_delimiter_byte(byte: u8) -> bool {
    matches!(byte, b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';')
}

fn is_delimiter_or_whitespace_byte(byte: u8) -> bool {
    is_whitespace_byte(byte) || is_delimiter_byte(byte)
}

pub fn parse_forms(text: &str) -> Vec<FormNode> {
    let bytes = text.as_bytes();
    let mut open_stack: Vec<OpenDelimiter> = Vec::new();
    let mut forms: Vec<FormNode> = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' | b',' => {
                index += 1;
            }
            b';' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'"' => {
                let start_index = index;
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 1;
                        if index < bytes.len() {
                            index += 1;
                        }
                    } else if bytes[index] == b'"' {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
                forms.push(FormNode {
                    range: start_index..index,
                    is_collection: false,
                    depth: open_stack.len(),
                });
            }
            b'\\' => {
                let start_index = index;
                index += 1;
                if index < bytes.len() {
                    let next_byte = bytes[index];
                    if matches!(
                        next_byte,
                        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';' | b'\\'
                    ) {
                        index += 1;
                    } else {
                        while index < bytes.len() && !is_delimiter_or_whitespace_byte(bytes[index])
                        {
                            index += 1;
                        }
                    }
                }
                forms.push(FormNode {
                    range: start_index..index,
                    is_collection: false,
                    depth: open_stack.len(),
                });
            }
            b')' | b']' | b'}' => {
                let close_char = byte;
                let close_end = index + 1;
                index += 1;
                let matching_open = match close_char {
                    b')' => b'(',
                    b']' => b'[',
                    b'}' => b'{',
                    _ => continue,
                };
                if let Some(position) = open_stack
                    .iter()
                    .rposition(|open| open.open_char == matching_open)
                {
                    while open_stack.len() > position + 1 {
                        if let Some(unclosed) = open_stack.pop() {
                            forms.push(FormNode {
                                range: unclosed.start..index.saturating_sub(1),
                                is_collection: true,
                                depth: unclosed.depth,
                            });
                        }
                    }
                    if let Some(open) = open_stack.pop() {
                        forms.push(FormNode {
                            range: open.start..close_end,
                            is_collection: true,
                            depth: open.depth,
                        });
                    }
                } else if let Some(open) = open_stack.pop() {
                    forms.push(FormNode {
                        range: open.start..close_end,
                        is_collection: true,
                        depth: open.depth,
                    });
                } else {
                    forms.push(FormNode {
                        range: index.saturating_sub(1)..index,
                        is_collection: false,
                        depth: 0,
                    });
                }
            }
            _ => {
                let start_index = index;
                if byte == b'#' && index + 1 < bytes.len() && bytes[index + 1] == b'"' {
                    index += 2;
                    while index < bytes.len() {
                        if bytes[index] == b'\\' {
                            index += 1;
                            if index < bytes.len() {
                                index += 1;
                            }
                        } else if bytes[index] == b'"' {
                            index += 1;
                            break;
                        } else {
                            index += 1;
                        }
                    }
                    forms.push(FormNode {
                        range: start_index..index,
                        is_collection: false,
                        depth: open_stack.len(),
                    });
                    continue;
                }

                let mut lookahead_index = index;
                while lookahead_index < bytes.len() {
                    match bytes[lookahead_index] {
                        b'#' => {
                            lookahead_index += 1;
                            if lookahead_index < bytes.len() && bytes[lookahead_index] == b'?' {
                                lookahead_index += 1;
                                if lookahead_index < bytes.len() && bytes[lookahead_index] == b'@' {
                                    lookahead_index += 1;
                                }
                            }
                        }
                        b'\'' | b'`' | b'@' | b'^' => {
                            lookahead_index += 1;
                        }
                        b'~' => {
                            lookahead_index += 1;
                            if lookahead_index < bytes.len() && bytes[lookahead_index] == b'@' {
                                lookahead_index += 1;
                            }
                        }
                        _ => break,
                    }
                }

                if lookahead_index < bytes.len()
                    && matches!(bytes[lookahead_index], b'(' | b'[' | b'{')
                {
                    let open_char = bytes[lookahead_index];
                    let depth = open_stack.len();
                    open_stack.push(OpenDelimiter {
                        start: start_index,
                        open_char,
                        depth,
                    });
                    index = lookahead_index + 1;
                } else {
                    while index < bytes.len() && !is_delimiter_or_whitespace_byte(bytes[index]) {
                        index += 1;
                    }
                    forms.push(FormNode {
                        range: start_index..index,
                        is_collection: false,
                        depth: open_stack.len(),
                    });
                }
            }
        }
    }

    while let Some(open) = open_stack.pop() {
        forms.push(FormNode {
            range: open.start..bytes.len(),
            is_collection: true,
            depth: open.depth,
        });
    }

    forms
}

pub fn text_top_level_form(text: &str, offset: usize) -> Option<Range<usize>> {
    if offset > text.len() {
        return None;
    }
    let forms = parse_forms(text);
    let mut top_level_ranges: Vec<Range<usize>> = forms
        .into_iter()
        .filter(|form| form.depth == 0)
        .map(|form| form.range)
        .collect();
    top_level_ranges.sort_by_key(|range| range.start);

    for range in &top_level_ranges {
        if offset == range.end {
            return Some(range.clone());
        }
    }

    for range in &top_level_ranges {
        if range.contains(&offset) {
            return Some(range.clone());
        }
    }

    None
}

pub fn text_current_form(text: &str, offset: usize) -> Option<Range<usize>> {
    if offset > text.len() {
        return None;
    }
    let forms = parse_forms(text);

    let form_ending_before_offset = forms
        .iter()
        .filter(|form| form.range.end == offset)
        .min_by_key(|form| form.range.len());
    if let Some(form) = form_ending_before_offset {
        return Some(form.range.clone());
    }

    let atom_under_cursor = forms
        .iter()
        .find(|form| !form.is_collection && form.range.contains(&offset));
    if let Some(atom) = atom_under_cursor {
        return Some(atom.range.clone());
    }

    let innermost_collection = forms
        .iter()
        .filter(|form| form.is_collection && form.range.start <= offset && offset <= form.range.end)
        .min_by_key(|form| form.range.len());
    if let Some(collection) = innermost_collection {
        return Some(collection.range.clone());
    }

    None
}

pub fn closing_brackets(code: &str) -> String {
    let mut stack = Vec::new();
    let mut chars = code.chars();
    let mut in_string = false;
    while let Some(ch) = chars.next() {
        if in_string {
            match ch {
                '\\' => {
                    chars.next();
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\\' => {
                chars.next();
            }
            ';' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '(' => stack.push(')'),
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ')' | ']' | '}' => {
                stack.pop();
            }
            _ => {}
        }
    }
    let mut closing: String = stack.into_iter().rev().collect();
    if in_string {
        closing.insert(0, '"');
    }
    closing
}

pub fn top_level_form_to_cursor(text: &str, offset: usize) -> Option<TopLevelForm> {
    let range = text_top_level_form(text, offset)?;
    let end = offset.clamp(range.start, range.end);
    let mut form_text = text.get(range.start..end)?.to_string();
    if form_text.trim().is_empty() {
        return None;
    }
    form_text.push_str(&closing_brackets(&form_text));
    Some(TopLevelForm {
        text: form_text,
        range: range.start..end,
    })
}

pub fn ns_form_text(text: &str) -> Option<String> {
    let forms = parse_forms(text);
    let mut top_level_ranges: Vec<Range<usize>> = forms
        .into_iter()
        .filter(|form| form.depth == 0)
        .map(|form| form.range)
        .collect();
    top_level_ranges.sort_by_key(|range| range.start);

    for range in top_level_ranges {
        let form_text = text.get(range.clone())?;
        let trimmed = form_text.trim_start();
        if trimmed.starts_with("(ns ")
            || trimmed
                .strip_prefix("(ns")
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            return Some(form_text.to_string());
        }
    }
    None
}

pub fn parse_namespace(text: &str) -> Option<String> {
    let form = ns_form_text(text)?;
    let trimmed = form.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    let inner = &trimmed[1..];
    let mut tokens = inner.split_whitespace();
    let head = tokens.next()?;
    if head != "ns" && !head.ends_with("/ns") {
        return None;
    }
    for token in tokens {
        if token.starts_with('^') || token.starts_with('"') {
            continue;
        }
        let clean = token.trim_matches(|c| c == ')' || c == '(' || c == ',');
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }
    None
}

pub fn symbol_at_offset(text: &str, offset: usize) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let offset = offset.min(text.len());
    let bytes = text.as_bytes();

    let pos = if offset < bytes.len() && !is_delimiter_or_whitespace_byte(bytes[offset]) {
        offset
    } else if offset > 0 && !is_delimiter_or_whitespace_byte(bytes[offset - 1]) {
        offset - 1
    } else {
        return None;
    };

    let mut start = pos;
    while start > 0 && !is_delimiter_or_whitespace_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = pos;
    while end < bytes.len() && !is_delimiter_or_whitespace_byte(bytes[end]) {
        end += 1;
    }

    let raw = text.get(start..end)?.trim();
    let symbol = raw.strip_prefix('\'').unwrap_or(raw);
    let symbol = symbol.strip_prefix('@').unwrap_or(symbol);
    if symbol.is_empty() {
        None
    } else {
        Some(symbol.to_string())
    }
}
