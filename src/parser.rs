use crate::PlnValue;
use std::mem;
use memchr::memchr;

// ---------------------------------------------------------------------------
// Direct PlnValue builder
// ---------------------------------------------------------------------------

enum BuildState { Obj(Vec<(String, PlnValue)>), Arr(Vec<PlnValue>) }
struct Frame { state: BuildState, key: Option<String> }

struct Parser { stack: Vec<Frame>, key: String, strbuf: String, in_string: bool }

pub fn from_str(text: &str) -> Result<PlnValue, String> {
    let mut p = Parser { stack: Vec::new(), key: String::new(), strbuf: String::new(), in_string: false };
    p.parse(text)
}

impl Parser {
    fn push_child(&mut self, val: PlnValue) {
        if let Some(frame) = self.stack.last_mut() {
            match &mut frame.state {
                BuildState::Obj(ref mut c) => c.push((mem::take(&mut self.key), val)),
                BuildState::Arr(ref mut c) => c.push(val),
            }
        }
    }

    fn open(&mut self, is_obj: bool) {
        let state = if is_obj { BuildState::Obj(Vec::new()) } else { BuildState::Arr(Vec::new()) };
        let key = if self.stack.is_empty() { None } else { Some(mem::take(&mut self.key)) };
        self.stack.push(Frame { state, key });
    }

    fn close(&mut self) -> Option<(PlnValue, Option<String>)> {
        self.stack.pop().map(|f| {
            let v = match f.state { BuildState::Obj(c) => PlnValue::Object(c), BuildState::Arr(c) => PlnValue::Array(c) };
            (v, f.key)
        })
    }

    fn pop_layers(&mut self, n: usize) {
        for _ in 0..n.min(self.stack.len().saturating_sub(1)) {
            if let Some((v, k)) = self.close() { self.key = k.unwrap_or_default(); self.push_child(v); }
        }
    }

    fn parse(&mut self, text: &str) -> Result<PlnValue, String> {
        let bytes = text.as_bytes();
        let text_len = text.len();
        let mut pos = 0;

        while pos < text_len {
            if self.in_string {
                let start = pos;
                while pos < text_len && bytes[pos] != b'"' { pos += 1; }
                if pos > start { self.strbuf.push_str(&text[start..pos]); }
                if pos >= text_len { break; }
                if pos + 1 < text_len && bytes[pos + 1] == b'"' {
                    self.strbuf.push('"'); pos += 2; continue;
                }
                self.in_string = false;
                let trailing = &text[pos + 1..];
                pos += 1;
                let nl = memchr(b'\n', trailing.as_bytes()).unwrap_or(trailing.len());
                let trail = &trailing[..nl];
                let n_pop = if trail.trim().is_empty() { 0 } else { pop_suffix_after(trail)? };
                let s = mem::take(&mut self.strbuf);
                self.push_child(PlnValue::String(s));
                self.pop_layers(n_pop);
                if nl < trailing.len() { pos += nl + 1; }
                continue;
            }

            let nl = match memchr(b'\n', &bytes[pos..]) {
                Some(offset) => pos + offset,
                None => text_len,
            };
            let line = &text[pos..nl];
            let line = if nl > pos && bytes[nl - 1] == b'\r' { &text[pos..nl - 1] } else { line };
            pos = nl + 1;

            if line.is_empty() {
                if !self.stack.is_empty() { return Err("empty line not allowed in message body".into()); }
                continue;
            }

            let is_root = self.stack.is_empty();
            let is_obj = !is_root && matches!(self.stack.last().unwrap().state, BuildState::Obj(_));

            if line.len() > 1 {
                let b0 = line.as_bytes()[0];
                if b0 == b'[' || (!is_root && !is_obj && b0 == b'{') {
                    let trimmed = line[1..].trim_start();
                    if trimmed.len() > 0 && (trimmed.as_bytes()[0] == b'[' || trimmed.as_bytes()[0] == b'{') {
                        self.parse_inline_containers(line)?; continue;
                    }
                }
            }

            match line {
                "{" => { self.open(true); if is_root { continue; } }
                "[" => { self.open(false); if is_root { continue; } }
                _ => {
                    if is_root { return self.parse_scalar_root(line); }
                    if is_obj { self.parse_object_line(line)?; }
                    else { self.parse_array_line(line)?; }
                }
            }
        }

        if self.in_string { return Err("unclosed string at end of input".into()); }
        while self.stack.len() > 1 {
            if let Some((v, k)) = self.close() { self.key = k.unwrap_or_default(); self.push_child(v); }
        }
        self.close().map(|(v, _)| v).ok_or("empty input".into())
    }

    fn parse_inline_containers(&mut self, s: &str) -> Result<(), String> {
        let mut pos = s.trim();
        while !pos.is_empty() { self.open(pos.as_bytes()[0] == b'{'); pos = pos[1..].trim_start(); }
        Ok(())
    }

