//! A tiny stand-in for yt-dlp used by the engine's integration tests.
//! Behaviour is selected by the path of the URL (the last argument), so the
//! tests are deterministic and never touch the network.

use std::io::Write;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

fn arg_value(args: &[String], flag: &str, prefix: &str) -> Option<String> {
    args.windows(2).filter(|w| w[0] == flag).find_map(|w| w[1].strip_prefix(prefix).map(str::to_string))
}

fn progress(out: &mut impl Write, done: u64, total: u64) {
    let _ = writeln!(out, "[waydlp:dl]downloading|{done}|{total}|NA|1000.0|{}|NA|NA|avc1|mp4a", (total - done) / 1000);
    let _ = out.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("2099.01.01");
        return;
    }
    let url = args.last().cloned().unwrap_or_default();
    let path = url.split("://").nth(1).and_then(|r| r.split_once('/')).map(|(_, p)| p).unwrap_or("");

    if args.iter().any(|a| a == "--dump-single-json") {
        match path {
            "video" => {
                println!(
                    r#"{{"id":"v1","title":"Fake video","uploader":"Tester","duration":12,"formats":[{{"format_id":"18","ext":"mp4","vcodec":"avc1","acodec":"mp4a","height":360}},{{"format_id":"140","ext":"m4a","vcodec":"none","acodec":"mp4a.40.2","abr":128}}]}}"#
                );
            }
            _ => {
                eprintln!("ERROR: Unsupported URL: {url}");
                std::process::exit(1);
            }
        }
        return;
    }

    let home = PathBuf::from(arg_value(&args, "--paths", "home:").expect("home path"));
    let temp = PathBuf::from(arg_value(&args, "--paths", "temp:").expect("temp path"));
    let mut out = std::io::stdout();

    match path {
        "ok" | "flaky" => {
            if path == "flaky" {
                let marker = temp.join("attempted");
                if !marker.exists() {
                    std::fs::create_dir_all(&temp).unwrap();
                    std::fs::write(&marker, b"1").unwrap();
                    eprintln!("ERROR: [generic] Unable to download webpage: getaddrinfo failed");
                    std::process::exit(1);
                }
            }
            let _ = writeln!(out, "[waydlp:item]NA|NA|v1|Fake video");
            for i in 1..=5u64 {
                progress(&mut out, i * 2000, 10_000);
                sleep(Duration::from_millis(30));
            }
            let _ = writeln!(out, "[waydlp:dl]finished|10000|10000|NA|NA|NA|NA|NA|avc1|mp4a");
            let _ = writeln!(out, "[waydlp:pp]started|Merger");
            let file = home.join("Fake video [v1].mp4");
            std::fs::create_dir_all(&home).unwrap();
            std::fs::write(&file, vec![0u8; 10_000]).unwrap();
            let _ = writeln!(out, "[waydlp:file]{}", file.display());
            eprintln!("WARNING: this is a fake warning");
        }
        "slow" => {
            std::fs::create_dir_all(&temp).unwrap();
            std::fs::write(temp.join("video.mp4.part"), b"partial").unwrap();
            for i in 0..600u64 {
                progress(&mut out, i * 10, 1_000_000);
                sleep(Duration::from_millis(50));
            }
        }
        "net" => {
            eprintln!("ERROR: [generic] Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>");
            std::process::exit(1);
        }
        "private" => {
            eprintln!("ERROR: [youtube] abc: Private video. Sign in if you've been granted access");
            std::process::exit(1);
        }
        other => {
            eprintln!("ERROR: Unsupported URL: {other}");
            std::process::exit(1);
        }
    }
}
