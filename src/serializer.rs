use crate::PlnValue;

pub fn to_string(value: &PlnValue) -> String {
    let mut g = Generator::new();
    g.write_value(value, 0);
    g.buf
}

struct Generator {
    buf: String,
    stack: Vec<u8>,
    need_key: bool,
    awaiting_value: bool,
}

impl Generator {
    fn new() -> Self {
        Generator { buf: String::new(), stack: Vec::new(), need_key: false, awaiting_value: false }
    }

    fn write_value(&mut self, v: &PlnValue, close_pop: usize) {
        match v {
            PlnValue::Object(ref obj) => {
                self.start_container(b'{');
                self.stack.push(b'o');
                self.need_key = true;
                self.awaiting_value = false;
                let n = obj.len();
                for (i, (key, val)) in obj.iter().enumerate() {
                    self.buf.push_str(key);
                    self.buf.push_str(": ");
                    self.need_key = false;
                    self.awaiting_value = true;
                    let child_pop = if i == n - 1  { close_pop + 1 } else { 0 };
                    self.write_value(val, child_pop);
                }
                self.stack.pop();
                if self.top() == b'o' { self.need_key = true; }
            }
            PlnValue::Array(_) => {
                self.write_container_inline(v, true, close_pop);
            }
            PlnValue::Null => self.put_scalar("null", close_pop),
            PlnValue::Bool(b) => self.put_scalar(if *b { "true" } else { "false" }, close_pop),
            PlnValue::Int(n) => self.put_scalar(&n.to_string(), close_pop),
            PlnValue::Float(f) => self.put_scalar(&format!("{}", f), close_pop),
            PlnValue::String(s) => self.put_string(s, close_pop),
        }
    }

    fn start_container(&mut self, ch: u8) {
        if self.top() == b'o' && self.awaiting_value {
            self.buf.push(ch as char);
            self.awaiting_value = false;
        } else {
            self.buf.push(ch as char);
        }
        self.buf.push('\n');
    }

    fn write_container_inline(&mut self, v: &PlnValue, first: bool, close_pop: usize) {
        let (ch, typ) = match v {
            PlnValue::Object(_) => (b'{', b'o'),
            PlnValue::Array(_) => (b'[', b'a'),
            _ => return,
        };

        if first && self.top() == b'o' && self.awaiting_value {
            self.buf.push(ch as char);
            self.awaiting_value = false;
        } else if first {
            self.buf.push(ch as char);
        } else {
            self.buf.push(ch as char);
        }

        // Non-inline path for correct close_pop propagation
        self.buf.push('\n');
        self.stack.push(typ);
        self.need_key = typ == b'o';
        self.awaiting_value = false;
        match v {
            PlnValue::Object(ref obj) => {
                let n = obj.len();
                for (i, (key, val)) in obj.iter().enumerate() {
                    let child_pop = if i == n - 1  { close_pop + 1 } else { 0 };
                    self.buf.push_str(key);
                    self.buf.push_str(": ");
                    self.need_key = false;
                    self.awaiting_value = true;
                    self.write_value(val, child_pop);
                }
            }
            PlnValue::Array(ref arr) => {
                let n = arr.len();
                for (i, val) in arr.iter().enumerate() {
                    let child_pop = if i == n - 1  { close_pop + 1 } else { 0 };
                    self.write_value(val, child_pop);
                }
            }
            _ => {}
        }
        self.stack.pop();
        if self.top() == b'o' { self.need_key = true; }
    }

    fn put_scalar(&mut self, s: &str, close_pop: usize) {
        if self.top() == b'o' {
            self.awaiting_value = false;
        }
        self.buf.push_str(s);
        if close_pop > 0 {
            self.buf.push_str(&format!(" {}", close_pop));
        }
        self.buf.push('\n');
        if self.top() == b'o' {
            self.need_key = true;
        }
    }

    fn put_string(&mut self, s: &str, close_pop: usize) {
        if self.top() == b'o' {
            self.awaiting_value = false;
        }
        self.buf.push('"');
        for c in s.chars() {
            self.buf.push(c);
            if c == '"' { self.buf.push('"'); }
        }
        self.buf.push('"');
        if close_pop > 0 {
            self.buf.push_str(&format!(" {}", close_pop));
        }
        self.buf.push('\n');
        if self.top() == b'o' {
            self.need_key = true;
        }
    }

    fn top(&self) -> u8 {
        *self.stack.last().unwrap_or(&0)
    }
}