    fn parse_object_line(&mut self, rest: &str) -> Result<(), String> {
        let bytes = rest.as_bytes();
        let mut key_end = None;
        for i in 0..bytes.len().saturating_sub(1) {
            let b = bytes[i];
            if b == b':' && bytes[i + 1] == b' ' { key_end = Some(i); break; }
            if b == b':' || b == b'"' || b == b'{' || b == b'[' || b == b'#' || b == b' ' || b == b'\t' {
                return Err("invalid key".into());
            }
        }
        let sep = key_end.ok_or_else(|| format!("missing 'key: value': '{}'", rest))?;
        self.key = rest[..sep].to_string();
        let val_part = &rest[sep + 2..];
        match val_part { "{" => { self.open(true); return Ok(()); } "[" => { self.open(false); return Ok(()); } _ => {} }
        let (pv, pop) = self.parse_one(val_part)?;
        self.push_child(pv);
        self.pop_layers(pop);
        Ok(())
    }

    fn parse_array_line(&mut self, rest: &str) -> Result<(), String> {
        match rest { "{" => { self.open(true); return Ok(()); } "[" => { self.open(false); return Ok(()); } _ => {} }
        let (pv, pop) = self.parse_one(rest)?;
        self.push_child(pv);
        self.pop_layers(pop);
        Ok(())
    }

    // -------------------------------------------------------------------
    // Single-pass value + pop suffix + number parsing
    // -------------------------------------------------------------------

    /// Parse value + detect " N" pop suffix in ONE pass over the bytes.
    /// Returns (PlnValue, pop_count). For strings, returns None if multi-line.
    fn parse_one(&mut self, s: &str) -> Result<(PlnValue, usize), String> {
        let bytes = s.as_bytes();
        if bytes.is_empty() { return Err("empty value".into()); }

        // String
        if bytes[0] == b'"' {
            let mut result = String::with_capacity(s.len());
            let mut i = 1;
            while i < s.len() {
                if bytes[i] == b'"' {
                    if i + 1 < s.len() && bytes[i + 1] == b'"' { result.push('"'); i += 2; continue; }
                    break; // closing quote
                }
                result.push(bytes[i] as char); i += 1;
            }
            if i >= s.len() {
                self.in_string = true; self.strbuf = result; self.strbuf.push('\n');
                return Ok((PlnValue::Null, 0));
            }
            let after = &s[i + 1..];
            let pop = if after.trim().is_empty() { 0 } else { pop_suffix_after(after)? };
            return Ok((PlnValue::String(result), pop));
        }

        // Pop suffix: reverse scan (O(1) for 95% no-pop)
        let len = bytes.len();
        let (ve, pop) = if len >= 2 && bytes[len - 1].is_ascii_digit() {
            let mut i = len - 1;
            while i > 0 && bytes[i - 1].is_ascii_digit() { i -= 1; }
            if i > 0 && bytes[i - 1] == b' ' {
                let mut n = 0usize;
                for &b in &bytes[i..] { n = n * 10 + (b - b'0') as usize; }
                (i - 1, n)
            } else { (len, 0) }
        } else { (len, 0) };

        let val = &s[..ve];
        let vb = &bytes[..ve];

        // Keywords
        match val {
            "true" => return Ok((PlnValue::Bool(true), pop)),
            "false" => return Ok((PlnValue::Bool(false), pop)),
            "null" => return Ok((PlnValue::Null, pop)),
            _ => {}
        }

        // Number: single pass (accumulate int + detect float simultaneously)
        if vb.is_empty() { return Err("empty value".into()); }
        if vb[0] == b'-' || vb[0].is_ascii_digit() {
            let mut i = 0;
            let neg = if vb[0] == b'-' { i = 1; true } else { false };
            if i >= vb.len() || !vb[i].is_ascii_digit() { return Err(format!("bare: '{}'", s)); }
            let mut is_float = false;
            let mut int_val: i64 = 0;
            for &b in &vb[i..] {
                let d = b.wrapping_sub(b'0');
                if d <= 9 { int_val = int_val.wrapping_mul(10).wrapping_add(d as i64); }
                else if b == b'.' || b == b'e' || b == b'E' { is_float = true; break; }
                else { return Err(format!("bare: '{}'", s)); }
            }
            if is_float { return Ok((PlnValue::Float(val.parse().map_err(|_| format!("bad num '{}'", s))?), pop)); }
            return Ok((PlnValue::Int(if neg { -int_val } else { int_val }), pop));
        }
        Err(format!("bare string must be quoted: '{}'", s))
    }


    fn parse_scalar_root(&mut self, s: &str) -> Result<PlnValue, String> {
        let (v, pop) = self.parse_one(s)?;
        if pop > 0 { return Err("pop suffix at root".into()); }
        if self.in_string { return Err("multi-line at root".into()); }
        Ok(v)
    }
}

fn pop_suffix_after(s: &str) -> Result<usize, String> {
    if s.is_empty() { return Ok(0); }
    let bytes = s.as_bytes();
    if bytes[0] != b' ' { return Err(format!("trailing after quote: '{}'", s)); }
    let mut n: usize = 0;
    for &b in &bytes[1..] { if !b.is_ascii_digit() { return Err(format!("trailing after quote: '{}'", s)); } n = n * 10 + (b - b'0') as usize; }
    Ok(n)
}
