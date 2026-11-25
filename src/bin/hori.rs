//! Minimal desktop CLI around the same engine, used for smoke-testing the
//! core on a PC (the Android app talks to the very same Engine through JNI).
//!
//! Usage:
//!   hori add <magnet-or-url-or-file> [--out DIR] [--paused] [--wait]
//!   hori list [--out DIR]
//!   hori daemon --out DIR   (keeps downloading/seeding, prints stats)

use std::path::PathBuf;
use std::time::Duration;

use hori_core::Engine;

fn usage() -> ! {
    eprintln!(
        "hori — Hori Torrent core CLI\n\n\
         hori add <magnet|url|file.torrent> [--out DIR] [--paused] [--wait]\n\
         hori list [--out DIR]\n\
         hori daemon [--out DIR]\n"
    );
    std::process::exit(2);
}

fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", UNITS[i])
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,librqbit=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args[0].as_str();
    let mut out = PathBuf::from("downloads");
    let mut paused = false;
    let mut wait = false;
    let mut source: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = PathBuf::from(args.get(i).unwrap_or_else(|| usage()));
            }
            "--paused" => paused = true,
            "--wait" => wait = true,
            s if !s.starts_with("--") && source.is_none() => source = Some(s.to_string()),
            other => {
                eprintln!("unknown argument: {other}");
                usage();
            }
        }
        i += 1;
    }

    let state_dir = out.join(".hori-session");
    let engine = match Engine::new(out.clone(), Some(state_dir)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("engine init failed: {e:#}");
            std::process::exit(1);
        }
    };

    match cmd {
        "add" => {
            let source = source.unwrap_or_else(|| usage());
            match engine.add(&source, paused) {
                Ok(id) => println!("added torrent id={id}"),
                Err(e) => {
                    eprintln!("add failed: {e:#}");
                    std::process::exit(1);
                }
            }
            if wait {
                println!("downloading… Ctrl+C to stop");
                loop {
                    engine.sleep(Duration::from_secs(2));
                    let (down, up) = engine.total_speed();
                    let list = engine.list_json().unwrap_or_default();
                    let done = list.contains("\"finished\":true");
                    println!(
                        "down {}/s  up {}/s{}",
                        fmt_bytes(down),
                        fmt_bytes(up),
                        if done { "  [finished]" } else { "" }
                    );
                    if done {
                        println!("{}", pretty(&list));
                        break;
                    }
                }
            } else {
                println!("{}", engine.list_json().unwrap_or_default());
            }
        }
        "list" => println!("{}", pretty(&engine.list_json().unwrap_or_default())),
        "daemon" => {
            println!("daemon running, Ctrl+C to stop");
            loop {
                engine.sleep(Duration::from_secs(5));
                let (down, up) = engine.total_speed();
                println!("down {}/s  up {}/s", fmt_bytes(down), fmt_bytes(up));
            }
        }
        _ => usage(),
    }
}

fn pretty(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        .unwrap_or_else(|_| json.to_string())
}
