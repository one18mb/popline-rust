use crate::PlnValue;

pub fn to_string(value: &PlnValue) -> String {
    let mut g = Generator { buf: String::new(), stack: Vec::new(), awaiting_value: false };
    g.write_value(value, 0);
    g.buf
}

struct Generator {
    buf: String,
    stack: Vec<u8>,
    awaiting_value: bool,
}

impl Generator {
    fn write_value(&mut self, v: &PlnValue, close_pop: usize) {
        match v {
            PlnValue::Object(ref obj) => {
                if self.top() == b'o' && self.awaiting_value {
                    self.buf.push('{');
                    self.awaiting_value = false;
                } else {
                    self.buf.push('{');
                }
                self.buf.push('\n');
                self.stack.push(b'o');
                self.awaiting_value = false;
                let n = obj.len();
                for (i, (key, val)) in obj.iter().enumerate() {
                    self.buf.push_str(key);
                    self.buf.push_str(": ");
                    self.awaiting_value = true;
                    let child_pop = if i == n - 1 { close_pop + 1 } else { 0 };
                    self.write_value(val, child_pop);
                }
                self.stack.pop();
            }
            PlnValue::Array(arr) => {
                let (ch, typ) = (b'[', b'a');
                if self.top() == b'o' && self.awaiting_value {
                    self.buf.push(ch as char);
                    self.awaiting_value = false;
                } else {
                    self.buf.push(ch as char);
                }
                self.buf.push('\n');
                self.stack.push(typ);
                self.awaiting_value = false;
                let n = arr.len();
                for (i, val) in arr.iter().enumerate() {
                    let child_pop = if i == n - 1 { close_pop + 1 } else { 0 };
                    self.write_value(val, child_pop);
                }
                self.stack.pop();
            }
            PlnValue::Null => self.put_scalar("null", close_pop),
            PlnValue::Bool(b) => self.put_scalar(if *b { "true" } else { "false" }, close_pop),
            PlnValue::Int(n) => {
                if self.top() == b'o' { self.awaiting_value = false; }
                self.buf.push_str(&itoa(*n));
                if close_pop > 0 { self.push_pop(close_pop); }
                self.buf.push('\n');
            }
            PlnValue::Float(f) => self.put_scalar(&format!("{}", f), close_pop),
            PlnValue::String(s) => self.put_string(s, close_pop),
        }
    }

    fn put_scalar(&mut self, s: &str, close_pop: usize) {
        if self.top() == b'o' { self.awaiting_value = false; }
        self.buf.push_str(s);
        if close_pop > 0 { self.push_pop(close_pop); }
        self.buf.push('\n');
    }

    fn put_string(&mut self, s: &str, close_pop: usize) {
        if self.top() == b'o' { self.awaiting_value = false; }
        self.buf.push('"');
        for c in s.chars() {
            self.buf.push(c);
            if c == '"' { self.buf.push('"'); }
        }
        self.buf.push('"');
        if close_pop > 0 { self.push_pop(close_pop); }
        self.buf.push('\n');
    }

    fn push_pop(&mut self, n: usize) {
        if n < 10 { let mut d = [0u8; 2]; d[0] = b' '; d[1] = b'0' + n as u8; self.buf.push_str(std::str::from_utf8(&d[..2]).unwrap()); }
        else if n < 100 { let mut d = [0u8; 3]; d[0] = b' '; d[1] = b'0' + (n/10) as u8; d[2] = b'0' + (n%10) as u8; self.buf.push_str(std::str::from_utf8(&d[..3]).unwrap()); }
        else { let s = format!(" {}", n); self.buf.push_str(&s); }
    }

    fn top(&self) -> u8 { *self.stack.last().unwrap_or(&0) }
}

fn itoa(n: i64) -> String {
    if n == 0 { return "0".into(); }
    let neg = n < 0;
    let mut d = if neg { (-n) as u64 } else { n as u64 };
    let mut buf = Vec::with_capacity(20);
    while d > 0 { buf.push(b'0' as u8 + (d % 10) as u8); d /= 10; }
    if neg { buf.push(b'-'); }
    buf.reverse();
    unsafe { String::from_utf8_unchecked(buf) }
}
