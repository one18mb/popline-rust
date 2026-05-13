use crate::PlnValue;
use std::mem;

/// Internal node: flat storage, no Rc/RefCell overhead.
#[derive(Debug, Clone)]
enum PlnRaw {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Obj(/* key, child_index */ Vec<(String, usize)>),
    Arr(Vec<usize>),
}

/// PopLine parser using arena (index-based node storage).
struct Parser {
    arena: Vec<PlnRaw>,
    /// Stack of container indices.
    frames: Vec<usize>,
    key: String,
    strbuf: String,
    in_string: bool,
}

pub fn from_str(text: &str) -> Result<PlnValue, String> {
    let mut p = Parser::new();
    p.parse(text)
}

/* ─── Arena helpers ─────────────────────────────────────── */

fn alloc_obj(arena: &mut Vec<PlnRaw>) -> usize {
    let idx = arena.len();
    arena.push(PlnRaw::Obj(Vec::new()));
    idx
}

fn alloc_arr(arena: &mut Vec<PlnRaw>) -> usize {
    let idx = arena.len();
    arena.push(PlnRaw::Arr(Vec::new()));
    idx
}

/* ─── Convert arena to PlnValue (consumes arena) ────────── */

fn arena_to_value(arena: &mut Vec<PlnRaw>, idx: usize) -> PlnValue {
    let node = &mut arena[idx];
    match mem::replace(node, PlnRaw::Null) {
        PlnRaw::Null => PlnValue::Null,
        PlnRaw::Bool(b) => PlnValue::Bool(b),
        PlnRaw::Int(n) => PlnValue::Int(n),
        PlnRaw::Float(f) => PlnValue::Float(f),
        PlnRaw::String(s) => PlnValue::String(s),
        PlnRaw::Obj(children) => {
            PlnValue::Object(
                children.into_iter()
                    .map(|(k, ci)| (k, arena_to_value(arena, ci)))
                    .collect()
            )
        }
        PlnRaw::Arr(children) => {
            PlnValue::Array(
                children.into_iter()
                    .map(|ci| arena_to_value(arena, ci))
                    .collect()
            )
        }
    }
}

impl Parser {
    fn new() -> Self {
        Parser {
            arena: Vec::new(),
            frames: Vec::new(),
            key: String::new(),
            strbuf: String::new(),
            in_string: false,
        }
    }

    fn alloc(&mut self, node: PlnRaw, _key: Option<&str>) -> usize {
        let idx = self.arena.len();
        self.arena.push(node);
        idx
    }

