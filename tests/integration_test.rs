use pln::{from_str, to_string, PlnValue};
use std::fs;
use std::time::Instant;

fn json_value(v: &PlnValue) -> serde_json::Value {
    match v {
        PlnValue::Null => serde_json::Value::Null,
        PlnValue::Bool(b) => serde_json::Value::Bool(*b),
        PlnValue::Int(n) => serde_json::Value::Number((*n).into()),
        PlnValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        PlnValue::String(s) => serde_json::Value::String(s.clone()),
        PlnValue::Object(obj) => {
            let m: serde_json::Map<_, _> = obj.iter().map(|(k, v)| (k.clone(), json_value(v))).collect();
            serde_json::Value::Object(m)
        }
        PlnValue::Array(arr) => serde_json::Value::Array(arr.iter().map(json_value).collect()),
    }
}

// ═══════════════ Unit Tests ═══════════════

#[test]
fn test_basic_types() {
    let v = from_str("{\nname: \"popline\"\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());

    let v = from_str("{\na: 42\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());
}

#[test]
fn test_nesting() {
    let v = from_str("{\nouter: {\ninner: \"value\"\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());
}

#[test]
fn test_pop() {
    // Prefix-style pops (still supported for containers and key:value lines)
    let v = from_str("{\nouter: {\ninner: \"x\"\n1 mid: \"y\"\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());

    let v = from_str("{\na: {\nb: {\nc: \"deep\"\n2 x: \"top\"\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());

    // Suffix-style pops (new format for leaf values)
    let v = from_str("{\nouter: {\ninner: \"x\"\nmid: \"other\" 1\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());

    let v = from_str("{\na: {\nb: {\nc: \"deep\"\nx: \"top\" 2\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());

    // Array element with suffix pop (pops the array, next line at parent level)
    let v = from_str("{\na: [\n1\n2 1\nb: true\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());
}

#[test]
fn test_strings() {
    let v = from_str("{\nmsg: \"He said: \"\"Hello\"\"\"\n").unwrap();
    assert_eq!(v, from_str(&to_string(&v)).unwrap());
}

#[test]
fn test_errors() {
    assert!(from_str("42\n").is_err());
    assert!(from_str("\"str\"\n").is_err());
    assert!(from_str("true\n").is_err());
    assert!(from_str("{\nbad:key: 1\n").is_err());
    assert!(from_str("{\n\"key\": 1\n").is_err());
}

// ═══════════════ Real Data Consistency ═══════════════

#[test]
fn test_real_data_consistency() {
    let json_text = match fs::read_to_string("package.json") {
        Ok(t) => t,
        Err(_) => { eprintln!("  SKIP: package.json not found"); return; }
    };
    let pln_text = match fs::read_to_string("package.pln") {
        Ok(t) => t,
        Err(_) => { eprintln!("  SKIP: package.pln not found"); return; }
    };

    let json_obj: serde_json::Value = serde_json::from_str(&json_text).unwrap();
    let pln_val = from_str(&pln_text).unwrap();
    let pln_as_json = json_value(&pln_val);

    assert_eq!(pln_as_json, json_obj, "PopLine vs JSON mismatch");

    let s = to_string(&pln_val);
    let v2 = from_str(&s).unwrap();
    assert_eq!(pln_val, v2, "PopLine roundtrip mismatch");

    println!("  data: JSON={}B, PopLine={}B ({:.1}%)",
        json_text.len(), pln_text.len(),
        pln_text.len() as f64 / json_text.len() as f64 * 100.0);
}

// ═══════════════ Performance Benchmark ═══════════════

#[test]
fn test_benchmark() {
    let json_text = match fs::read_to_string("package.json") {
        Ok(t) => t,
        Err(_) => { eprintln!("  SKIP: package.json not found"); return; }
    };
    let pln_text = match fs::read_to_string("package.pln") {
        Ok(t) => t,
        Err(_) => { eprintln!("  SKIP: package.pln not found"); return; }
    };

    let json_obj: serde_json::Value = serde_json::from_str(&json_text).unwrap();
    let pln_val = from_str(&pln_text).unwrap();
    let N = 5000;

    println!("\n── Performance Benchmark ({} iterations) ──", N);

    fn bench<F: FnMut()>(label: &str, mut f: F, n: usize) -> f64 {
        f();
        let start = Instant::now();
        for _ in 0..n { f(); }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let us = ms * 1000.0 / n as f64;
        println!("  {:26} {:8.1} ms  {:8.1} us/op", label, ms, us);
        ms
    }

    let js_ser = bench("serde_json::to_string", || {
        serde_json::to_string(&json_obj).unwrap();
    }, N);
    let pl_ser = bench("to_string", || {
        to_string(&pln_val);
    }, N);
    println!("  {:26} {:7.2}x", "PopLine/JSON", pl_ser / js_ser);

    let js_par = bench("serde_json::from_str", || {
        let _: serde_json::Value = serde_json::from_str(&json_text).unwrap();
    }, N);
    let pl_par = bench("from_str", || {
        from_str(&pln_text).unwrap();
    }, N);
    println!("  {:26} {:7.2}x", "PopLine/JSON", pl_par / js_par);
}
