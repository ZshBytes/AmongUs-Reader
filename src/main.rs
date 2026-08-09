// Author: @szuwer
// Among Us Live External Overlay

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::Offsets;
use crate::game::scanner::{GameScanner, ScanSnapshot};
use crate::game::state::SharedGameState;
use crate::memory::process::ProcessHandle;
use crate::overlay::OverlayApp;

mod config;
mod game;
mod memory;
mod overlay;

fn offsets_path() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        let beside = dir.join("offsets.toml");
        if beside.exists() {
            return beside;
        }
    }
    PathBuf::from("offsets.toml")
}

fn spawn_scanner(offsets: Arc<Offsets>, shared: Arc<SharedGameState>) {
    thread::Builder::new()
        .name("memory-scanner".into())
        .spawn(move || {
            let interval = Duration::from_millis(offsets.runtime.poll_interval_ms);
            let mut scanner = GameScanner::new(offsets);

            loop {
                match ProcessHandle::attach(
                    &scanner.offsets().process.executable_name,
                    &scanner.offsets().process.module_name,
                ) {
                    Ok(handle) => {
                        scanner.set_process(handle);
                        match scanner.scan() {
                            Ok(snapshot) => shared.apply_snapshot(&snapshot),
                            Err(err) => shared.apply_snapshot(&ScanSnapshot {
                                connected: true,
                                in_active_match: false,
                                game_state: -1,
                                players: Vec::new(),
                                status_message: format!("Scan failed: {err}"),
                            }),
                        }
                    }
                    Err(err) => shared.apply_snapshot(&ScanSnapshot {
                        connected: false,
                        in_active_match: false,
                        game_state: -1,
                        players: Vec::new(),
                        status_message: format!("Waiting for Among Us... ({err})"),
                    }),
                }
                thread::sleep(interval);
            }
        })
        .expect("failed to spawn memory scanner thread");
}

fn main() {
    let (offsets, notes) = Offsets::load(offsets_path()).expect("failed to load offsets.toml");
    for note in notes {
        eprintln!("[offsets] {note}");
    }

    let offsets = Arc::new(offsets);
    let shared = Arc::new(SharedGameState::default());

    spawn_scanner(Arc::clone(&offsets), Arc::clone(&shared));
    OverlayApp::run(offsets.overlay.clone(), shared);
}
