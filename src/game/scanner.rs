use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Offsets;
use crate::game::player::PlayerSnapshot;
use crate::game::validation::{dedupe_players, PlayerValidator};
use crate::memory::error::MemoryError;
use crate::memory::il2cpp::{find_static_fields_block, read_pointer_list};
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
    /// Cached static fields block for PlayerControl (discovered once, reused every tick).
    pc_sf_block: Option<u64>,
    /// Cached static fields block for GameData.
    gd_sf_block: Option<u64>,
    /// Cached static fields block for AmongUsClient.
    auc_sf_block: Option<u64>,
    /// Cached static fields block for MeetingHud.
    meeting_hud_sf_block: Option<u64>,
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
    }

    pub fn set_process(&mut self, process: ProcessHandle) {
        // Clear cached blocks when re-attaching to the process.
        self.pc_sf_block = None;
        self.gd_sf_block = None;
        self.auc_sf_block = None;
        self.meeting_hud_sf_block = None;
        self.process = Some(process);
    }

    pub fn scan(&mut self) -> Result<ScanSnapshot, MemoryError> {
        if !self.offsets.offsets_configured() {
            return Ok(ScanSnapshot {
                connected: false,
                in_active_match: false,
                game_state: -1,
                players: Vec::new(),
                status_message: "Configure offsets.toml (Il2CppDumper TypeInfo addresses)".into(),
            });
        }

        let process = self
            .process
            .as_ref()
            .ok_or(MemoryError::ProcessNotFound("not attached".into()))?;

        let reader = process.reader();
        let module_base = process.module_base();

        // ── Discover static fields blocks (once per class, then cached) ────────────
        // Using a SINGLE discovered block for all fields of the same class is critical:
        // independent calls to resolve_static_instance can land on different (wrong)
        // sf_offsets, giving garbage field values.
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

        // Helper: read a pointer from a static fields block, returning None on null/invalid.
        let read_sf_ptr = |block: Option<u64>, offset: u64| -> Option<u64> {
            let base = block?;
            let ptr = reader.read_pointer(base + offset).ok()?;
            if ptr == 0 || !reader.process().is_valid_pointer(ptr) {
                None
            } else {
                Some(ptr)
            }
        };

        // ── AmongUsClient.Instance + GameState ────────────────────────────────────
        // NOTE: game_state is read for metadata/display only.  We do NOT gate the
        // player scan on it — in release builds the AmongUsClient pointer resolution
        // can transiently return -1 under LTO, so blocking here would produce a false
        // "Waiting for lobby" even when players are live.
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

        let active_states = self.offsets.active_game_states();
        let in_active_match = active_states.contains(&game_state);

        // ── Read LocalPlayer and AllPlayerControls from THE SAME pc_sf_block ──────
        let local_player_ptr = read_sf_ptr(self.pc_sf_block, 0x0); // PlayerControl.LocalPlayer
        let all_controls_list = read_sf_ptr(
            self.pc_sf_block, // AllPlayerControls List*
            self.offsets
                .static_fields
                .player_control_all_player_controls,
        );

        // ── GameData.Instance -> AllPlayers ───────────────────────────────────────
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

        // ── 1. Read AllPlayerControls (fastest access to live PlayerControl objects) ──
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

        // ── 2. Read GameData.Instance.AllPlayers (full metadata for every player & dummy) ──
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

                            // Try direct read_player with resolved pc
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

                            // Fallback to read_player_data with info_ptr and resolved_pc
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

        // ── 3. Fallback: if GameData didn't return players, use AllPlayerControls directly ──
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

        // 3. PlayerControl.LocalPlayer fallback
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

        // ── Resolve LocalPlayer position and compute relative distances ─────────
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

            // Mark is_local on matching snapshot
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

        // Maintain continuous shapeshift disguise targets without flicker
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

        // ── Read Meeting Voting States (during discussions / meetings) ─────────
        if self.meeting_hud_sf_block.is_none() {
            self.meeting_hud_sf_block = find_static_fields_block(
                &reader,
                module_base,
                0x2AC8E84, // MeetingHud_TypeInfo
                &self.offsets.il2cpp,
            )
            .ok();
        }

        if let Some(sf) = self.meeting_hud_sf_block {
            if let Ok(hud_ptr) = reader.read_pointer(sf) {
                if hud_ptr != 0 && reader.process().is_valid_pointer(hud_ptr) {
                    if let Ok(states_array_ptr) = reader.read_pointer(hud_ptr + 0x5C) {
                        if states_array_ptr != 0
                            && reader.process().is_valid_pointer(states_array_ptr)
                        {
                            if let Ok(count) = reader.read_i32(states_array_ptr + 0xC) {
                                let clamped = count.clamp(0, 15) as u64;
                                for i in 0..clamped {
                                    if let Ok(vote_area_ptr) =
                                        reader.read_pointer(states_array_ptr + 0x10 + (i * 4))
                                    {
                                        if vote_area_ptr != 0
                                            && reader.process().is_valid_pointer(vote_area_ptr)
                                        {
                                            let pid =
                                                reader.read_u8(vote_area_ptr + 0x17).unwrap_or(255);
                                            let did_vote =
                                                reader.read_u8(vote_area_ptr + 0x16).unwrap_or(0)
                                                    == 1;
                                            let voted_for_id =
                                                reader.read_u8(vote_area_ptr + 0x18).unwrap_or(255);

                                            if did_vote && pid <= 15 {
                                                if let Some(p) =
                                                    players.iter_mut().find(|p| p.player_id == pid)
                                                {
                                                    p.voted_for = match voted_for_id {
                                                        253 => Some(-1), // Skipped
                                                        id if id <= 15 => Some(id as i16),
                                                        _ => None,
                                                    };
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if players.is_empty() {
            return Ok(ScanSnapshot {
                connected: true,
                in_active_match,
                game_state,
                players: Vec::new(),
                status_message: "Waiting for players (in lobby)...".into(),
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
