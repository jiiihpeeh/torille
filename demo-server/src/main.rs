//! `serve-demo` — a tiny static file server for the wasm demo.
//!
//! Serves `torille-wasm/demo/` (index.html, `dist/`, and the local book
//! corpus under `books/`) with a permissive `Access-Control-Allow-Origin`
//! on every response. There is no proxy: the reader's books are downloaded
//! once by `tools/fetch_finnish_books.py` and served same-origin, so no
//! cross-origin fetch is needed.
//!
//! Usage:
//!   cargo run --release -p torille-demo-server            # root=torille-wasm/demo, :8765
//!   cargo run --release -p torille-demo-server -- --root path --port 9000

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread;

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn main() {
    let mut root = PathBuf::from(".");
    let mut port: u16 = 8765;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().unwrap_or_default()),
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(8765),
            "-h" | "--help" => {
                println!("usage: serve-demo [--root DIR] [--port N]");
                return;
            }
            other => eprintln!("serve-demo: ignoring unknown argument {other:?}"),
        }
    }
    // Default root: the demo, when run from the workspace root.
    if root == Path::new(".") && Path::new("torille-wasm/demo/index.html").exists() {
        root = PathBuf::from("torille-wasm/demo");
    }
    let root: Arc<PathBuf> = Arc::new(root.canonicalize().unwrap_or(root));

    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!(
        "serve-demo: http://{addr}/  (root {}, CORS *)",
        root.display()
    );

    for request in server.incoming_requests() {
        let root = Arc::clone(&root);
        thread::spawn(move || handle(request, &root));
    }
}

fn handle(request: Request, root: &Path) {
    if *request.method() == Method::Options {
        // Preflight: the CORS headers and no body.
        let mut response = Response::empty(StatusCode(204));
        add_cors(&mut response);
        let _ = request.respond(response);
        return;
    }

    let url = request.url().to_string();
    println!("{} {}", request.method(), url);
    serve_static(request, root, &url);
}

fn serve_static(request: Request, root: &Path, url: &str) {
    let path_part = url.split('?').next().unwrap_or("/");
    let rel = percent_decode(path_part.trim_start_matches('/'));

    let mut path = root.to_path_buf();
    for component in Path::new(&rel).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            // `..`, a root, or a prefix: refuse. No traversal above `root`.
            _ => {
                respond(request, 400, "text/plain; charset=utf-8", b"bad path\n");
                return;
            }
        }
    }
    if path.is_dir() || rel.is_empty() {
        path.push("index.html");
    }

    match std::fs::read(&path) {
        Ok(data) => respond(request, 200, content_type(&path), &data),
        Err(_) => respond(request, 404, "text/plain; charset=utf-8", b"not found\n"),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "map" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn respond(request: Request, status: u16, ctype: &str, body: &[u8]) {
    let mut response = Response::from_data(body.to_vec()).with_status_code(StatusCode(status));
    if let Ok(header) = Header::from_bytes("Content-Type", ctype) {
        response.add_header(header);
    }
    add_cors(&mut response);
    let _ = request.respond(response);
}

fn add_cors<R: Read>(response: &mut Response<R>) {
    for (name, value) in [
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Methods", "GET, OPTIONS"),
        ("Access-Control-Allow-Headers", "Content-Type, Accept"),
    ] {
        if let Ok(header) = Header::from_bytes(name, value) {
            response.add_header(header);
        }
    }
}

/// Percent-decode a URL component (`%20` etc.). A literal `+` is left
/// alone: `encodeURIComponent`, which the demo uses, encodes space as `%20`.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
