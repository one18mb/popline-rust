use crate::PlnValue;
use std::mem;
use memchr::memchr;

// ---------------------------------------------------------------------------
// Direct PlnValue builder — no intermediate arena
// ---------------------------------------------------------------------------

enum BuildState {
    Obj(Vec<(String, PlnValue)>),
    Arr(Vec<PlnValue>),
}

struct Frame {
    state: BuildState,
    key: Option<String>,
}

struct Parser {
    stack: Vec<Frame>,
    key: String,
    strbuf: String,
    in_string: bool,
}

pub fn from_str(text: &str) -> Result<PlnValue, String> {
    let mut p = Parser::new();
    p.parse(text)
}

impl Parser {
    fn new() -> Self {
        Parser {
            stack: Vec::new(),
            key: String::new(),
            strbuf: String::new(),
            in_string: false,
        }
    }

    fn push_child(&mut self, val: PlnValue) {
        if let Some(frame) = self.stack.last_mut() {
            match &mut frame.state {
                BuildState::Obj(ref mut children) => {
                    children.push((mem::take(&mut self.key), val));
                }
                BuildState::Arr(ref mut children) => {
                    children.push(val);
                }
            }
        }
    }

    fn open(&mut self, is_obj: bool) {
        let state = if is_obj { BuildState::Obj(Vec::new()) } else { BuildState::Arr(Vec::new()) };
        let key = if self.stack.is_empty() { None } else { Some(mem::take(&mut self.key)) };
        self.stack.push(Frame { state, key });
    }

    fn close(&mut self) -> Option<(PlnValue, Option<String>)> {
        self.stack.pop().map(|frame| {
            let val = match frame.state {
                BuildState::Obj(c) => PlnValue::Object(c),
                BuildState::Arr(c) => PlnValue::Array(c),
            };
            (val, frame.key)
        })
    }

    fn pop_layers(&mut self, n: usize) {
        let n = n.min(self.stack.len().saturating_sub(1));
        for _ in 0..n {
            if let Some((val, key)) = self.close() {
                // Use the stored key from when the container opened.
                // This is the key that was set by parse_object_line (e.g. outer, a).
                self.key = key.unwrap_or_default();
                self.push_child(val);
            }
        }
    }

    fn parse(&mut self, text: &str) -> Result<PlnValue, String> {
        let bytes = text.as_bytes();
        let text_len = text.len();
        let mut line_start = 0;

        while line_start < text_len {
            // memchr-accelerated \n search
            let nl = match memchr(b'\n', &bytes[line_start..]) {
                Some(offset) => line_start + offset,
                None => text_len,
            };
            let ls = line_start; // save original position before advance
            line_start = nl + 1;

            // Strip \r (check byte before \n)
            let line = if nl > ls && bytes[nl - 1] == b'\r' {
                &text[ls..nl - 1]
            } else {
                &text[ls..nl]
            };

            // Multi-line string continuation
            if self.in_string {
                if let Some((ss, n_pop)) = self.handle_string_line(line)? {
                    self.in_string = false;
                    self.strbuf.clear();
                    self.push_child(PlnValue::String(ss));
                    self.pop_layers(n_pop);
                }
                continue;
            }

            if line.is_empty() {
                if !self.stack.is_empty() {
                    return Err("empty line not allowed in message body".to_string());
                }
                continue;
            }

            // Root level
            if self.stack.is_empty() {
                // Inline containers: `[ [` or `[ {`
                if line.len() > 1 && line.as_bytes()[0] == b'[' {
                    let trimmed = line[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(line)?;
                        continue;
                    }
                }
                match line {
                    "{" => { self.open(true); continue; }
                    "[" => { self.open(false); continue; }
                    _ => { return self.parse_scalar_root(line); }
                }
            }

            let is_obj = matches!(self.stack.last().unwrap().state, BuildState::Obj(_));

            // Inline containers in array context: `[ [` / `[ {` / `{ [` / `{ {`
            if !is_obj && line.len() > 1 {
                let b = line.as_bytes();
                if b[0] == b'[' || b[0] == b'{' {
                    let trimmed = line[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(line)?;
                        continue;
                    }
                }
            }

            match line {
                "{" => { self.open(true); }
                "[" => { self.open(false); }
                _ => {
                    if is_obj {
                        self.parse_object_line(line)?;
                    } else {
                        self.parse_array_line(line)?;
                    }
                }
            }
        }

        if self.in_string {
            return Err("unclosed string at end of input".to_string());
        }
        while self.stack.len() > 1 {
            if let Some((val, key)) = self.close() {
                self.key = key.unwrap_or_default();
                self.push_child(val);
            }
        }
        self.close()
            .map(|(val, _key)| val)
            .ok_or("empty input".to_string())
    }

