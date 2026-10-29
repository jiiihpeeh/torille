//! `serve-demo` — a tiny static file server for the wasm demo.
//!
//! Serves `torille-wasm/demo/` (index.html, `dist/`, and the local book
//! corpus under `books/`) with a permissive `Access-Control-Allow-Origin`
//! on every response. There is no proxy: the reader's books are downloaded
//! once by `tools/fetch_finnish_books.py` and served same-origin, so no
//! cross-origin fetch is needed.
//!
//! Text responses are brotli-compressed when the client sends
//! `Accept-Encoding: br`. The 17 MB wasm is compressed at build time by
//! `torille-wasm/demo/build.mjs` into `dist/*.wasm.br`; that sidecar is
//! served directly when it is present and newer than the source. Anything
//! else compressible (JS, JSON, source maps, the book corpus) is
//! compressed on first request and cached in memory.
//!
//! Usage:
//!   cargo run --release -p torille-demo-server            # root=torille-wasm/demo, :8765
//!   cargo run --release -p torille-demo-server -- --root path --port 9000

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::SystemTime;

use brotli::enc::BrotliEncoderParams;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

/// Quality for on-the-fly compression. Low enough to keep a first
/// request (a book chapter, a source map) under a few hundred ms; the
/// wasm is pre-compressed at quality 9 by the build, not here.
const ON_THE_FLY_QUALITY: i32 = 5;
/// 22 = 4 MB window. Good ratio on text without a large decode buffer.
const ON_THE_FLY_LGWIN: i32 = 22;
/// Responses smaller than this gain too little to bother.
const MIN_COMPRESS: usize = 512;
/// Cap on cached compressed bodies. The book corpus is nearly 1 GB, so
/// the cache is cleared and rebuilt rather than allowed to grow.
const CACHE_BUDGET: usize = 64 << 20;

/// Compressed bodies keyed by source path. Each entry remembers the
/// source's mtime and length so a rebuilt file at the same path is
/// recompressed instead of served stale.
static COMPRESSED: LazyLock<Mutex<CompressedCache>> =
    LazyLock::new(|| Mutex::new(CompressedCache::default()));

#[derive(Default)]
struct CompressedCache {
    entries: HashMap<PathBuf, CachedBody>,
    bytes: usize,
}

struct CachedBody {
    modified: Option<SystemTime>,
    len: u64,
    body: Arc<Vec<u8>>,
}

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
        "serve-demo: http://{addr}/  (root {}, CORS *, brotli)",
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
        Ok(data) => respond_file(request, &path, content_type(&path), data),
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

/// Serve one static file, brotli-compressing it when the client accepts
/// `br`. A build-time `<path>.br` sidecar wins over compressing at
/// request time; everything else goes through the in-memory cache.
fn respond_file(request: Request, path: &Path, ctype: &'static str, data: Vec<u8>) {
    let compressible = is_compressible(ctype);
    if compressible && wants_brotli(&request) {
        if let Some(body) = read_precompressed(path, data.len()) {
            respond_with(request, 200, ctype, body, Some("br"), true);
            return;
        }
        if data.len() >= MIN_COMPRESS
            && let Some((body, size)) = compressed_cached(path, &data)
        {
            let compressed = body.to_vec();
            println!(
                "  br {} -> {} bytes ({:.0}% of {})",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                size,
                size as f64 * 100.0 / data.len() as f64,
                data.len()
            );
            respond_with(request, 200, ctype, compressed, Some("br"), true);
            return;
        }
    }
    // Identity response: still advertise the negotiation so caches do
    // not hand a brotli body to a client that cannot decode it.
    respond_with(request, 200, ctype, data, None, compressible);
}

/// `<path>.br`, if the build wrote one and it is not older than the
/// source (a rebuild in `--watch` mode replaces both together).
fn read_precompressed(path: &Path, source_len: usize) -> Option<Vec<u8>> {
    let mut br_path = path.as_os_str().to_owned();
    br_path.push(".br");
    let br_path = PathBuf::from(br_path);

    let source = std::fs::metadata(path).ok()?;
    let compressed = std::fs::metadata(&br_path).ok()?;
    if compressed.len() as usize >= source_len {
        // No saving: fall through to request-time compression, which
        // can still win on bodies the build did not touch.
        return None;
    }
    let source_at = source.modified().ok()?;
    let compressed_at = compressed.modified().ok()?;
    if compressed_at < source_at {
        return None;
    }
    std::fs::read(&br_path).ok()
}

