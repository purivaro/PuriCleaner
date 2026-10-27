//! Print installed apps and what each leaves behind: `cargo run -p puri_core --example apps`
use serde_json::json;

fn main() {
    let app = puri_core::App::new(None);
    let v = loop {
        let v = app.call("apps", &json!({})).unwrap();
        if !v["loading"].as_bool().unwrap_or(true) && v.get("apps").is_some() {
            break v;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    };
    let mb = |b: &serde_json::Value| format!("{:.1} MB", b.as_u64().unwrap_or(0) as f64 / 1e6);
    for a in v["apps"].as_array().unwrap() {
        println!("{:40} {:>10}  +{:>10}  {}{}", a["name"].as_str().unwrap(), mb(&a["size"]), mb(&a["leftover_size"]),
            a["id"].as_str().unwrap_or("-"), if a["running"].as_bool() == Some(true) { "  [running]" } else { "" });
        for l in a["leftovers"].as_array().unwrap() {
            println!("      {} {:9} {:>10}  {}", if l["sure"].as_bool() == Some(true) { "✓" } else { "?" }, l["kind"].as_str().unwrap(), mb(&l["size"]), l["path"].as_str().unwrap());
        }
    }
    println!("\nLeftovers of removed apps:");
    for o in v["orphans"].as_array().unwrap() {
        println!("  {:50} {:>10}  {} items", o["id"].as_str().unwrap(), mb(&o["size"]), o["items"].as_array().unwrap().len());
    }
}
