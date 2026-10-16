//! Quick check from the terminal: cargo run --release --example scan [path]
use std::time::{Duration, Instant};

use puri_core::clean::fmt_size;
use serde_json::json;

fn main() {
    let app = puri_core::App::new(std::env::args().nth(1).map(Into::into));
    let t0 = Instant::now();
    app.call("scan", &json!({})).unwrap();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let s = app.call("status", &json!({})).unwrap();
        if s["phase"] == "ready" || s["phase"] == "idle" {
            break;
        }
        eprint!("\r{} files  {}      ", s["files"], fmt_size(s["bytes"].as_u64().unwrap()));
    }
    eprintln!("\nscanned in {:.1}s", t0.elapsed().as_secs_f64());
    let o = app.call("overview", &json!({})).unwrap();
    println!("{}", serde_json::to_string_pretty(&o).unwrap());
    // PURI_DRILL="Library,Library/Developer" prints the biggest folders inside each.
    for rel in std::env::var("PURI_DRILL").unwrap_or_default().split(',').filter(|s| !s.is_empty()) {
        let mut id = 0u64;
        for part in rel.split('/') {
            let n = app.call("node", &json!({ "id": id })).unwrap();
            id = n["children"].as_array().unwrap().iter().find(|c| c["name"] == part).and_then(|c| c["id"].as_u64()).unwrap_or(0);
        }
        let n = app.call("node", &json!({ "id": id })).unwrap();
        println!("── {}", n["path"].as_str().unwrap());
        for c in n["children"].as_array().unwrap().iter().take(12) {
            println!("   {:>10}  {:5} {}", fmt_size(c["size"].as_u64().unwrap()), c["cat"].as_str().unwrap(), c["name"].as_str().unwrap_or("·"));
        }
    }
    let recs = app.call("recommendations", &json!({})).unwrap();
    // PURI_REC=unused-apps prints that recommendation's items.
    let show = std::env::var("PURI_REC").unwrap_or_default();
    for r in recs.as_array().unwrap() {
        if r["id"] == show.as_str() {
            for it in r["items"].as_array().unwrap() {
                println!("   {:>10}  last_used={}  {}  {}", fmt_size(it["size"].as_u64().unwrap()), it["last_used"], it["label"], it["action"]);
            }
        }
        println!(
            "{:7} {:>10}  {:3} items  {}",
            r["level"].as_str().unwrap(),
            fmt_size(r["size"].as_u64().unwrap()),
            r["items"].as_array().unwrap().len(),
            r["title_en"].as_str().unwrap()
        );
    }
}