    fn parse_inline_containers(&mut self, s: &str) -> Result<(), String> {
        let part = s.trim();
        let mut pos = part;
        while !pos.is_empty() {
            let ch = pos.as_bytes()[0];
            self.open(ch == b'{');
            pos = pos[1..].trim_start();
        }
        Ok(())
    }

    fn parse_object_line(&mut self, rest: &str) -> Result<(), String> {
        // Single-pass key validation + ": " search (like C parser)
        let bytes = rest.as_bytes();
        let len = bytes.len();
        let mut key_end = None;
        for i in 0..len.saturating_sub(1) {
            let b = bytes[i];
            if b == b':' && bytes[i + 1] == b' ' { key_end = Some(i); break; }
            if b == b':' { return Err("invalid key (colon)".into()); }
            if b == b'"' || b == b'{' || b == b'[' || b == b'#' ||
               b == b' ' || b == b'\t' { return Err("invalid key".into()); }
            // \n and \r can't appear in a single line
        }
        let sep = key_end.ok_or_else(|| format!("missing 'key: value': '{}'", rest))?;
        self.key = rest[..sep].to_string();
        let val_part = &rest[sep + 2..];

        match val_part {
            "{" => { self.open(true); return Ok(()); }
            "[" => { self.open(false); return Ok(()); }
            _ => {}
        }

        let (val, n_pop) = if val_part.as_bytes()[0] != b'{' && val_part.as_bytes()[0] != b'[' {
            fwd_trim_pop_suffix(val_part)
        } else {
            (val_part, 0)
        };

        match self.parse_scalar_value(val)? {
            Some(pv) => { self.push_child(pv); self.pop_layers(n_pop); }
            None => {} // multi-line string started, key is in self.key
        }
        Ok(())
    }

    fn parse_array_line(&mut self, rest: &str) -> Result<(), String> {
        let (trimmed, n_pop) = if rest.as_bytes()[0] != b'{' && rest.as_bytes()[0] != b'[' {
            fwd_trim_pop_suffix(rest)
        } else {
            (rest, 0)
        };

        match self.parse_scalar_value(trimmed)? {
            Some(pv) => { self.push_child(pv); self.pop_layers(n_pop); }
            None => {}
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Scalar/string parsing helpers
    // -----------------------------------------------------------------------

    fn parse_scalar_root(&mut self, s: &str) -> Result<PlnValue, String> {
        if s.is_empty() { return Err("empty value".to_string()); }
        if s.as_bytes()[0] == b'"' {
            let content = &s[1..];
            return self.parse_quoted_string(content);
        }
        match s {
            "true" => return Ok(PlnValue::Bool(true)),
            "false" => return Ok(PlnValue::Bool(false)),
            "null" => return Ok(PlnValue::Null),
            _ => {}
        }
        if let Some(n) = parse_number(s) { return Ok(n); }
        Err(format!("bare string must be quoted: '{}'", s))
    }

    fn parse_scalar_value(&mut self, s: &str) -> Result<Option<PlnValue>, String> {
        if s.is_empty() { return Err("empty value".to_string()); }
        let bytes = s.as_bytes();
        if bytes[0] == b'"' {
            return self.parse_quoted(&s[1..]);
        }
        match s {
            "true" => return Ok(Some(PlnValue::Bool(true))),
            "false" => return Ok(Some(PlnValue::Bool(false))),
            "null" => return Ok(Some(PlnValue::Null)),
            _ => {}
        }
        if let Some(n) = parse_number(s) { return Ok(Some(n)); }
        Err(format!("bare string must be quoted: '{}'", s))
    }

    /// Parse a quoted string. Returns Ok(v) if complete, Err("multi-line") if continues.
    fn parse_quoted_to(&self, content: &str, out: &mut String) -> Result<(), String> {
        out.clear();
        let mut chars = content.chars();
        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.as_str().starts_with('"') {
                    out.push('"'); chars.next();
                } else {
                    let trailing = chars.as_str();
                    if !trailing.trim().is_empty() {
                        return Err(format!("extra after closing quote: '{}'", trailing));
                    }
                    return Ok(());
                }
            } else {
                out.push(c);
            }
        }
        Err("multi-line".into())
    }

