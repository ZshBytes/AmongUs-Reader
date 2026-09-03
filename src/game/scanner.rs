use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Offsets;
use crate::game::player::PlayerSnapshot;
use crate::game::validation::{dedupe_players, PlayerValidator};
use crate::memory::error::MemoryError;
use crate::memory::il2cpp::{find_static_fields_block, read_pointer_list};
use crate::memory::process::ProcessHandle;
use crate::memory::reader::MemoryReader;

#[derive(Debug, Clone, PartialEq)]
pub struct RoleSettingEntry {
    pub role_name: String,
    pub count: i32,
    pub chance: i32,
    pub is_impostor_role: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LobbyRulesSnapshot {
    pub map_id: u8,
    pub max_players: i32,
    pub player_speed: f32,
    pub crew_light: f32,
    pub impostor_light: f32,
    pub kill_cooldown: f32,
    pub num_common_tasks: i32,
    pub num_long_tasks: i32,
    pub num_short_tasks: i32,
    pub num_emergency_meetings: i32,
    pub emergency_cooldown: i32,
    pub num_impostors: i32,
    pub ghosts_do_tasks: bool,
    pub kill_distance: i32,
    pub discussion_time: i32,
    pub voting_time: i32,
    pub confirm_impostor: bool,
    pub visual_tasks: bool,
    pub anonymous_votes: bool,
    pub task_bar_mode: i32,
    pub role_settings: Vec<RoleSettingEntry>,
}

impl LobbyRulesSnapshot {
    pub fn kill_distance_str(&self) -> &'static str {
        match self.kill_distance {
            0 => "Short",
            1 => "Medium",
            2 => "Long",
            _ => "Custom",
        }
    }

