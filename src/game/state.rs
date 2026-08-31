use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;

use crate::game::player::PlayerSnapshot;
use crate::game::role::color_name;
use crate::game::scanner::ScanSnapshot;

#[derive(Debug, Clone, PartialEq)]
pub struct KillEvent {
    pub message: String,
    pub victim_name: String,
    pub killer_name: Option<String>,
    pub location: (f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisguiseEvent {
    pub message: String,
    pub morpher_name: String,
    pub target_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleLogConfig {
    pub log_kills: bool,
    pub log_game_state: bool,
    pub log_player_list: bool,
}

impl Default for ConsoleLogConfig {
    fn default() -> Self {
        Self {
            log_kills: true,
            log_game_state: true,
            log_player_list: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeadBodyMarker {
    pub victim_name: String,
    pub location: (f32, f32),
}

#[derive(Debug, Clone)]
pub struct OverlayStatus {
    pub connected: bool,
    pub in_active_match: bool,
    pub game_state: i32,
    pub players: Vec<PlayerSnapshot>,
    pub kill_events: Vec<KillEvent>,
    pub disguise_events: Vec<DisguiseEvent>,
    pub dead_bodies: Vec<DeadBodyMarker>,
    pub last_kill_time: Option<u64>,
    pub log_config: ConsoleLogConfig,
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
            kill_events: Vec::new(),
            disguise_events: Vec::new(),
            dead_bodies: Vec::new(),
            last_kill_time: None,
            log_config: ConsoleLogConfig::default(),
            status_message: String::new(),
            last_update_ms: 0,
            stream_proof: true,
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

        // 1. Detect any alive -> dead transitions (Kill Events)
        // Strictly ignore deaths that occur during meetings or from voting ejections (was_ejected == true)
        let prev_players = state.players.clone();
        let is_discussion = snapshot.game_state == 3 || state.game_state == 3;
        if !is_discussion && !prev_players.is_empty() {
            for prev in &prev_players {
                if !prev.is_dead && !prev.was_ejected {
                    if let Some(curr) = snapshot
                        .players
                        .iter()
                        .find(|p| p.player_id == prev.player_id)
                    {
                        if curr.is_dead && !curr.was_ejected {
                            let local_opt = snapshot.players.iter().find(|p| p.is_local);
                            let victim_pos = if curr.position.0 != 0.0 || curr.position.1 != 0.0 {
                                curr.position
                            } else if prev.position.0 != 0.0 || prev.position.1 != 0.0 {
                                prev.position
                            } else if let Some(loc) = local_opt {
                                loc.position
                            } else {
                                (0.0, 0.0)
                            };

                            // Find killer:
                            // 1. Alive Impostor within 7m (in online matches)
                            // 2. If no impostor assigned or in Freeplay: local player if within 4m or nearest player
                            let mut impostors: Vec<_> = snapshot
                                .players
                                .iter()
                                .filter(|p| {
                                    p.player_id != curr.player_id
                                        && !p.is_dead
                                        && p.role.is_impostor_team()
                                })
                                .map(|p| {
                                    let dx = p.position.0 - victim_pos.0;
                                    let dy = p.position.1 - victim_pos.1;
                                    let dist = (dx * dx + dy * dy).sqrt();
                                    (p, dist)
                                })
                                .collect();
                            impostors.sort_by(|a, b| {
                                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                            });

                            let killer = if let Some(first_imp) = impostors.first() {
                                Some(first_imp.0.name.clone())
                            } else if let Some(loc) =
                                local_opt.filter(|p| !p.is_dead && p.player_id != curr.player_id)
                            {
                                let dx = loc.position.0 - victim_pos.0;
                                let dy = loc.position.1 - victim_pos.1;
                                let dist = (dx * dx + dy * dy).sqrt();
                                if dist < 5.0 || (victim_pos.0 == 0.0 && victim_pos.1 == 0.0) {
                                    Some(loc.name.clone())
                                } else {
                                    let mut others: Vec<_> = snapshot
                                        .players
                                        .iter()
                                        .filter(|p| {
                                            p.player_id != curr.player_id
                                                && !p.is_dead
                                                && !p.name.starts_with("Dummy")
                                        })
                                        .map(|p| {
                                            let dx = p.position.0 - victim_pos.0;
                                            let dy = p.position.1 - victim_pos.1;
                                            let dist = (dx * dx + dy * dy).sqrt();
                                            (p, dist)
                                        })
                                        .collect();
                                    others.sort_by(|a, b| {
                                        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    others
                                        .first()
                                        .map(|p| p.0.name.clone())
                                        .or_else(|| Some(loc.name.clone()))
                                }
                            } else {
                                let mut others: Vec<_> = snapshot
                                    .players
                                    .iter()
                                    .filter(|p| p.player_id != curr.player_id && !p.is_dead)
                                    .map(|p| {
                                        let dx = p.position.0 - victim_pos.0;
                                        let dy = p.position.1 - victim_pos.1;
                                        let dist = (dx * dx + dy * dy).sqrt();
                                        (p, dist)
                                    })
                                    .collect();
                                others.sort_by(|a, b| {
                                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                                });
                                others.first().map(|p| p.0.name.clone())
                            };

                            let msg = match &killer {
                                Some(k) => format!(
                                    "{} killed {} at ({:.1}, {:.1})",
                                    k, curr.name, victim_pos.0, victim_pos.1
                                ),
                                None => format!(
                                    "{} died at ({:.1}, {:.1})",
                                    curr.name, victim_pos.0, victim_pos.1
                                ),
                            };

                            if state.log_config.log_kills {
                                println!("[KILL] {msg}");
                            }

                            let event = KillEvent {
                                message: msg,
                                victim_name: curr.name.clone(),
                                killer_name: killer,
                                location: victim_pos,
                            };

                            state.last_kill_time = Some(now_ms());
                            state.kill_events.insert(0, event);
                            if state.kill_events.len() > 300 {
                                state.kill_events.truncate(300);
                            }

                            // Add physical dead body marker for ESP canvas
                            state.dead_bodies.push(DeadBodyMarker {
                                victim_name: curr.name.clone(),
                                location: victim_pos,
                            });
                        }
                    }
                }
            }
        }

        // Clear physical dead bodies from tracers when a meeting starts or returning to lobby
        if snapshot.game_state == 3 || snapshot.game_state == 1 {
            state.dead_bodies.clear();
        }

        // Reset logs when transitioning from lobby to a new match
        if state.game_state == 1 && snapshot.game_state == 2 {
            state.kill_events.clear();
            state.disguise_events.clear();
            state.dead_bodies.clear();
            state.last_kill_time = None;
        }

        // 2. Detect Shapeshift / Disguise transitions (strictly during regular gameplay, NOT during meetings or meeting transitions)
        let is_meeting = snapshot.game_state == 3 || state.game_state == 3;
        if !is_meeting && !prev_players.is_empty() {
            for curr in &snapshot.players {
                let prev_entry = prev_players.iter().find(|p| p.player_id == curr.player_id);
                let prev_ss = prev_entry.map(|p| p.shapeshifting).unwrap_or(false);
                let prev_target = prev_entry.and_then(|p| p.shapeshift_target);

                if curr.shapeshifting {
                    let target_opt = curr.shapeshift_target.and_then(|tid| {
                        if tid != curr.player_id {
                            snapshot
                                .players
                                .iter()
                                .find(|p| p.player_id == tid && p.player_id != curr.player_id)
                                .map(|p| p.name.as_str())
                        } else {
                            None
                        }
                    });

                    if (!prev_ss || prev_target.is_none()) && target_opt.is_some() {
                        let tname = target_opt.unwrap();
                        if tname != curr.name {
                            let msg = format!("{} shapeshifted into {}", curr.name, tname);
                            if state.log_config.log_kills {
                                println!("[SHAPESHIFT] {msg}");
                            }
                            let event = DisguiseEvent {
                                message: msg,
                                morpher_name: curr.name.clone(),
                                target_name: tname.to_string(),
                            };
                            state.disguise_events.insert(0, event);
                            if state.disguise_events.len() > 300 {
                                state.disguise_events.truncate(300);
                            }
                        }
                    }
                } else if prev_ss && !curr.shapeshifting {
                    let msg = format!("{} un-shapeshifted", curr.name);
                    if state.log_config.log_kills {
                        println!("[SHAPESHIFT] {msg}");
                    }
                    let event = DisguiseEvent {
                        message: msg,
                        morpher_name: curr.name.clone(),
                        target_name: "Normal".to_string(),
                    };
                    state.disguise_events.insert(0, event);
                    if state.disguise_events.len() > 300 {
                        state.disguise_events.truncate(300);
                    }
                }
            }
        }

        // 3. Log game state changes if enabled
        let status_changed = state.connected != snapshot.connected
            || state.game_state != snapshot.game_state
            || state.status_message != snapshot.status_message;

        if status_changed && state.log_config.log_game_state {
            let state_desc = match snapshot.game_state {
                1 => "LOBBY",
                2 => "IN MATCH (ACTIVE)",
                3 => "MATCH ENDED / DISCUSSION",
                _ => "OFFLINE / DISCONNECTED",
            };
            println!(
                "[GAME STATE] Status: {} (code={}) | {}",
                state_desc, snapshot.game_state, snapshot.status_message
            );
        }

        // 4. Log player list changes if enabled
        let players_changed = state.players.len() != snapshot.players.len()
            || state
                .players
                .iter()
                .zip(&snapshot.players)
                .any(|(a, b)| a.name != b.name || a.role != b.role || a.is_dead != b.is_dead);

        if players_changed && state.log_config.log_player_list && !snapshot.players.is_empty() {
            let player_names: Vec<String> = snapshot
                .players
                .iter()
                .map(|p| {
                    let dead = if p.is_dead { " [DEAD]" } else { "" };
                    format!("{} ({:?}{})", p.name, p.role, dead)
                })
                .collect();
            println!(
                "[PLAYERS LOG] Players ({}) [State={}]: {}",
                snapshot.players.len(),
                snapshot.game_state,
                player_names.join(", ")
            );
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

    pub fn toggle_log_kills(&self) -> bool {
        let mut state = self.inner.write();
        state.log_config.log_kills = !state.log_config.log_kills;
        let val = state.log_config.log_kills;
        println!(
            "[LOG CONFIG] Kill Event Logging: {}",
            if val { "ENABLED" } else { "DISABLED" }
        );
        self.generation.fetch_add(1, Ordering::Relaxed);
        val
    }

    pub fn toggle_log_game_state(&self) -> bool {
        let mut state = self.inner.write();
        state.log_config.log_game_state = !state.log_config.log_game_state;
        let val = state.log_config.log_game_state;
        println!(
            "[LOG CONFIG] Game State Logging: {}",
            if val { "ENABLED" } else { "DISABLED" }
        );
        self.generation.fetch_add(1, Ordering::Relaxed);
        val
    }

    pub fn toggle_log_player_list(&self) -> bool {
        let mut state = self.inner.write();
        state.log_config.log_player_list = !state.log_config.log_player_list;
        let val = state.log_config.log_player_list;
        println!(
            "[LOG CONFIG] Player List Logging: {}",
            if val { "ENABLED" } else { "DISABLED" }
        );
        self.generation.fetch_add(1, Ordering::Relaxed);
        val
    }

    pub fn clear_logs(&self) {
        let mut state = self.inner.write();
        state.kill_events.clear();
        state.disguise_events.clear();
        state.dead_bodies.clear();
        state.last_kill_time = None;
        for p in &mut state.players {
            p.shapeshifting = false;
            p.shapeshift_target = None;
        }
        println!("[LOGS] Cleared match kills, dead bodies, and disguise event logs.");
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub fn export_match_log(&self) -> Result<String, String> {
        let state = self.inner.read();
        if state.players.is_empty() && state.kill_events.is_empty() {
            return Err("No active player data or match logs to export.".into());
        }

        let now_s = now_ms() / 1000;
        let filename = format!("match_log_{now_s}.txt");
        let dir = std::path::Path::new("match_logs");
        if !dir.exists() {
            let _ = std::fs::create_dir_all(dir);
        }
        let filepath = dir.join(&filename);

        let mut lines = Vec::new();
        lines.push(format!("Match Log ({now_s})"));
        lines.push(String::new());
        lines.push("Player Data:".into());

        for p in &state.players {
            let fc = if p.friend_code.is_empty() {
                String::new()
            } else {
                format!(" | Friend Code: {}", p.friend_code)
            };
            let status = if p.is_dead { "Dead" } else { "Alive" };
            lines.push(format!(
                "- [{}] {} ({}, {}){}",
                color_name(p.color_id),
                p.name,
                p.role.to_string(),
                status,
                fc
            ));
        }

        lines.push(String::new());
        lines.push("Kills:".into());
        if state.kill_events.is_empty() {
            lines.push("- None".into());
        } else {
            for event in state.kill_events.iter().rev() {
                lines.push(format!("- {}", event.message));
            }
        }

        if !state.disguise_events.is_empty() {
            lines.push(String::new());
            lines.push("Shapeshifts:".into());
            for event in state.disguise_events.iter().rev() {
                lines.push(format!("- {}", event.message));
            }
        }

        let content = lines.join("\r\n");
        match std::fs::write(&filepath, content) {
            Ok(_) => {
                let path_str = filepath.to_string_lossy().to_string();
                println!("[EXPORT] Match report successfully saved to: {path_str}");
                Ok(path_str)
            }
            Err(e) => Err(format!("Failed to write match log: {e}")),
        }
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
