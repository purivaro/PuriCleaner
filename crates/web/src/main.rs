//! Puri Cleaner in the browser: a tiny local server that serves the same UI
//! as the desktop app and answers its API calls.
//!
//! It listens on 127.0.0.1 only, and every API call must carry a random
//! token handed to the browser in the URL, so other web pages cannot ask it
//! to delete anything.

use std::path::PathBuf;

use serde_json::Value;
use tiny_http::{Header, Method, Response, Server};

const INDEX: &str = include_str!("../../../ui/index.html");
const CSS: &str = include_str!("../../../ui/styles.css");
const JS: &str = include_str!("../../../ui/app.js");
const ICON: &[u8] = include_bytes!("../../../ui/icon.svg");

/// 128 random bits from the OS (std seeds RandomState from the system RNG).
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            format!("{:016x}", h.finish())
        })
        .collect()
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).unwrap()
}

fn main() {
    let mut port: u16 = 7171;
    let mut root: Option<PathBuf> = None;
    let mut open_browser = true;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port),
            "--no-open" => open_browser = false,
            "-h" | "--help" => {
                println!("puri-cleaner-web [--port N] [--no-open] [FOLDER]");
                return;
            }
            _ => root = Some(PathBuf::from(a)),
        }
    }
    // During UI work, PURI_UI_DIR serves files from disk instead of the copy built in.
    let ui_dir = std::env::var_os("PURI_UI_DIR").map(PathBuf::from);
    let server = Server::http(("127.0.0.1", port)).unwrap_or_else(|e| {
        eprintln!("cannot listen on 127.0.0.1:{port}: {e}");
        std::process::exit(1);
    });
    let token = std::env::var("PURI_TOKEN").unwrap_or_else(|_| token());
    let url = format!("http://127.0.0.1:{port}/#t={token}");
    println!("Puri Cleaner: {url}");
    if open_browser {
        #[cfg(windows)]
        let _ = std::process::Command::new("explorer").arg(&url).spawn();
        #[cfg(not(windows))]
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    let app = puri_core::App::new(root);
    let host_ok = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];

    for mut req in server.incoming_requests() {
        // Refuse other host names (DNS-rebinding protection).
        let host = req.headers().iter().find(|h| h.field.equiv("Host")).map(|h| h.value.to_string());
        if !host.is_some_and(|h| host_ok.contains(&h)) {
            let _ = req.respond(Response::from_string("forbidden").with_status_code(403));
            continue;
        }
        let path = req.url().split(['?', '#']).next().unwrap_or("/").to_string();

        if let Some(cmd) = path.strip_prefix("/api/") {
            let authed = req
                .headers()
                .iter()
                .any(|h| h.field.equiv("X-Puri-Token") && h.value.as_str() == token);
            if *req.method() != Method::Post || !authed {
                let _ = req.respond(Response::from_string("forbidden").with_status_code(403));
                continue;
            }
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let args: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let (status, out) = match app.call(cmd, &args) {
                Ok(v) => (200, v),
                Err(e) => (400, serde_json::json!({ "error": e })),
            };
            let resp = Response::from_string(out.to_string())
                .with_status_code(status)
                .with_header(header("Content-Type", "application/json"))
                .with_header(header("Cache-Control", "no-store"));
            let _ = req.respond(resp);
            continue;
        }

        let file = match path.as_str() {
            "/" | "/index.html" => Some(("index.html", "text/html; charset=utf-8")),
            "/styles.css" => Some(("styles.css", "text/css; charset=utf-8")),
            "/app.js" => Some(("app.js", "text/javascript; charset=utf-8")),
            "/icon.svg" => Some(("icon.svg", "image/svg+xml")),
            _ => None,
        };
        let Some((name, mime)) = file else {
            let _ = req.respond(Response::from_string("not found").with_status_code(404));
            continue;
        };
        let bytes: Vec<u8> = match &ui_dir {
            Some(d) => std::fs::read(d.join(name)).unwrap_or_default(),
            None => match name {
                "index.html" => INDEX.as_bytes().to_vec(),
                "styles.css" => CSS.as_bytes().to_vec(),
                "app.js" => JS.as_bytes().to_vec(),
                _ => ICON.to_vec(),
            },
        };
        let resp = Response::from_data(bytes)
            .with_header(header("Content-Type", mime))
            .with_header(header("Cache-Control", "no-cache"));
        let _ = req.respond(resp);
    }
}