    pub fn map_name(&self) -> &'static str {
        match self.map_id {
            0 => "The Skeld",
            1 => "MIRA HQ",
            2 => "Polus",
            3 => "The Airship",
            4 => "The Fungle",
            5 => "Submerged",
            _ => "Custom / Other",
        }
    }

    pub fn task_bar_mode_str(&self) -> &'static str {
        match self.task_bar_mode {
            0 => "Always",
            1 => "Meetings Only",
            2 => "Never (Invisible)",
            _ => "Custom",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanSnapshot {
    pub connected: bool,
    pub in_active_match: bool,
    pub game_state: i32,
    pub room_code: String,
    pub players: Vec<PlayerSnapshot>,
    pub status_message: String,
    pub lobby_rules: Option<LobbyRulesSnapshot>,
}

pub fn decode_game_code(game_id: i32, alphabet: Option<&[u8]>) -> String {
    if game_id == 0 {
        return String::new();
    }
    let raw = game_id as u32;

    const DEFAULT_V2_ALPHABET: &[u8; 26] = b"QWXRTYLPESDFGHJKZUOCVBINMA";
    let alphabet = alphabet.unwrap_or(DEFAULT_V2_ALPHABET);
    if alphabet.len() < 26 {
        return String::new();
    }

    // Among Us V2 game codes have bit 31 (0x80000000 / int.MinValue) set
    if (raw & 0x8000_0000) != 0 {
        let a = raw & 0x3FF;
        let b = (raw >> 10) & 0xFFFFF;

        let c0 = alphabet[(a % 26) as usize] as char;
        let c1 = alphabet[((a / 26) % 26) as usize] as char;
        let c2 = alphabet[(b % 26) as usize] as char;
        let c3 = alphabet[((b / 26) % 26) as usize] as char;
        let c4 = alphabet[((b / 676) % 26) as usize] as char;
        let c5 = alphabet[((b / 17576) % 26) as usize] as char;

        return format!("{c0}{c1}{c2}{c3}{c4}{c5}");
    }

    // Legacy V1 4-letter code
    let bytes = raw.to_le_bytes();
    if bytes.iter().all(|&b| b.is_ascii_uppercase()) {
        return String::from_utf8_lossy(&bytes).to_string();
    }

    String::new()
}

#[allow(dead_code)]
pub fn int_to_game_code(game_id: i32) -> String {
    decode_game_code(game_id, None)
}

fn read_utf16_string(
    reader: &crate::memory::reader::MemoryReader<'_>,
    string_ptr: u64,
    layout: &crate::config::MonoStringLayout,
) -> Result<String, crate::memory::error::MemoryError> {
    if string_ptr == 0 || !reader.process().is_valid_pointer(string_ptr) {
        return Err(crate::memory::error::MemoryError::InvalidPointer(string_ptr));
    }
    let pointer_size = reader.process().pointer_size();
    let length_offset = layout.length_offset(pointer_size);
    let chars_offset = layout.chars_offset(pointer_size);

    let length = reader.read_i32(string_ptr + length_offset)?;
    if length <= 0 || length > 100 {
        return Err(crate::memory::error::MemoryError::InvalidString);
    }
    let byte_len = (length as usize) * 2;
    let mut raw = vec![0u8; byte_len];
    reader.read_bytes(string_ptr + chars_offset, &mut raw)?;

    let units = raw
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();

    String::from_utf16(&units).map_err(|_| crate::memory::error::MemoryError::InvalidString)
}

pub struct GameScanner {
    offsets: Arc<Offsets>,
    process: Option<ProcessHandle>,
    /// Cached static fields block for PlayerControl (discovered once, reused every tick).
    pc_sf_block: Option<u64>,
    /// Cached static fields block for GameData.
    gd_sf_block: Option<u64>,
    /// Cached static fields block for AmongUsClient.
    auc_sf_block: Option<u64>,
    /// Cached static fields block for MeetingHud.
    meeting_hud_sf_block: Option<u64>,
    /// Cached static fields block for GameOptionsManager.
    gom_sf_block: Option<u64>,
    /// Live GameCode.V2 alphabet read dynamically from process memory.
    game_code_alphabet: Option<Vec<u8>>,
    cached_disguises: HashMap<u8, u8>,
}

impl GameScanner {
    pub fn new(offsets: Arc<Offsets>) -> Self {
        Self {
            offsets,
            process: None,
            pc_sf_block: None,
            gd_sf_block: None,
            auc_sf_block: None,
            meeting_hud_sf_block: None,
            gom_sf_block: None,
            game_code_alphabet: None,
            cached_disguises: HashMap::new(),
        }
    }

    pub fn offsets(&self) -> &Arc<Offsets> {
        &self.offsets
    }

    pub fn has_process(&self) -> bool {
        self.process.is_some()
    }

    #[allow(dead_code)]
    pub fn clear_process(&mut self) {
        self.process = None;
        self.pc_sf_block = None;
        self.gd_sf_block = None;
        self.auc_sf_block = None;
        self.meeting_hud_sf_block = None;
        self.gom_sf_block = None;
    }

    pub fn set_process(&mut self, process: ProcessHandle) {
        // Clear cached blocks when re-attaching to the process.
        self.pc_sf_block = None;
        self.gd_sf_block = None;
        self.auc_sf_block = None;
        self.meeting_hud_sf_block = None;
        self.gom_sf_block = None;
        self.process = Some(process);
    }

    pub fn scan(&mut self) -> Result<ScanSnapshot, MemoryError> {
        if !self.offsets.offsets_configured() {
            return Ok(ScanSnapshot {
                connected: false,
                in_active_match: false,
                game_state: -1,
                room_code: String::new(),
                players: Vec::new(),
                status_message: "Configure offsets.toml (Il2CppDumper TypeInfo addresses)".into(),
                lobby_rules: None,
            });
        }

        let process = self
            .process
            .as_ref()
            .ok_or(MemoryError::ProcessNotFound("not attached".into()))?;

        let reader = process.reader();
        let module_base = process.module_base();

        if self.pc_sf_block.is_none() {
            self.pc_sf_block = find_static_fields_block(
                &reader,
                module_base,
                self.offsets.static_pointers.player_control_type_info,
                &self.offsets.il2cpp,
            )
            .ok();
        }
        if self.gd_sf_block.is_none() {
            self.gd_sf_block = find_static_fields_block(
                &reader,
                module_base,
                self.offsets.static_pointers.game_data_type_info,
                &self.offsets.il2cpp,
            )
            .ok();
        }
        if self.auc_sf_block.is_none() {
            self.auc_sf_block = find_static_fields_block(
                &reader,
                module_base,
                self.offsets.static_pointers.among_us_client_type_info,
                &self.offsets.il2cpp,
            )
            .ok();
        }

        let read_sf_ptr = |block: Option<u64>, offset: u64| -> Option<u64> {
            let base = block?;
            let ptr = reader.read_pointer(base + offset).ok()?;
            if ptr == 0 || !reader.process().is_valid_pointer(ptr) {
                None
            } else {
                Some(ptr)
            }
        };

        let client_ptr = read_sf_ptr(
            self.auc_sf_block,
            self.offsets.static_fields.among_us_client_instance,
        );
        let game_state = client_ptr
            .and_then(|c| {
                reader
                    .read_i32(c + self.offsets.among_us_client.game_state)
                    .ok()
            })
            .unwrap_or(-1);

        if self.game_code_alphabet.is_none() {
            let type_info = self.offsets.static_pointers.game_code_type_info;
            for candidate in [type_info, 0x2AE8F80_u64, 0x2AE8F7C, 0x2AE8F84, 0x2AE8F78] {
                if candidate == 0 {
                    continue;
                }
                if let Ok(sf) = find_static_fields_block(&reader, module_base, candidate, &self.offsets.il2cpp) {
                    for str_off in [0x4_u64, 0x8, 0x0, 0xC] {
                        if let Ok(str_ptr) = reader.read_pointer(sf + str_off) {
                            if let Ok(alpha) = read_utf16_string(&reader, str_ptr, &self.offsets.mono_string) {
                                let trimmed = alpha.trim();
                                if trimmed.len() == 26 && trimmed.chars().all(|c| c.is_ascii_uppercase()) {
                                    self.game_code_alphabet = Some(trimmed.as_bytes().to_vec());
                                    break;
                                }
                            }
                        }
                    }
                    if self.game_code_alphabet.is_some() {
                        break;
                    }
                }
            }
        }

        let mut room_code = String::new();
        if let Some(c) = client_ptr {
            for gid_off in [0x2C_u64, 0x44, 0x48, 0x30, 0x34, 0x38, 0x4C, 0x50, 0x54, 0x58] {
                if let Ok(gid) = reader.read_i32(c + gid_off) {
                    let code = decode_game_code(gid, self.game_code_alphabet.as_deref());
                    if !code.is_empty() && code.len() >= 4 {
                        room_code = code;
                        break;
                    }
                }
            }
        }

        let active_states = self.offsets.active_game_states();
        let in_active_match = active_states.contains(&game_state);

        let local_player_ptr = read_sf_ptr(self.pc_sf_block, 0x0);
        let all_controls_list = read_sf_ptr(
            self.pc_sf_block,
            self.offsets
                .static_fields
                .player_control_all_player_controls,
        );

        let game_data_ptr = read_sf_ptr(
            self.gd_sf_block,
            self.offsets.static_fields.game_data_instance,
        );

        let validator = PlayerValidator::new(
            &reader,
            &self.offsets.validation,
            self.offsets.valid_roles(),
        );

        let mut players = Vec::new();
        let cnt = &self.offsets.custom_network_transform;

        let mut pc_map = std::collections::HashMap::<u8, u64>::new();
        let mut pos_map = std::collections::HashMap::<u8, (f32, f32)>::new();

        if let Some(list_ptr) = all_controls_list {
            if let Ok(ptrs) = read_pointer_list(
                &reader,
                list_ptr,
                &self.offsets.list,
                &self.offsets.array,
                self.offsets.validation.max_players,
            ) {
                for pc_ptr in ptrs {
                    if pc_ptr != 0 && reader.process().is_valid_pointer(pc_ptr) {
                        let pid = reader.read_u8(pc_ptr + 0x28).unwrap_or(255);
                        let pos = validator.read_player_position(
                            pc_ptr,
                            0,
                            cnt.net_transform,
                            cnt.last_position,
                        );
                        if pid <= 15 {
                            pc_map.insert(pid, pc_ptr);
                            if pos != (0.0, 0.0) {
                                pos_map.insert(pid, pos);
                            }
                        }
                    }
                }
            }
        }

        if let Some(game_data) = game_data_ptr {
            for list_field_off in [0x10_u64, 0x14] {
                if let Ok(ap_list_ptr) = reader.read_pointer(game_data + list_field_off) {
                    if ap_list_ptr == 0 || !reader.process().is_valid_pointer(ap_list_ptr) {
                        continue;
                    }
                    if let Ok(info_ptrs) = read_pointer_list(
                        &reader,
                        ap_list_ptr,
                        &self.offsets.list,
                        &self.offsets.array,
                        self.offsets.validation.max_players,
                    ) {
                        for info_ptr in info_ptrs {
                            let pid = reader.read_u8(info_ptr + 0x28).unwrap_or(255);
                            let resolved_pc = pc_map.get(&pid).copied().unwrap_or(0);

                            let mut snapshot_opt = None;
                            if resolved_pc != 0 {
                                if let Ok(player) = validator.read_player(
                                    resolved_pc,
                                    self.offsets.player_control.data,
                                    &self.offsets.networked_player_info,
                                    cnt,
                                    &self.offsets.mono_string,
                                ) {
                                    if !player.disconnected {
                                        snapshot_opt = Some(player);
                                    }
                                }
                            }

                            if snapshot_opt.is_none() {
                                if let Ok(player) = validator.read_player_data(
                                    info_ptr,
                                    resolved_pc,
                                    &self.offsets.networked_player_info,
                                    cnt,
                                    &self.offsets.mono_string,
                                ) {
                                    if !player.disconnected {
                                        snapshot_opt = Some(player);
                                    }
                                }
                            }

                            if let Some(mut player) = snapshot_opt {
                                if player.position == (0.0, 0.0) {
                                    if let Some(&pos) = pos_map.get(&player.player_id) {
                                        player.position = pos;
                                    }
                                }
                                players.push(player);
                            }
                        }
                        if !players.is_empty() {
                            break;
                        }
                    }
                }
            }
        }

        if players.is_empty() {
            if let Some(list_ptr) = all_controls_list {
                if let Ok(ptrs) = read_pointer_list(
                    &reader,
                    list_ptr,
                    &self.offsets.list,
                    &self.offsets.array,
                    self.offsets.validation.max_players,
                ) {
                    for player_ptr in ptrs {
                        if let Ok(player) = validator.read_player(
                            player_ptr,
                            self.offsets.player_control.data,
                            &self.offsets.networked_player_info,
                            cnt,
                            &self.offsets.mono_string,
                        ) {
                            if !player.disconnected {
                                players.push(player);
                            }
                        }
                    }
                }
            }
        }

        if players.is_empty() {
            if let Some(local_ptr) = local_player_ptr {
                if let Ok(player) = validator.read_player(
                    local_ptr,
                    self.offsets.player_control.data,
                    &self.offsets.networked_player_info,
                    cnt,
                    &self.offsets.mono_string,
                ) {
                    if !player.disconnected {
                        players.push(player);
                    }
                }
            }
        }

        players = dedupe_players(players);

        let local_pos = if let Some(local_ptr) = local_player_ptr {
            let local_data = reader
                .read_pointer(local_ptr + self.offsets.player_control.data)
                .unwrap_or(0);
            let local_id = if local_data != 0 {
                reader.read_u8(local_data + 0x28).ok()
            } else {
                None
            };

            let pos = validator.read_player_position(
                local_ptr,
                local_data,
                cnt.net_transform,
                cnt.last_position,
            );

            for p in &mut players {
                if let Some(lid) = local_id {
                    if p.player_id == lid || p.is_local {
                        p.is_local = true;
                        p.position = pos;
                    }
                }
            }

            Some(pos)
        } else {
            None
        };

        if let Some((lx, ly)) = local_pos {
            for p in &mut players {
                if p.is_local {
                    p.distance = 0.0;
                } else {
                    let dx = p.position.0 - lx;
                    let dy = p.position.1 - ly;
                    p.distance = (dx * dx + dy * dy).sqrt();
                }
            }
        }

        for p in &mut players {
            if p.shapeshifting {
                if let Some(tid) = p.shapeshift_target {
                    self.cached_disguises.insert(p.player_id, tid);
                } else if let Some(&tid) = self.cached_disguises.get(&p.player_id) {
                    p.shapeshift_target = Some(tid);
                }
            } else {
                self.cached_disguises.remove(&p.player_id);
            }
        }

        if self.meeting_hud_sf_block.is_none() {
            let meeting_type_info = self.offsets.static_pointers.meeting_hud_type_info;
            for candidate in [meeting_type_info, 0x2AC8E84_u64, 0x2AC8E80, 0x2AC8EC0, 0x2AC7874, 0x2ADC244] {
                if candidate == 0 {
                    continue;
                }
                if let Ok(sf) = find_static_fields_block(&reader, module_base, candidate, &self.offsets.il2cpp) {
                    if let Ok(hud_ptr) = reader.read_pointer(sf) {
                        if hud_ptr != 0 && reader.process().is_valid_pointer(hud_ptr) {
                            self.meeting_hud_sf_block = Some(sf);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(sf) = self.meeting_hud_sf_block {
            if let Ok(hud_ptr) = reader.read_pointer(sf) {
                if hud_ptr != 0 && reader.process().is_valid_pointer(hud_ptr) {
                    let meeting_state = reader.read_i32(hud_ptr + 0x88).unwrap_or(-1);
                    // Only read votes when actively in a meeting (0: Animating, 1: Discussion, 2: NotVoted, 3: Voted, 4: Results)
                    if (0..=4).contains(&meeting_state) {
                        for arr_off in [0x5C_u64, 0x60, 0x64, 0x58, 0x50, 0x54] {
                            if let Ok(states_array_ptr) = reader.read_pointer(hud_ptr + arr_off) {
                                if states_array_ptr != 0 && reader.process().is_valid_pointer(states_array_ptr) {
                                    if let Ok(count) = reader.read_i32(states_array_ptr + 0xC) {
                                        if count > 0 && count <= 15 {
                                            let clamped = count as u64;
                                            for i in 0..clamped {
                                                if let Ok(vote_area_ptr) = reader.read_pointer(states_array_ptr + 0x10 + (i * 4)) {
                                                    if vote_area_ptr != 0 && reader.process().is_valid_pointer(vote_area_ptr) {
                                                        let did_vote = reader.read_u8(vote_area_ptr + 0x16).unwrap_or(0) == 1
                                                            || reader.read_u8(vote_area_ptr + 0x1A).unwrap_or(0) == 1;

                                                        let pid = match reader.read_u8(vote_area_ptr + 0x17) {
                                                            Ok(p) if p <= 15 => p,
                                                            _ => reader.read_u8(vote_area_ptr + 0x1B).unwrap_or(255),
                                                        };

                                                        let voted_for_raw = reader.read_u8(vote_area_ptr + 0x18)
                                                            .or_else(|_| reader.read_u8(vote_area_ptr + 0x1C))
                                                            .unwrap_or(255);

                                                        if did_vote && pid <= 15 {
                                                            if let Some(p) = players.iter_mut().find(|p| p.player_id == pid) {
                                                                p.voted_for = match voted_for_raw {
                                                                    253 | 254 => Some(-1),
                                                                    id if id <= 15 => Some(id as i16),
                                                                    _ => None,
                                                                };
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.gom_sf_block.is_none() {
            let gom_type_info = self.offsets.static_pointers.game_options_manager_type_info;
            for candidate in [gom_type_info, 0x2AE987C_u64, 0x2AE9C7C] {
                if candidate == 0 {
                    continue;
                }
                if let Ok(sf) = find_static_fields_block(&reader, module_base, candidate, &self.offsets.il2cpp) {
                    self.gom_sf_block = Some(sf);
                    break;
                }
            }
        }

        let lobby_rules = if let Some(sf) = self.gom_sf_block {
            if let Ok(gom_instance) = reader.read_pointer(sf) {
                if gom_instance != 0 && reader.process().is_valid_pointer(gom_instance) {
                    let mut found_rules = None;

                    // Scan across all possible option pointers on GameOptionsManager
                    for opt_off in [0x18_u64, 0x14, 0x24, 0x20, 0x30, 0x2C] {
                        let opts_ptr = reader.read_pointer(gom_instance + opt_off).unwrap_or(0);
                        if opts_ptr == 0 || !reader.process().is_valid_pointer(opts_ptr) {
                            continue;
                        }

                        let speed = reader.read_f32(opts_ptr + 0x18).unwrap_or(1.0);
                        let kill_cd = reader.read_f32(opts_ptr + 0x24).unwrap_or(25.0);

                        if !(0.2..=10.0).contains(&speed) || !(0.0..=180.0).contains(&kill_cd) {
                            continue;
                        }

                        let map_id = reader.read_u8(opts_ptr + 0x14).unwrap_or(0);
                        let max_players = reader.read_i32(opts_ptr + 0xC).unwrap_or(15);
                        let crew_light = reader.read_f32(opts_ptr + 0x1C).unwrap_or(1.0);
                        let imp_light = reader.read_f32(opts_ptr + 0x20).unwrap_or(1.5);
                        let common = reader.read_i32(opts_ptr + 0x28).unwrap_or(1);
                        let long = reader.read_i32(opts_ptr + 0x2C).unwrap_or(1);
                        let short = reader.read_i32(opts_ptr + 0x30).unwrap_or(2);
                        let emerg_meetings = reader.read_i32(opts_ptr + 0x34).unwrap_or(1);
                        let emerg_cd = reader.read_i32(opts_ptr + 0x38).unwrap_or(15);
                        let num_imps = reader.read_i32(opts_ptr + 0x3C).unwrap_or(2);
                        let ghosts_do_tasks = reader.read_u8(opts_ptr + 0x40).unwrap_or(1) != 0;
                        let kill_dist = reader.read_i32(opts_ptr + 0x44).unwrap_or(1);
                        let disc_time = reader.read_i32(opts_ptr + 0x48).unwrap_or(15);
                        let vote_time = reader.read_i32(opts_ptr + 0x4C).unwrap_or(120);
                        let confirm_imp = reader.read_u8(opts_ptr + 0x50).unwrap_or(1) != 0;
                        let visual_tasks = reader.read_u8(opts_ptr + 0x51).unwrap_or(1) != 0;
                        let anon_votes = reader.read_u8(opts_ptr + 0x52).unwrap_or(0) != 0;
                        let task_bar_mode = reader.read_i32(opts_ptr + 0x54).unwrap_or(0);

                        let mut role_settings = Vec::new();

                        // Try finding RoleOptionsCollectionV11 pointer
                        for rc_off in [0x60_u64, 0x5C, 0x64, 0x78, 0x58] {
                            let role_coll_ptr = reader.read_pointer(opts_ptr + rc_off).unwrap_or(0);
                            if role_coll_ptr == 0 || !reader.process().is_valid_pointer(role_coll_ptr) {
                                continue;
                            }

                            // Try finding roles Dictionary
                            for dict_off in [0x8_u64, 0xC, 0x10] {
                                let dict_ptr = reader.read_pointer(role_coll_ptr + dict_off).unwrap_or(0);
                                if dict_ptr == 0 || !reader.process().is_valid_pointer(dict_ptr) {
                                    continue;
                                }

                                // In 32-bit IL2CPP Dictionary, _entries is at 0xC or 0x10, _count is at 0x10 or 0x14
                                let mut candidate_entries = None;
                                for (e_off, c_off) in [(0xC_u64, 0x10_u64), (0x10, 0x14), (0x14, 0x18)] {
                                    let entries_ptr = reader.read_pointer(dict_ptr + e_off).unwrap_or(0);
                                    let count = reader.read_i32(dict_ptr + c_off).unwrap_or(0);
                                    if entries_ptr != 0 && reader.process().is_valid_pointer(entries_ptr) && count > 0 && count <= 32 {
                                        candidate_entries = Some((entries_ptr, count));
                                        break;
                                    }
                                }

                                if let Some((entries_ptr, count)) = candidate_entries {
                                    for i in 0..(count as u64) {
                                        let entry_addr = entries_ptr + 0x10 + (i * 16);
                                        let role_type_id = reader.read_i32(entry_addr + 8).unwrap_or(-1);
                                        let role_data_ptr = reader.read_pointer(entry_addr + 12).unwrap_or(0);

                                        if role_data_ptr != 0 && reader.process().is_valid_pointer(role_data_ptr) {
                                            let val1 = reader.read_i32(role_data_ptr + 0x10).unwrap_or(0);
                                            let val2 = reader.read_i32(role_data_ptr + 0x14).unwrap_or(0);
                                            let (max_count, chance) = if val1 <= 15 && val2 <= 100 {
                                                (val1, val2)
                                            } else if val1 <= 100 && val2 <= 15 {
                                                (val2, val1)
                                            } else {
                                                (val1.clamp(0, 15), val2.clamp(0, 100))
                                            };

                                            let role_opts_ptr = reader.read_pointer(role_data_ptr + 0xC).unwrap_or(0);
                                            let (role_name, is_imp, details) = match decode_role_option_entry(&reader, role_type_id, role_opts_ptr) {
                                                Some(res) => res,
                                                None => continue,
                                            };

                                            role_settings.push(RoleSettingEntry {
                                                role_name: role_name.to_string(),
                                                count: max_count,
                                                chance,
                                                is_impostor_role: is_imp,
                                                details,
                                            });
                                        }
                                    }
                                    break;
                                }
                            }

                            if !role_settings.is_empty() {
                                break;
                            }
                        }

                        found_rules = Some(LobbyRulesSnapshot {
                            map_id,
                            max_players,
                            player_speed: speed,
                            crew_light,
                            impostor_light: imp_light,
                            kill_cooldown: kill_cd,
                            num_common_tasks: common,
                            num_long_tasks: long,
                            num_short_tasks: short,
                            num_emergency_meetings: emerg_meetings,
                            emergency_cooldown: emerg_cd,
                            num_impostors: num_imps,
                            ghosts_do_tasks,
                            kill_distance: kill_dist,
                            discussion_time: disc_time,
                            voting_time: vote_time,
                            confirm_impostor: confirm_imp,
                            visual_tasks,
                            anonymous_votes: anon_votes,
                            task_bar_mode,
                            role_settings,
                        });
                        break;
                    }

                    found_rules
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if players.is_empty() {
            return Ok(ScanSnapshot {
                connected: true,
                in_active_match,
                game_state,
                room_code,
                players: Vec::new(),
                status_message: "Waiting for players (in lobby)...".into(),
                lobby_rules,
            });
        }

        Ok(ScanSnapshot {
            connected: true,
            in_active_match,
            game_state,
            room_code,
            players,
            status_message: String::new(),
            lobby_rules,
        })
    }
}

#[allow(dead_code)]
fn scan_players_fallback<'a>(
    reader: &'a crate::memory::reader::MemoryReader<'a>,
    validator: &PlayerValidator<'a>,
    data_offset: u64,
    info: &crate::config::NetworkedPlayerInfoFields,
    cnt: &crate::config::CustomNetworkTransformFields,
    mono_string: &crate::config::MonoStringLayout,
) -> Vec<PlayerSnapshot> {
    let mut players = Vec::new();
    let pointer_size = reader.process().pointer_size() as usize;
    let module_base = reader.process().module_base();
    let module_size = reader.process().module_size();
    let regions = reader.process().query_committed_regions();

    let candidate_data_offsets: Vec<usize> = if pointer_size == 4 {
        vec![
            0x58, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50, 0x54,
        ]
    } else {
        vec![data_offset as usize]
    };

    let back_offsets: Vec<u64> = if pointer_size == 4 {
        vec![0x58, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50, 0x54]
    } else {
        vec![0x58]
    };

    // Track data_ptr values already attempted this scan to avoid redundant work
    // when the same false-positive object is found from multiple candidate addresses.
    let mut seen_data_ptrs = std::collections::HashSet::<u64>::new();

    for (base, size) in regions {
        let mut buffer = vec![0u8; size];
        if reader.read_bytes(base, &mut buffer).is_err() {
            continue;
        }

        let step = pointer_size;
        let mut i = 0;

        while i + 0x60 <= buffer.len() {
            let ptr_a = base + i as u64;

            for &d_off in &candidate_data_offsets {
                if i + d_off + pointer_size > buffer.len() {
                    continue;
                }

                let data_ptr_val = match pointer_size {
                    4 => u32::from_le_bytes(buffer[i + d_off..i + d_off + 4].try_into().unwrap())
                        as u64,
                    _ => u64::from_le_bytes(buffer[i + d_off..i + d_off + 8].try_into().unwrap()),
                };

                if data_ptr_val == 0 || !reader.process().is_valid_pointer(data_ptr_val) {
                    continue;
                }

                // Skip data_ptrs we've already evaluated this tick.
                if seen_data_ptrs.contains(&data_ptr_val) {
                    continue;
                }

                // Quick pre-filter: the klass pointer at data_ptr[0] must be inside the
                // GameAssembly module (all real IL2CPP managed objects satisfy this).
                // Also double-check by reading klass's own first field (Il2CppClass.image)
                // — this must also be in module range, eliminating ASCII false-positives
                // that happen to land in the module address window by coincidence.
                //
                // 4 MB buffer: MODULEENTRY32W.modBaseSize can underreport the actual
                // mapped range when IL2CPP metadata sections extend past the PE image.
                let module_end = module_base
                    .saturating_add(module_size)
                    .saturating_add(0x40_0000);
                let klass_ok = {
                    let mut kbuf = [0u8; 4];
                    if reader.read_bytes(data_ptr_val, &mut kbuf).is_ok() {
                        let klass = u32::from_le_bytes(kbuf) as u64;
                        if klass >= module_base && klass < module_end {
                            // Verify klass.image (first field of Il2CppClass) is also in module.
                            let mut mbuf = [0u8; 4];
                            reader
                                .read_bytes(klass, &mut mbuf)
                                .map(|_| {
                                    let meta = u32::from_le_bytes(mbuf) as u64;
                                    meta >= module_base && meta < module_end
                                })
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if !klass_ok {
                    continue;
                }

                for &b_off in &back_offsets {
                    if let Ok(back_ptr) = reader.read_pointer(data_ptr_val + b_off) {
                        if back_ptr == ptr_a {
                            seen_data_ptrs.insert(data_ptr_val);
                            if let Ok(player) =
                                validator.read_player(ptr_a, d_off as u64, info, cnt, mono_string)
                            {
                                players.push(player);
                            }
                        }
                    }
                }
            }
            i += step;
        }
    }

    players
}

fn decode_role_option_entry(
    reader: &MemoryReader,
    role_type_id: i32,
    role_opts_ptr: u64,
) -> Option<(&'static str, bool, Vec<String>)> {
    let has_opts = role_opts_ptr != 0 && reader.process().is_valid_pointer(role_opts_ptr);
    let mut details = Vec::new();

    let (name, is_imp) = match role_type_id {
        2 => {
            if has_opts {
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(15.0);
                let batt = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(5.0);
                if (0.0..=120.0).contains(&cd) {
                    details.push(format!("Vitals CD: {cd:.1}s"));
                }
                if (0.0..=120.0).contains(&batt) {
                    details.push(format!("Battery: {batt:.1}s"));
                }
            }
            ("Scientist", false)
        }
        3 => {
            if has_opts {
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(30.0);
                let vent = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(15.0);
                if (0.0..=120.0).contains(&cd) {
                    details.push(format!("Vent CD: {cd:.1}s"));
                }
                if (0.0..=120.0).contains(&vent) {
                    details.push(format!("Max Vent Time: {vent:.1}s"));
                }
            }
            ("Engineer", false)
        }
        4 => {
            if has_opts {
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(60.0);
                let dur = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(10.0);
                let imp_see = reader.read_u8(role_opts_ptr + 0x14).unwrap_or(0) != 0;
                if (0.0..=180.0).contains(&cd) {
                    details.push(format!("Protect CD: {cd:.1}s"));
                }
                if (0.0..=60.0).contains(&dur) {
                    details.push(format!("Shield Duration: {dur:.1}s"));
                }
                details.push(format!("Imps See Protect: {}", if imp_see { "Yes" } else { "No" }));
            }
            ("Guardian Angel", false)
        }
        5 => {
            if has_opts {
                let skin = reader.read_u8(role_opts_ptr + 0xA).unwrap_or(0) != 0;
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(10.0);
                let dur = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(30.0);
                if (0.0..=120.0).contains(&cd) {
                    details.push(format!("Shift CD: {cd:.1}s"));
                }
                if (0.0..=120.0).contains(&dur) {
                    details.push(format!("Shift Duration: {dur:.1}s"));
                }
                details.push(format!("Leave SS Evidence: {}", if skin { "Yes" } else { "No" }));
            }
            ("Shapeshifter", true)
        }
        8 => {
            if has_opts {
                let imp_alert = reader.read_u8(role_opts_ptr + 0xA).unwrap_or(1) != 0;
                let dur = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(10.0);
                if (0.0..=60.0).contains(&dur) {
                    details.push(format!("Alert Duration: {dur:.1}s"));
                }
                details.push(format!("Impostor Alert: {}", if imp_alert { "Yes" } else { "No" }));
            }
            ("Noisemaker", false)
        }
        9 => {
            if has_opts {
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(20.0);
                let dur = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(15.0);
                if (0.0..=120.0).contains(&cd) {
                    details.push(format!("Vanish CD: {cd:.1}s"));
                }
                if (0.0..=120.0).contains(&dur) {
                    details.push(format!("Vanish Duration: {dur:.1}s"));
                }
            }
            ("Phantom", true)
        }
        10 | 11 => {
            if has_opts {
                let cd = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(15.0);
                let dur = reader.read_f32(role_opts_ptr + 0x10).unwrap_or(10.0);
                let delay = reader.read_f32(role_opts_ptr + 0x14).unwrap_or(5.0);
                if (0.0..=120.0).contains(&cd) {
                    details.push(format!("Tracking CD: {cd:.1}s"));
                }
                if (0.0..=120.0).contains(&dur) {
                    details.push(format!("Tracking Duration: {dur:.1}s"));
                }
                if (0.0..=60.0).contains(&delay) {
                    details.push(format!("Delay: {delay:.1}s"));
                }
            }
            ("Tracker", false)
        }
        12 => ("Detective", false),
        18 => {
            if has_opts {
                let dissolve = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(30.0);
                if (0.0..=120.0).contains(&dissolve) {
                    details.push(format!("Dissolve Time: {dissolve:.1}s"));
                }
            }
            ("Viper", true)
        }
        19 => {
            if has_opts {
                let req = reader.read_f32(role_opts_ptr + 0xC).unwrap_or(0.0);
                let pct = if (0.0..=1.0).contains(&req) && req > 0.0 {
                    req * 100.0
                } else {
                    req
                };
                if (0.0..=100.0).contains(&pct) {
                    details.push(format!("Task Unlock: {pct:.0}%"));
                }
            }
            ("Judge", false)
        }
        _ => return None,
    };

    Some((name, is_imp, details))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_to_game_code_valid() {
        let code = int_to_game_code(i32::MIN | 123456789);
        assert!(!code.is_empty());
        assert_eq!(code.len(), 6);
    }
}
