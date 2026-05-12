use crate::PlnValue;
use std::cell::RefCell;
use std::rc::Rc;

/// Internal mutable DOM node for tree building (mirrors C pointer approach).
#[derive(Debug, Clone)]
enum PlnNode {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Object(Vec<(String, Rc<RefCell<PlnNode>>)>),
    Array(Vec<Rc<RefCell<PlnNode>>>),
}

impl PlnNode {
    fn new_object() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(PlnNode::Object(Vec::new())))
    }
    fn new_array() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(PlnNode::Array(Vec::new())))
    }
    fn new_string(s: &str) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(PlnNode::String(s.to_string())))
    }

    fn to_value(&self) -> PlnValue {
        match self {
            PlnNode::Null => PlnValue::Null,
            PlnNode::Bool(b) => PlnValue::Bool(*b),
            PlnNode::Int(n) => PlnValue::Int(*n),
            PlnNode::Float(f) => PlnValue::Float(*f),
            PlnNode::String(s) => PlnValue::String(s.clone()),
            PlnNode::Object(obj) => {
                PlnValue::Object(obj.iter().map(|(k, v)| (k.clone(), v.borrow().to_value())).collect())
            }
            PlnNode::Array(arr) => {
                PlnValue::Array(arr.iter().map(|v| v.borrow().to_value()).collect())
            }
        }
    }
}

/// PopLine parser: line-by-line, builds DOM tree.
struct Parser {
    /// Stack of open containers (each an Rc into the tree).
    frames: Vec<Rc<RefCell<PlnNode>>>,
    /// Key for the next value being parsed (object context).
    key: String,
    /// Multi-line string accumulation buffer.
    strbuf: String,
    /// True while inside a multi-line string value.
    in_string: bool,
}

pub fn from_str(text: &str) -> Result<PlnValue, String> {
    let mut p = Parser::new();
    p.from_str(text)
}

impl Parser {
    fn new() -> Self {
        Parser {
            frames: Vec::new(),
            key: String::new(),
            strbuf: String::new(),
            in_string: false,
        }
    }

