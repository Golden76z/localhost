use localhost::http::{ParseError, Parser};

#[test]
fn parses_simple_get() {
    let mut p = Parser::new(1024);
    let req = p
        .feed(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap()
        .expect("complete request");
    assert_eq!(req.method, "GET");
    assert_eq!(req.uri, "/");
}

#[test]
fn rejects_oversized_body() {
    let mut p = Parser::new(4);
    let err = p
        .feed(
            b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\n",
        )
        .unwrap_err();
    assert!(matches!(err, ParseError::BodyTooLarge));
}

#[test]
fn decodes_chunked_body() {
    let raw = b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nRust\r\n0\r\n\r\n";
    let mut p = Parser::new(1024);
    let req = p.feed(raw).unwrap().expect("chunked");
    assert_eq!(req.body, b"Rust");
}