    fn add_to_top(&mut self, child: usize) {
        let parent = self.frames.last().copied();
        match parent {
            None => {
                self.frames.push(child);
            }
            Some(p) => {
                let key = mem::take(&mut self.key);
                match &mut self.arena[p] {
                    PlnRaw::Obj(ref mut children) => {
                        children.push((key, child));
                    }
                    PlnRaw::Arr(ref mut children) => {
                        children.push(child);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    fn pop_layers(&mut self, n: usize) {
        let n = n.min(self.frames.len().saturating_sub(1));
        for _ in 0..n { self.frames.pop(); }
    }

    fn parse(&mut self, text: &str) -> Result<PlnValue, String> {
        for line in text.lines() {
            let line = line.trim_end_matches('\r');

            if self.in_string {
                match self.handle_string_line(line)? {
                    Some((val_str, n_pop)) => {
                        self.in_string = false;
                        self.strbuf.clear();
                        let idx = self.alloc(PlnRaw::String(val_str), None);
                        self.add_to_top(idx);
                        self.pop_layers(n_pop);
                    }
                    None => {}
                }
                continue;
            }

            if line.is_empty() {
                if !self.frames.is_empty() {
                    return Err("empty line not allowed in message body".to_string());
                }
                continue;
            }

            // Root level
            if self.frames.is_empty() {
                if line.len() > 1 && line.as_bytes()[0] == b'[' {
                    let trimmed = line[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(line)?;
                        continue;
                    }
                }
                match line {
                    "{" => { self.frames.push(alloc_obj(&mut self.arena)); continue; }
                    "[" => { self.frames.push(alloc_arr(&mut self.arena)); continue; }
                    _ => {
                        match parse_scalar(line, self)? {
                            Some(raw) => {
                                let idx = self.alloc(raw, None);
                                return Ok(arena_to_value(&mut self.arena, idx));
                            }
                            None => return Err("multi-line string at root not supported".to_string()),
                        }
                    }
                }
            }

            let is_obj = matches!(self.arena[*self.frames.last().unwrap()], PlnRaw::Obj(_));

            if !is_obj && line.len() > 1 {
                let bytes = line.as_bytes();
                if bytes[0] == b'[' || bytes[0] == b'{' {
                    let trimmed = line[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(line)?;
                        continue;
                    }
                }
            }

            match line {
                "{" => {
                    let idx = alloc_obj(&mut self.arena);
                    self.add_to_top(idx);
                    self.frames.push(idx);
                }
                "[" => {
                    let idx = alloc_arr(&mut self.arena);
                    self.add_to_top(idx);
                    self.frames.push(idx);
                }
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
        while self.frames.len() > 1 {
            self.frames.pop();
        }
        self.frames
            .pop()
            .map(|root| arena_to_value(&mut self.arena, root))
            .ok_or("empty input".to_string())
    }

    fn parse_inline_containers(&mut self, s: &str) -> Result<(), String> {
        let part = s.trim();
        let mut pos = part;
        while !pos.is_empty() {
            let ch = pos.as_bytes()[0];
            let idx = if ch == b'{' { alloc_obj(&mut self.arena) } else { alloc_arr(&mut self.arena) };
            if self.frames.is_empty() {
                self.frames.push(idx);
            } else {
                self.add_to_top(idx);
                self.frames.push(idx);
            }
            pos = pos[1..].trim_start();
        }
        Ok(())
    }

    fn parse_object_line(&mut self, rest: &str) -> Result<(), String> {
        let sep = rest.find(": ")
            .ok_or_else(|| format!("object line must be 'key: value': '{}'", rest))?;
        let key = &rest[..sep];
        if !is_key_valid(key) {
            return Err(format!("invalid key: '{}'", key));
        }
        let val_part = &rest[sep + 2..];
        self.key = key.to_string();

        match val_part {
            "{" => {
                let idx = alloc_obj(&mut self.arena);
                self.add_to_top(idx);
                self.frames.push(idx);
                return Ok(());
            }
            "[" => {
                let idx = alloc_arr(&mut self.arena);
                self.add_to_top(idx);
                self.frames.push(idx);
                return Ok(());
            }
            _ => {}
        }

        let (val, n_pop) = if val_part.as_bytes()[0] != b'{' && val_part.as_bytes()[0] != b'[' {
            fwd_trim_pop_suffix(val_part)
        } else {
            (val_part, 0)
        };

        match parse_scalar(val, self)? {
            Some(raw) => {
                let idx = self.alloc(raw, None);
                self.add_to_top(idx);
                self.pop_layers(n_pop);
            }
            None => {}
        }
        Ok(())
    }

    fn parse_array_line(&mut self, rest: &str) -> Result<(), String> {
        let (trimmed, n_pop) = if rest.as_bytes()[0] != b'{' && rest.as_bytes()[0] != b'[' {
            fwd_trim_pop_suffix(rest)
        } else {
            (rest, 0)
        };

        match parse_scalar(trimmed, self)? {
            Some(raw) => {
                let idx = self.alloc(raw, None);
                self.add_to_top(idx);
                self.pop_layers(n_pop);
            }
            None => {}
        }
        Ok(())
    }

    fn handle_string_line(&mut self, line: &str) -> Result<Option<(String, usize)>, String> {
        let line_bytes = line.as_bytes();
        let mut start = 0usize;
        for i in 0..line.len() {
            if line_bytes[i] == b'"' {
                if i + 1 < line.len() && line_bytes[i + 1] == b'"' {
                    self.strbuf.push_str(&line[start..=i]);
                    start = i + 2;
                    continue;
                }
                // Closing quote
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
// Pop suffix detection
// ---------------------------------------------------------------------------

fn fwd_trim_pop_suffix<'a>(s: &'a str) -> (&'a str, usize) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    for i in 0..len {
        if bytes[i] == b'"' { continue; } /* don't track in_string, just skip */
        if bytes[i] == b' ' {
            let mut all_digits = true;
            for j in i + 1..len {
                if !bytes[j].is_ascii_digit() { all_digits = false; break; }
            }
            if all_digits && i + 1 < len {
                let n: usize = s[i + 1..len].parse().unwrap_or(0);
                return (&s[..i], n);
            }
        }
    }
    (s, 0)
}

fn pop_suffix_after(s: &str) -> Result<usize, String> {
    if s.is_empty() { return Ok(0); }
    let bytes = s.as_bytes();
    if bytes[0] != b' ' { return Err(format!("extra after closing quote: '{}'", s)); }
    if bytes.len() < 2 || !bytes[1].is_ascii_digit() {
        return Err(format!("extra after closing quote: '{}'", s));
    }
    let mut n: usize = 0;
    for &b in &bytes[1..] {
        if !b.is_ascii_digit() { return Err(format!("extra after closing quote: '{}'", s)); }
        n = n * 10 + ((b - b'0') as usize);
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Value / string parsing
// ---------------------------------------------------------------------------

fn parse_scalar(s: &str, p: &mut Parser) -> Result<Option<PlnRaw>, String> {
    if s.is_empty() { return Err("empty value".to_string()); }
    let bytes = s.as_bytes();
    if bytes[0] == b'"' { return parse_quoted(&s[1..], p); }
    match s {
        "true" => return Ok(Some(PlnRaw::Bool(true))),
        "false" => return Ok(Some(PlnRaw::Bool(false))),
        "null" => return Ok(Some(PlnRaw::Null)),
        _ => {}
    }
    if bytes[0] == b'-' || bytes[0].is_ascii_digit() {
        if s.contains('.') || s.contains('e') || s.contains('E') {
            if let Ok(f) = s.parse::<f64>() {
                return Ok(Some(PlnRaw::Float(f)));
            }
        } else if let Ok(n) = s.parse::<i64>() {
            return Ok(Some(PlnRaw::Int(n)));
        }
    }
    Err(format!("bare string must be quoted: '{}'", s))
}

fn parse_quoted(content: &str, p: &mut Parser) -> Result<Option<PlnRaw>, String> {
    let bytes = content.as_bytes();
    let mut result = String::with_capacity(content.len());
    let mut i = 0;
    while i < content.len() {
        if bytes[i] == b'"' {
            if i + 1 < content.len() && bytes[i + 1] == b'"' {
                result.push('"');
                i += 2;
            } else {
                let trailing = &content[i + 1..];
                if !trailing.trim().is_empty() {
                    return Err(format!("extra after closing quote: '{}'", trailing));
                }
                return Ok(Some(PlnRaw::String(result)));
            }
        } else {
            result.push(content[i..].chars().next().unwrap());
            i += 1;
        }
    }
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
    if key.is_empty() { return false; }
    for &b in key.as_bytes() {
        match b {
            b':' | b'"' | b'{' | b'[' | b'#' | b' ' | b'\t' | b'\n' | b'\r' => return false,
            _ => {}
        }
    }
    true
}
