use popline_rust::{parse, serialize, PlnValue};

#[test]
fn test_basic_types() {
    let v = parse("{\nname: \"popline\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("{\na: 42\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("{\na: 3.14\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("{\na: true\nb: false\nc: null\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());
}

#[test]
fn test_nesting() {
    let v = parse("{\nouter: {\ninner: \"value\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("[\n[\n1\n2\n1 [\n3\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());
}

#[test]
fn test_pop() {
    let v = parse("{\nouter: {\ninner: \"x\"\n1 mid: \"y\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("{\na: {\nb: {\nc: \"deep\"\n2 x: \"top\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());
}

#[test]
fn test_strings() {
    let v = parse("{\nmsg: \"He said: \"\"Hello\"\"\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());

    let v = parse("{\nkey: \"你好世界\"\n").unwrap();
    assert_eq!(v, parse(&serialize(&v)).unwrap());
}

#[test]
fn test_roundtrip_complex() {
    let input = "{\nname: \"test\"\nversion: 2\nactive: true\ntags: [\n\"web\"\n\"primary\"\n1 nested: {\nkey: \"val\"\n1 msg: \"He said: \"\"Hi\"\"\"\n";
    let v = parse(input).unwrap();
    let s = serialize(&v);
    let v2 = parse(&s).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn test_array_roundtrip() {
    let input = "[\n1\n2\n3\n[\n4\n5\n";
    let v = parse(input).unwrap();
    let s = serialize(&v);
    let v2 = parse(&s).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn test_errors() {
    assert!(parse("42\n").is_err());
    assert!(parse("\"str\"\n").is_err());
    assert!(parse("true\n").is_err());
    assert!(parse("{\nbad:key: 1\n").is_err());
    assert!(parse("{\n\"key\": 1\n").is_err());
}