    fn from_str(&mut self, text: &str) -> Result<PlnValue, String> {
        for line in text.lines() {
            let line = line.trim_end_matches('\r');

            // Multi-line string continuation
            if self.in_string {
                match self.handle_string_line(line) {
                    Ok(Some((node, n_pop))) => {
                        self.in_string = false;
                        self.strbuf.clear();
                        self.add_to_top(node);
                        if n_pop > 0 {
                            self.pop_layers(n_pop)?;
                        }
                    }
                    Ok(None) => {
                        // string continues
                    }
                    Err(e) => return Err(e),
                }
                continue;
            }

            if line.is_empty() {
                continue;
            }

            // Detect pop prefix: digits at line start followed by space
            let (n_pop, content_start) = parse_pop_prefix(line);
            let content  = &line[content_start..];

            // Validate bare pop line
            if n_pop > 0 && content.is_empty() {
                return Err("bare pop line: no content after pop prefix".to_string());
            }

            // Close N containers (pop from innermost out)
            if n_pop > 0 {
                self.pop_layers(n_pop)?;
            }

            // If no frames yet, this line must open the root container
            if self.frames.is_empty() {
                // Check top-level inline containers: `[ [` or `[ {`
                if content.len() > 1 && content.as_bytes()[0] == b'[' {
                    let trimmed = content[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(content)?;
                        continue;
                    }
                }
                match content {
                    "{" => {
                        self.frames.push(PlnNode::new_object());
                        Ok(())
                    }
                    "[" => {
                        self.frames.push(PlnNode::new_array());
                        Ok(())
                    }
                    _ => Err("top level must be object or array".to_string()),
                }?;
                continue;
            }

            // Determine current container type
            let is_object = {
                let top = self.frames.last().unwrap();
                matches!(*top.borrow(), PlnNode::Object(_))
            };

            // Check for inline containers in array context: `[ [`、`[ {`、`{ [`、`{ {`
            if !is_object && content.len() > 1 {
                let bytes = content.as_bytes();
                if bytes[0] == b'[' || bytes[0] == b'{' {
                    let trimmed = content[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(content)?;
                        continue;
                    }
                }
            }

            match content {
                "{" => {
                    let n = PlnNode::new_object();
                    self.add_to_top(n.clone());
                    self.frames.push(n);
                }
                "[" => {
                    let n = PlnNode::new_array();
                    self.add_to_top(n.clone());
                    self.frames.push(n);
                }
                _ => {
                    if is_object {
                        self.parse_object_line(content)?;
                    } else {
                        self.parse_array_line(content)?;
                    }
                }
            }
        }

        // EOF: auto-close all containers
        if self.in_string {
            return Err("unclosed string at end of input".to_string());
        }
        while self.frames.len() > 1 {
            self.pop_one()?;
        }
        self.frames
            .pop()
            .map(|root| root.borrow().to_value())
            .ok_or("empty input".to_string())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Add `child` as a child of the current top-of-stack container.
    /// The child's key (self.key) is used if the top is an Object.
    fn add_to_top(&mut self, child: Rc<RefCell<PlnNode>>) {
        if self.frames.is_empty() {
            self.frames.push(child);
            return;
        }
        let top = self.frames.last().unwrap();
        let mut top_mut = top.borrow_mut();
        match &mut *top_mut {
            PlnNode::Object(ref mut obj) => {
                obj.push((self.key.clone(), child));
            }
            PlnNode::Array(ref mut arr) => {
                arr.push(child);
            }
            _ => unreachable!(),
        }
    }

    /// Pop the top container from the tracking stack.
    fn pop_one(&mut self) -> Result<(), String> {
        if self.frames.len() <= 1 {
            return Err("cannot pop root container".to_string());
        }
        self.frames.pop().ok_or("pop from empty stack")?;
        Ok(())
    }

    /// Pop `n` container layers from the stack, protecting the root.
    fn pop_layers(&mut self, n: usize) -> Result<(), String> {
        let n = n.min(self.frames.len().saturating_sub(1)); // protect root
        for _ in 0..n {
            self.frames.pop().ok_or("pop from empty stack")?;
        }
        Ok(())
    }

    /// Parse consecutive container openers on a single line: `[ [`, `[ {`, etc.
    fn parse_inline_containers(&mut self, s: &str) -> Result<(), String> {
        let trimmed = s.trim();
        let mut part = trimmed;
        while !part.is_empty() {
            let ch = part.as_bytes()[0] as char;
            if ch != '{' && ch != '[' {
                return Err("inline containers must be '{' or '['".to_string());
            }
            let n = if ch == '{' { PlnNode::new_object() } else { PlnNode::new_array() };
            if self.frames.is_empty() {
                self.frames.push(n);
            } else {
                self.add_to_top(n.clone());
                self.frames.push(n);
            }
            part = part[1..].trim_start();
        }
        Ok(())
    }

    /// Parse a line in object context: `key: value`.
    fn parse_object_line(&mut self, rest: &str) -> Result<(), String> {
        // Find ": " separator
        let sep = rest
            .find(": ")
            .ok_or_else(|| format!("object line must be 'key: value': '{}'", rest))?;
        let key = &rest[..sep];
        if !is_key_valid(key) {
            return Err(format!("invalid key: '{}'", key));
        }
        let mut val_part = &rest[sep + 2..];
        if val_part.is_empty() {
            return Err("empty value in object".to_string());
        }

        self.key = key.to_string();

        // Check value inline containers: `key: [ [` or `key: [ {`
        if val_part.len() > 1 {
            let bytes = val_part.as_bytes();
            if bytes[0] == b'[' || bytes[0] == b'{' {
                let trimmed = val_part[1..].trim_start();
                if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                    return self.parse_inline_containers(val_part);
                }
            }
        }

        // Check for inline container openers
        match val_part {
            "{" => {
                let n = PlnNode::new_object();
                self.add_to_top(n.clone());
                self.frames.push(n);
                return Ok(());
            }
            "[" => {
                let n = PlnNode::new_array();
                self.add_to_top(n.clone());
                self.frames.push(n);
                return Ok(());
            }
            _ => {}
        }

        // Leaf value: try pop suffix (only for non-container values)
        let n_pop = if val_part.as_bytes()[0] != b'{' && val_part.as_bytes()[0] != b'[' {
            trim_pop_suffix(val_part)
        } else {
            0
        };

        // Scalar / string value
        match parse_scalar(val_part, self)? {
            Some(node) => {
                self.add_to_top(node);
                if n_pop > 0 {
                    self.pop_layers(n_pop)?;
                }
            }
            None => {
                // multi-line string started; key is already in self.key
            }
        }
        Ok(())
    }

