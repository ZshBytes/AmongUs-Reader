use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;

use crate::game::player::PlayerSnapshot;
use crate::game::scanner::ScanSnapshot;

#[derive(Debug, Clone)]
pub struct OverlayStatus {
    pub connected: bool,
    pub in_active_match: bool,
    pub game_state: i32,
    pub players: Vec<PlayerSnapshot>,
    pub status_message: String,
    pub last_update_ms: u64,
    pub stream_proof: bool,
}

impl Default for OverlayStatus {
    fn default() -> Self {
        Self {
            connected: false,
            in_active_match: false,
            game_state: -1,
            players: Vec::new(),
            status_message: String::new(),
            last_update_ms: 0,
            stream_proof: false,
        }
    }
}

#[derive(Default)]
pub struct SharedGameState {
    inner: RwLock<OverlayStatus>,
    generation: AtomicU64,
}

impl SharedGameState {
    pub fn apply_snapshot(&self, snapshot: &ScanSnapshot) {
        let mut state = self.inner.write();

        let players_changed = state.players.len() != snapshot.players.len()
            || state.players.iter().zip(&snapshot.players).any(|(a, b)| a.name != b.name || a.role != b.role || a.is_dead != b.is_dead);
        let status_changed = state.connected != snapshot.connected
            || state.game_state != snapshot.game_state
            || state.status_message != snapshot.status_message;

        if status_changed || players_changed {
            if !snapshot.connected {
                eprintln!("[scanner] {}", snapshot.status_message);
            } else if snapshot.players.is_empty() {
                if status_changed {
                    let msg = if snapshot.status_message.is_empty() {
                        "Connected to process".to_string()
                    } else {
                        snapshot.status_message.clone()
                    };
                    eprintln!("[scanner] Connected (GameState={}) | {}", snapshot.game_state, msg);
                }
            } else {
                let player_names: Vec<String> = snapshot
                    .players
                    .iter()
                    .map(|p| {
                        let dead = if p.is_dead { " [DEAD]" } else { "" };
                        format!("{} ({:?}{})", p.name, p.role, dead)
                    })
                    .collect();
                eprintln!(
                    "[scanner] Players ({}) [GameState={}]: {}",
                    snapshot.players.len(),
                    snapshot.game_state,
                    player_names.join(", ")
                );
            }
        }

        state.connected = snapshot.connected;
        state.in_active_match = snapshot.in_active_match;
        state.game_state = snapshot.game_state;
        state.players = snapshot.players.clone();
        state.status_message = snapshot.status_message.clone();
        state.last_update_ms = now_ms();
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn toggle_stream_proof(&self) -> bool {
        let mut state = self.inner.write();
        state.stream_proof = !state.stream_proof;
        let new_val = state.stream_proof;
        self.generation.fetch_add(1, Ordering::Relaxed);
        new_val
    }

    pub fn snapshot(&self) -> OverlayStatus {
        self.inner.read().clone()
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
