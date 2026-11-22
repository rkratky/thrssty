//! End-to-end checks that feed fetching survives feeds whose bytes are not valid
//! UTF-8. These serve the feed from a loopback HTTP server so the real ureq
//! body-reading path is exercised, not just the parser.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn serve_once(body: Vec<u8>, content_type: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });
    format!("http://{addr}/feed.xml")
}

#[test]
fn latin1_feed_served_as_application_rss_is_decoded() {
    let body = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\
<rss version=\"2.0\"><channel><title>Caf\xe9</title></channel></rss>"
        .to_vec();
    let url = serve_once(body, "application/rss+xml");
    let title = thrssty::feed::fetcher::probe_feed(&url).expect("latin-1 feed should load");
    assert_eq!(title.as_deref(), Some("Caf\u{e9}"));
}

#[test]
fn invalid_utf8_without_declaration_still_loads() {
    // No encoding declaration and an invalid UTF-8 byte: this used to load with
    // a replacement character, and must keep loading rather than failing.
    let body = b"<?xml version=\"1.0\"?>\
<rss version=\"2.0\"><channel><title>Bad \xff Byte</title></channel></rss>"
        .to_vec();
    let url = serve_once(body, "application/xml");
    let title = thrssty::feed::fetcher::probe_feed(&url).expect("feed with bad byte should load");
    assert!(title.is_some(), "title should be present, got {title:?}");
}
