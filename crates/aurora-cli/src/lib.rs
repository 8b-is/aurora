#![forbid(unsafe_code)]
//! Integer-only CLI and the durable memory bridge used by the Go cortex.
use aurora_consolidation::{DreamConfig, DreamWorker, EpisodicBuffer};
use aurora_core::{Rational, TernarySpin};
use aurora_dyad::DyadState;
use aurora_mem8::MemoryLog;
use aurora_phoenix::PhoenixState;
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn run(args: &[String]) -> Result<String, String> {
    match args.get(1).map(String::as_str).unwrap_or("status") {
        "init" => {
            let dir = home_dir().join(".aurora"); std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let path = dir.join("Aurora.toml");
            use std::io::Write;
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => { f.write_all(include_str!("../../../Aurora.toml").as_bytes()).map_err(|e| e.to_string())?; f.sync_all().map_err(|e| e.to_string())?; }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
            Ok(format!("AURORA configuration: {}\n", path.display()))
        }
        "memory" => memory(&args[2..]),
        "breathe" => {
            let mut p = PhoenixState::new(); for j in [900_000, 400_000, 100_000, 0, 0, 0] { p.update(j); }
            Ok(format!("demonstration stability={}\n", Rational::new(i64::from(p.stability()), 1_000_000).unwrap()))
        }
        "dyad" => {
            let mut d = DyadState::new(); d.human_phase = 125_000; d.step_dyad(TernarySpin::Resonance, 0);
            Ok(format!("demonstration alignment={}\n", Rational::new(d.phase_alignment(), 1_000_000).unwrap()))
        }
        "dream" => {
            let path = args.get(2).map(PathBuf::from).unwrap_or_else(|| home_dir().join(".aurora/tasks.m8"));
            let events = MemoryLog::load(&path).map_err(|e| format!("MEM8: {e:?}"))?;
            Ok(format!("MEM8 durable events={} (no synthetic memories)\n", events.len()))
        }
        "consolidate" => {
            let path = args.get(2).map(PathBuf::from).unwrap_or_else(|| home_dir().join(".aurora/tasks.m8"));
            let events = MemoryLog::load(&path).map_err(|e| format!("MEM8: {e:?}"))?;
            let config = DreamConfig { idle_threshold: std::time::Duration::from_micros(0), ..DreamConfig::default() };
            let mut worker = DreamWorker::new(config)
                .with_buffer(EpisodicBuffer::from_events(events));
            let start = std::time::Instant::now();
            worker.heartbeat(0);
            Ok(format!("dream-phase: {:?}\n", worker.run_once(start)))
        }
        "kintsugi" => Ok("AURORA: preserve errors and repair from evidence.\n".into()),
        "status" => Ok("AURORA integer/p-q foundation · MEM8AU02/MEM8EV02\nLanguage-model and maintenance commands: aurora-agy help\n".into()),
        "help" | "--help" | "-h" => Ok("aurora init|status|breathe|dyad|dream [FILE]|consolidate [FILE]|kintsugi\naurora memory append FILE P Q < RECEIPT\naurora memory recall FILE [LIMIT]\n".into()),
        other => Err(format!("unknown command: {other}")),
    }
}
fn memory(args: &[String]) -> Result<String, String> {
    let path = Path::new(args.get(1).ok_or("memory requires a file path")?);
    match args.first().map(String::as_str) {
        Some("append") => {
            let p: i64 = args
                .get(2)
                .ok_or("missing p")?
                .parse()
                .map_err(|_| "invalid p")?;
            let q: u64 = args
                .get(3)
                .ok_or("missing q")?
                .parse()
                .map_err(|_| "invalid q")?;
            let salience = Rational::new(p, q).map_err(|e| format!("ratio: {e:?}"))?;
            let mut payload = String::new();
            std::io::stdin()
                .take(262_145)
                .read_to_string(&mut payload)
                .map_err(|e| e.to_string())?;
            let id = MemoryLog::append(path, payload, salience)
                .map_err(|e| format!("MEM8 append: {e:?}"))?;
            Ok(format!("{id}\n"))
        }
        Some("recall") => {
            let limit = args
                .get(2)
                .map(|s| s.parse::<usize>())
                .transpose()
                .map_err(|_| "invalid limit")?
                .unwrap_or(8)
                .min(64);
            let events = MemoryLog::load(path).map_err(|e| format!("MEM8 recall: {e:?}"))?;
            Ok(events
                .iter()
                .rev()
                .take(limit)
                .rev()
                .map(|e| format!("{}\n", e.payload))
                .collect())
        }
        _ => Err("memory requires append or recall".into()),
    }
}
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