    /// Parse a line in array context: the line IS the value (after pop stripping).
    fn parse_array_line(&mut self, rest: &str) -> Result<(), String> {
        // Leaf value: try pop suffix (only for non-container values)
        let (trimmed_rest, n_pop) = if rest.as_bytes()[0] != b'{' && rest.as_bytes()[0] != b'[' {
            let (trimmed, n) = trim_pop_suffix_and_len(rest);
            (trimmed, n)
        } else {
            (rest, 0)
        };

        match parse_scalar(trimmed_rest, self)? {
            Some(node) => {
                self.add_to_top(node);
                if n_pop > 0 {
                    self.pop_layers(n_pop)?;
                }
            }
            None => {
                // multi-line string started
            }
        }
        Ok(())
    }

    /// Continue a multi-line string on a subsequent line.
    /// Returns Ok(Some((node, pop_count))) if the string closed on this line,
    /// Ok(None) if it continues, Err on invalid input.
    fn handle_string_line(&mut self, line: &str) -> Result<Option<(Rc<RefCell<PlnNode>>, usize)>, String> {
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote: ""
                    self.strbuf.push('"');
                    chars.next(); // consume second '"'
                } else {
                    // Closing quote — check for trailing content (pop suffix)
                    let trailing: String = chars.collect();
                    if trailing.is_empty() {
                        return Ok(Some((Rc::new(RefCell::new(PlnNode::String(
                            self.strbuf.clone(),
                        ))), 0)));
                    }
                    // Check for valid pop suffix " N"
                    let n_pop = pop_suffix_after(&trailing)?;
                    return Ok(Some((Rc::new(RefCell::new(PlnNode::String(
                        self.strbuf.clone(),
                    ))), n_pop)));
                }
            } else {
                self.strbuf.push(c);
            }
        }
        // End of line without closing — continue string
        self.strbuf.push('\n');
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Pop-prefix detection (digits + space at line start)
// ---------------------------------------------------------------------------

/// Returns (pop_count, content_start_index).
/// If no pop prefix, returns (0, 0).
fn parse_pop_prefix(line: &str) -> (usize, usize) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && bytes[i] == b' ' {
        let n: usize = line[..i].parse().unwrap_or(0);
        (n, i + 1)
    } else {
        (0, 0)
    }
}

// ---------------------------------------------------------------------------
// Pop-suffix detection (trailing " N" on leaf value lines)
// ---------------------------------------------------------------------------

/// Strip trailing " N" (space + digits) from a leaf value string.
/// Returns the pop count. The trimmed string is modified via slice.
fn trim_pop_suffix(s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len < 2 { return 0; }

    // Check if last char is a digit
    let mut i = len - 1;
    if !bytes[i].is_ascii_digit() { return 0; }

    // Scan backward while digits
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }

    // Check preceded by space
    if i == 0 || bytes[i - 1] != b' ' { return 0; }

    // Parse pop count
    s[i..len].parse().unwrap_or(0)
}

/// Strip trailing " N" from a leaf value string.
/// Returns (trimmed_slice, pop_count). The slice is the content before " N".
fn trim_pop_suffix_and_len<'a>(s: &'a str) -> (&'a str, usize) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len < 2 { return (s, 0); }

    let mut i = len - 1;
    if !bytes[i].is_ascii_digit() { return (s, 0); }

    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }

    if i == 0 || bytes[i - 1] != b' ' { return (s, 0); }

    let n: usize = s[i..len].parse().unwrap_or(0);

    // Also trim trailing whitespace after stripping "N" suffix
    // to avoid the trailing space in "value N \n" from serializer
    let content_end = i - 1;
    // Check if there's trailing whitespace before the " N" suffix
    // (e.g. serializer outputs "value 1 \n", we need "value")
    let mut actual_end = content_end;
    while actual_end > 0 && bytes[actual_end - 1] == b' ' {
        actual_end -= 1;
    }
    // But don't strip spaces that are part of quoted content
    // Only strip if the suffix pattern was clearly " N" at the end
    if actual_end < content_end {
        // Additional space after "N" means "value 1 " pattern from serializer
        // The actual content is "value" and pop is 1
        let content_str = &s[..actual_end];
        // But we must verify we didn't strip quotes
        if actual_end > 1 && bytes[actual_end - 1] == b'"' {
            // Quoted content: don't strip trailing spaces inside quotes
            // e.g. "hello " 1 -> content is "hello ", pop is 1
            (&s[..content_end], n)
        } else {
            (content_str, n)
        }
    } else {
        (&s[..content_end], n)
    }
}

