use std::sync::Arc;

use crate::config::Offsets;
use crate::game::player::PlayerSnapshot;
use crate::game::validation::{dedupe_players, PlayerValidator};
use crate::memory::error::MemoryError;
use crate::memory::il2cpp::{read_pointer_list, resolve_static_instance};
use crate::memory::process::ProcessHandle;

#[derive(Debug, Clone)]
pub struct ScanSnapshot {
    pub connected: bool,
    pub in_active_match: bool,
    pub game_state: i32,
    pub players: Vec<PlayerSnapshot>,
    pub status_message: String,
}

pub struct GameScanner {
    offsets: Arc<Offsets>,
    process: Option<ProcessHandle>,
}

impl GameScanner {
    pub fn new(offsets: Arc<Offsets>) -> Self {
        Self {
            offsets,
            process: None,
        }
    }

    pub fn offsets(&self) -> &Arc<Offsets> {
        &self.offsets
    }

    pub fn set_process(&mut self, process: ProcessHandle) {
        self.process = Some(process);
    }

    pub fn scan(&mut self) -> Result<ScanSnapshot, MemoryError> {
        if !self.offsets.offsets_configured() {
            return Ok(ScanSnapshot {
                connected: false,
                in_active_match: false,
                game_state: -1,
                players: Vec::new(),
                status_message:
                    "Configure offsets.toml (Il2CppDumper TypeInfo addresses)".into(),
            });
        }

        let process = self
            .process
            .as_ref()
            .ok_or(MemoryError::ProcessNotFound("not attached".into()))?;

        let reader = process.reader();
        let module_base = process.module_base();

        eprintln!("[scan] module_base=0x{module_base:X}");

        let client = resolve_static_instance(
            &reader,
            module_base,
            self.offsets.static_pointers.among_us_client_type_info,
            self.offsets.static_fields.among_us_client_instance,
            &self.offsets.il2cpp,
        )
        .ok();

        let game_state = if let Some(client) = client {
            match reader.read_i32(client + self.offsets.among_us_client.game_state) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("[scan] failed to read GameState at client=0x{client:X}: {err}");
                    -1
                }
            }
        } else {
            -1
        };

        if let Some(client) = client {
            eprintln!("[scan] client_ptr=0x{client:X} game_state={game_state}");
        } else {
            eprintln!("[scan] could not resolve AmongUsClient singleton");
        }
        let active_states = self.offsets.active_game_states();
        let in_active_match = active_states.contains(&game_state);

        if client.is_none() || game_state < 0 {
            return Ok(ScanSnapshot {
                connected: false,
                in_active_match: false,
                game_state,
                players: Vec::new(),
                status_message: "Unable to resolve AmongUsClient singleton".into(),
            });
        }

        let _game_data = resolve_static_instance(
            &reader,
            module_base,
            self.offsets.static_pointers.game_data_type_info,
            self.offsets.static_fields.game_data_instance,
            &self.offsets.il2cpp,
        )
        .ok();

        let list_ptr = resolve_static_instance(
            &reader,
            module_base,
            self.offsets.static_pointers.player_control_type_info,
            self.offsets.static_fields.player_control_all_player_controls,
            &self.offsets.il2cpp,
        )
        .ok();

        let player_ptrs = if let Some(list_ptr) = list_ptr {
            eprintln!("[scan] player_list_ptr=0x{list_ptr:X}");
            match read_pointer_list(
                &reader,
                list_ptr,
                &self.offsets.list,
                &self.offsets.array,
                self.offsets.validation.max_players,
            ) {
                Ok(ptrs) => ptrs,
                Err(err) => {
                    eprintln!("[scan] failed to read player list: {err}");
                    return Ok(ScanSnapshot {
                        connected: true,
                        in_active_match,
                        game_state,
                        players: Vec::new(),
                        status_message: format!("Failed to read player list: {err}"),
                    });
                }
            }
        } else {
            return Ok(ScanSnapshot {
                connected: true,
                in_active_match,
                game_state,
                players: Vec::new(),
                status_message: "Unable to resolve PlayerControl list".into(),
            });
        };

        if player_ptrs.is_empty() {
            return Ok(ScanSnapshot {
                connected: true,
                in_active_match,
                game_state,
                players: Vec::new(),
                status_message: format!("State {game_state}: Player list empty"),
            });
        }

        let validator = PlayerValidator::new(
            &reader,
            &self.offsets.validation,
            self.offsets.valid_roles(),
        );

        let mut players = Vec::new();
        for player_ptr in player_ptrs {
            match validator.read_player(
                player_ptr,
                self.offsets.player_control.data,
                &self.offsets.networked_player_info,
                &self.offsets.mono_string,
            ) {
                Ok(player) => players.push(player),
                Err(_) => continue,
            }
        }

        players = dedupe_players(players);

        if players.is_empty() {
            return Ok(ScanSnapshot {
                connected: true,
                in_active_match,
                game_state,
                players: Vec::new(),
                status_message: "No validated players found".into(),
            });
        }

        Ok(ScanSnapshot {
            connected: true,
            in_active_match,
            game_state,
            players,
            status_message: String::new(),
        })
    }
}