/// Compress and memoise `data`. Returns `None` when brotli would not
/// make the body smaller (the caller serves it unchanged).
fn compressed_cached(path: &Path, data: &[u8]) -> Option<(Arc<Vec<u8>>, usize)> {
    let modified = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok());
    let len = data.len() as u64;

    if let Ok(cache) = COMPRESSED.lock()
        && let Some(entry) = cache.entries.get(path)
        && entry.len == len
        && entry.modified == modified
    {
        return Some((Arc::clone(&entry.body), entry.body.len()));
    }

    let compressed = brotli_compress(data, ON_THE_FLY_QUALITY, ON_THE_FLY_LGWIN)?;
    if compressed.len() >= data.len() {
        return None;
    }
    let body = Arc::new(compressed);

    // Don't let one huge book pin the whole budget.
    if body.len() <= CACHE_BUDGET / 4
        && let Ok(mut cache) = COMPRESSED.lock()
    {
        if cache.bytes + body.len() > CACHE_BUDGET {
            cache.entries.clear();
            cache.bytes = 0;
        }
        if let Some(old) = cache.entries.insert(
            path.to_path_buf(),
            CachedBody {
                modified,
                len,
                body: Arc::clone(&body),
            },
        ) {
            cache.bytes = cache.bytes.saturating_sub(old.body.len());
        }
        cache.bytes += body.len();
    }

    let size = body.len();
    Some((body, size))
}

fn brotli_compress(data: &[u8], quality: i32, lgwin: i32) -> Option<Vec<u8>> {
    let params = BrotliEncoderParams {
        quality,
        lgwin,
        size_hint: data.len(),
        ..Default::default()
    };
    let mut out = Vec::with_capacity(data.len() / 2);
    brotli::BrotliCompress(&mut &data[..], &mut out, &params)
        .ok()
        .map(|_| out)
}

fn is_compressible(ctype: &str) -> bool {
    ctype.starts_with("text/")
        || ctype.starts_with("application/json")
        || ctype.starts_with("application/javascript")
        || ctype.starts_with("application/wasm")
        || ctype.starts_with("image/svg+xml")
}

/// True when the request's `Accept-Encoding` allows brotli.
fn wants_brotli(request: &Request) -> bool {
    for header in request.headers() {
        if header.field.equiv("Accept-Encoding")
            && let Some(allowed) = accepts_encoding(header.value.as_str(), "br")
        {
            return allowed;
        }
    }
    false
}

/// Parse one coding out of an `Accept-Encoding` value. An explicit
/// mention beats `*`; `q=0` (and `q=0.0`, ...) means "not acceptable".
fn accepts_encoding(header: &str, coding: &str) -> Option<bool> {
    let mut wildcard = None;
    for item in header.split(',') {
        let mut parts = item.split(';');
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let q = parts
            .find_map(|param| {
                let param = param.trim();
                let value = param
                    .strip_prefix("q=")
                    .or_else(|| param.strip_prefix("Q="))?;
                value.trim().parse::<f32>().ok()
            })
            .unwrap_or(1.0);
        let allowed = q > 0.0;
        if name.eq_ignore_ascii_case(coding) {
            return Some(allowed);
        }
        if name == "*" {
            wildcard = Some(allowed);
        }
    }
    wildcard
}

fn respond(request: Request, status: u16, ctype: &str, body: &[u8]) {
    respond_with(request, status, ctype, body.to_vec(), None, false);
}

fn respond_with(
    request: Request,
    status: u16,
    ctype: &str,
    body: Vec<u8>,
    encoding: Option<&str>,
    vary: bool,
) {
    let mut response = Response::from_data(body).with_status_code(StatusCode(status));
    if let Ok(header) = Header::from_bytes("Content-Type", ctype) {
        response.add_header(header);
    }
    if let Some(encoding) = encoding
        && let Ok(header) = Header::from_bytes("Content-Encoding", encoding)
    {
        response.add_header(header);
    }
    if vary && let Ok(header) = Header::from_bytes("Vary", "Accept-Encoding") {
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

#[cfg(test)]
mod tests {
    use super::{accepts_encoding, is_compressible};

    #[test]
    fn an_explicit_coding_wins_over_the_wildcard() {
        assert_eq!(
            accepts_encoding("gzip, deflate, br, zstd", "br"),
            Some(true)
        );
        assert_eq!(accepts_encoding("gzip, br;q=0.5", "br"), Some(true));
        assert_eq!(accepts_encoding("BR", "br"), Some(true));
        assert_eq!(accepts_encoding("*;q=0, br", "br"), Some(true));
        assert_eq!(accepts_encoding("*", "br"), Some(true));
    }

    #[test]
    fn q_zero_and_absence_mean_identity() {
        assert_eq!(accepts_encoding("br;q=0", "br"), Some(false));
        assert_eq!(accepts_encoding("*;q=0", "br"), Some(false));
        assert_eq!(accepts_encoding("gzip, deflate", "br"), None);
        assert_eq!(accepts_encoding("", "br"), None);
    }

    #[test]
    fn only_text_like_bodies_are_compressed() {
        assert!(is_compressible("text/javascript; charset=utf-8"));
        assert!(is_compressible("application/json; charset=utf-8"));
        assert!(is_compressible("application/wasm"));
        assert!(is_compressible("image/svg+xml"));
        assert!(!is_compressible("image/png"));
        assert!(!is_compressible("application/octet-stream"));
    }
}