    fn parse_quoted(&mut self, content: &str) -> Result<Option<PlnValue>, String> {
        let mut res = String::with_capacity(content.len());
        match self.parse_quoted_to(content, &mut res) {
            Ok(()) => Ok(Some(PlnValue::String(res))),
            Err(_) => {
                self.in_string = true;
                self.strbuf = res;
                self.strbuf.push('\n');
                Ok(None)
            }
        }
    }

    fn parse_quoted_string(&self, content: &str) -> Result<PlnValue, String> {
        let mut res = String::with_capacity(content.len());
        self.parse_quoted_to(content, &mut res).map(|_| PlnValue::String(res))
            .map_err(|_| "multi-line strings not supported at root".into())
    }

    fn handle_string_line(&mut self, line: &str) -> Result<Option<(String, usize)>, String> {
        let bytes = line.as_bytes();
        let mut start = 0usize;
        for i in 0..line.len() {
            if bytes[i] == b'"' {
                if i + 1 < line.len() && bytes[i + 1] == b'"' {
                    self.strbuf.push_str(&line[start..=i]);
                    start = i + 2;
                    continue;
                }
                self.strbuf.push_str(&line[start..i]);
                let trailing = &line[i + 1..];
                if trailing.is_empty() {
                    return Ok(Some((mem::take(&mut self.strbuf), 0)));
                }
                let n_pop = pop_suffix_after(trailing)?;
                return Ok(Some((mem::take(&mut self.strbuf), n_pop)));
            }
        }
        self.strbuf.push_str(&line[start..]);
        self.strbuf.push('\n');
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Number parsing — hand-rolled, no str::parse
// ---------------------------------------------------------------------------

fn parse_number(s: &str) -> Option<PlnValue> {
    let bytes = s.as_bytes();
    if bytes.is_empty() { return None; }

    let mut i = 0;
    let neg = if bytes[0] == b'-' { i = 1; true } else { false };
    if i >= bytes.len() || bytes[i] < b'0' || bytes[i] > b'9' { return None; }

    // Check for float (has '.' or 'e' or 'E')
    let mut is_float = false;
    for &b in &bytes[i..] {
        if b == b'.' || b == b'e' || b == b'E' { is_float = true; break; }
    }

    if is_float {
        // Use f64 parse for floats (lexical/simd is a crate dependency)
        let f: f64 = s.parse().ok()?;
        Some(PlnValue::Float(f))
    } else {
        // Hand-rolled i64 accumulation
        let mut val: i64 = 0;
        while i < bytes.len() {
            let d = bytes[i].wrapping_sub(b'0');
            if d > 9 { return None; }
            val = val.wrapping_mul(10).wrapping_add(d as i64);
            i += 1;
        }
        Some(PlnValue::Int(if neg { -val } else { val }))
    }
}

// ---------------------------------------------------------------------------
// Pop suffix detection
// ---------------------------------------------------------------------------

fn fwd_trim_pop_suffix<'a>(s: &'a str) -> (&'a str, usize) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    for i in 0..len {
        if bytes[i] == b'"' { continue; }
        if bytes[i] == b' ' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
            // Validate remaining chars are all digits
            let mut all_digits = true;
            for j in i + 1..len {
                if !bytes[j].is_ascii_digit() { all_digits = false; break; }
            }
            if all_digits {
                let mut n: usize = (bytes[i + 1] - b'0') as usize;
                for j in i + 2..len { n = n * 10 + (bytes[j] - b'0') as usize; }
                return (&s[..i], n);
            }
        }
    }
    (s, 0)
}

fn pop_suffix_after(s: &str) -> Result<usize, String> {
    if s.is_empty() { return Ok(0); }
    let bytes = s.as_bytes();
    if bytes[0] != b' ' { return Err(format!("trailing after quote: '{}'", s)); }
    let mut n: usize = 0;
    for &b in &bytes[1..] {
        if !b.is_ascii_digit() { return Err(format!("trailing after quote: '{}'", s)); }
        n = n * 10 + ((b - b'0') as usize);
    }
    Ok(n)
}