/// Validate that a suffix after a closing quote is a valid " N" pop marker.
/// Returns Ok(pop_count) on success.
fn pop_suffix_after(s: &str) -> Result<usize, String> {
    if s.is_empty() {
        return Ok(0);
    }
    let bytes = s.as_bytes();
    if bytes[0] != b' ' {
        return Err(format!(
            "extra content after closing quote: '{}'",
            s
        ));
    }
    if bytes.len() < 2 || !bytes[1].is_ascii_digit() {
        return Err(format!(
            "extra content after closing quote: '{}'",
            s
        ));
    }
    // All remaining chars must be digits
    let mut n: usize = 0;
    for &b in &bytes[1..] {
        if !b.is_ascii_digit() {
            return Err(format!(
                "extra content after closing quote: '{}'",
                s
            ));
        }
        n = n * 10 + ((b - b'0') as usize);
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Value parsing
// ---------------------------------------------------------------------------

/// Parse a scalar/string value from `s`.
/// Returns:
///   Ok(Some(node))  — value fully parsed
///   Ok(None)        — multi-line string started, not yet closed
///   Err(msg)        — parse error
fn parse_scalar(
    s: &str,
    p: &mut Parser,
) -> Result<Option<Rc<RefCell<PlnNode>>>, String> {
    if s.is_empty() {
        return Err("empty value".to_string());
    }

    // String
    if s.starts_with('"') {
        return parse_quoted(&s[1..], p);
    }

    // Keywords
    match s {
        "true" => return Ok(Some(Rc::new(RefCell::new(PlnNode::Bool(true))))),
        "false" => return Ok(Some(Rc::new(RefCell::new(PlnNode::Bool(false))))),
        "null" => return Ok(Some(Rc::new(RefCell::new(PlnNode::Null)))),
        _ => {}
    }

    // Number (starts with digit or '-')
    let first = s.as_bytes()[0] as char;
    if first == '-' || first.is_ascii_digit() {
        if s.contains('.') || s.contains('e') || s.contains('E') {
            if let Ok(f) = s.parse::<f64>() {
                return Ok(Some(Rc::new(RefCell::new(PlnNode::Float(f)))));
            }
        } else if let Ok(n) = s.parse::<i64>() {
            return Ok(Some(Rc::new(RefCell::new(PlnNode::Int(n)))));
        }
    }

    Err(format!("bare string must be quoted: '{}'", s))
}

/// Parse a quoted string value (content is everything after the opening '"').
/// Same return convention as parse_scalar.
fn parse_quoted(
    content: &str,
    p: &mut Parser,
) -> Result<Option<Rc<RefCell<PlnNode>>>, String> {
    let mut result = String::new();
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.peek() == Some(&'"') {
                // Escaped quote
                result.push('"');
                chars.next(); // consume second '"'
            } else {
                // Closing quote
                let trailing: String = chars.collect();
                if !trailing.trim().is_empty() {
                    return Err(format!(
                        "extra content after closing quote: '{}'",
                        trailing
                    ));
                }
                return Ok(Some(Rc::new(RefCell::new(PlnNode::String(result)))));
            }
        } else {
            result.push(c);
        }
    }

    // End of content without closing — multi-line string
    p.in_string = true;
    p.strbuf.clear();
    p.strbuf.push_str(content);
    p.strbuf.push('\n');
    Ok(None)
}

// ---------------------------------------------------------------------------
// Key validation
// ---------------------------------------------------------------------------

fn is_key_valid(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    for c in key.chars() {
        match c {
            ':' | '"' | '{' | '[' | '#' | ' ' | '\t' | '\n' | '\r' => {
                return false;
            }
            _ => {}
        }
    }
    true
}
