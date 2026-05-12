use crate::PlnValue;

pub fn to_string(value: &PlnValue) -> String {
    let mut g = Generator::new();
    g.write_value(value);
    g.buf
}

struct Generator {
    buf: String,
    stack: Vec<u8>,
    pending_pop: usize,
    need_key: bool,
    awaiting_value: bool,
}

impl Generator {
    fn new() -> Self {
        Generator { buf: String::new(), stack: Vec::new(), pending_pop: 0, need_key: false, awaiting_value: false }
    }

    fn write_value(&mut self, v: &PlnValue) {
        match v {
            PlnValue::Object(ref obj) => {
                self.start_container(b'{');
                self.stack.push(b'o');
                self.need_key = true;
                self.awaiting_value = false;
                for (key, val) in obj {
                    self.flush_pop();
                    self.buf.push_str(key);
                    self.buf.push_str(": ");
                    self.need_key = false;
                    self.awaiting_value = true;
                    self.write_value(val);
                }
                self.stack.pop();
                self.pending_pop += 1;
                if self.top() == b'o' { self.need_key = true; }
            }
            PlnValue::Array(_) => {
                self.write_container_inline(v, true);
            }
            PlnValue::Null => self.put_scalar("null"),
            PlnValue::Bool(b) => self.put_scalar(if *b { "true" } else { "false" }),
            PlnValue::Int(n) => self.put_scalar(&n.to_string()),
            PlnValue::Float(f) => self.put_scalar(&format!("{}", f)),
            PlnValue::String(s) => self.put_string(s),
        }
    }

    fn start_container(&mut self, ch: u8) {
        if self.top() == b'o' && self.awaiting_value {
            self.buf.push(ch as char);
            self.awaiting_value = false;
        } else {
            self.flush_pop();
            self.buf.push(ch as char);
        }
        self.buf.push('\n');
    }

    /// Write a container with inline first-child support.
    /// `v` is the PlnValue (Object or Array), `first` indicates if this is the first in the chain.
    /// The `ch` and `typ` are ignored when not `first` (inferred from v).
    fn write_container_inline(&mut self, v: &PlnValue, first: bool) {
        let (ch, typ) = match v {
            PlnValue::Object(_) => (b'{', b'o'),
            PlnValue::Array(_) => (b'[', b'a'),
            _ => return,
        };

        if first && self.top() == b'o' && self.awaiting_value {
            self.buf.push(ch as char);
            self.awaiting_value = false;
        } else if first {
            self.flush_pop();
            self.buf.push(ch as char);
        } else {
            self.buf.push(ch as char);
        }

        let can_inline = if let PlnValue::Array(ref arr) = v {
            arr.len() > 0 && (matches!(arr[0], PlnValue::Object(_)) || matches!(arr[0], PlnValue::Array(_)))
        } else {
            false
        };

        if can_inline {
            self.stack.push(b'a');
            self.need_key = false;
            self.awaiting_value = false;
            if let PlnValue::Array(ref arr) = v {
                self.write_container_inline(&arr[0], false);
                for val in &arr[1..] { self.write_value(val); }
            }
            self.stack.pop();
            self.pending_pop += 1;
            if self.top() == b'o' { self.need_key = true; }
        } else {
            self.buf.push('\n');
            self.stack.push(typ);
            self.need_key = typ == b'o';
            self.awaiting_value = false;
            match v {
                PlnValue::Object(ref obj) => {
                    for (key, val) in obj {
                        self.flush_pop();
                        self.buf.push_str(key);
                        self.buf.push_str(": ");
                        self.need_key = false;
                        self.awaiting_value = true;
                        self.write_value(val);
                    }
                }
                PlnValue::Array(ref arr) => {
                    for val in arr { self.write_value(val); }
                }
                _ => {}
            }
            self.stack.pop();
            self.pending_pop += 1;
            if self.top() == b'o' { self.need_key = true; }
        }
    }

    fn put_scalar(&mut self, s: &str) {
        self.buf.push_str(s);
        self.flush_pop();
        self.buf.push('\n');
        if self.top() == b'o' {
            self.awaiting_value = false;
            self.need_key = true;
        }
    }

    fn put_string(&mut self, s: &str) {
        if self.top() == b'o' {
            self.awaiting_value = false;
        }
        self.buf.push('"');
        for c in s.chars() {
            self.buf.push(c);
            if c == '"' { self.buf.push('"'); }
        }
        self.buf.push('"');
        self.flush_pop();
        self.buf.push('\n');
        if self.top() == b'o' {
            self.need_key = true;
        }
    }

    fn flush_pop(&mut self) {
        if self.pending_pop > 0 {
            self.buf.push_str(&self.pending_pop.to_string());
            self.buf.push(' ');
            self.pending_pop = 0;
        }
    }

    fn top(&self) -> u8 {
        *self.stack.last().unwrap_or(&0)
    }
}
