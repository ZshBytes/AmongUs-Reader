use std::collections::HashSet;

use crate::config::{
    CustomNetworkTransformFields, MonoStringLayout, NetworkedPlayerInfoFields, ValidationConfig,
};
use crate::game::player::PlayerSnapshot;
use crate::game::role::RoleType;
use crate::memory::error::MemoryError;
use crate::memory::il2cpp::read_mono_string;
use crate::memory::reader::MemoryReader;

pub struct PlayerValidator<'a> {
    reader: &'a MemoryReader<'a>,
    validation: &'a ValidationConfig,
    valid_roles: HashSet<u16>,
}

impl<'a> PlayerValidator<'a> {
    pub fn new(
        reader: &'a MemoryReader<'a>,
        validation: &'a ValidationConfig,
        valid_roles: HashSet<u16>,
    ) -> Self {
        Self {
            reader,
            validation,
            valid_roles,
        }
    }

    pub fn read_player(
        &self,
        player_control_ptr: u64,
        data_offset: u64,
        info: &NetworkedPlayerInfoFields,
        cnt: &CustomNetworkTransformFields,
        string_layout: &MonoStringLayout,
    ) -> Result<PlayerSnapshot, MemoryError> {
        let data_ptr = self.reader.read_pointer(player_control_ptr + data_offset)?;
        self.read_player_data(data_ptr, player_control_ptr, info, cnt, string_layout)
    }

    pub fn read_player_data(
        &self,
        data_ptr: u64,
        player_control_ptr: u64,
        info: &NetworkedPlayerInfoFields,
        cnt: &CustomNetworkTransformFields,
        string_layout: &MonoStringLayout,
    ) -> Result<PlayerSnapshot, MemoryError> {
        if data_ptr == 0 || data_ptr % 4 != 0 || !self.reader.process().is_valid_pointer(data_ptr) {
            return Err(MemoryError::InvalidPointer(data_ptr));
        }

        let klass_ptr = match self.reader.read_pointer(data_ptr) {
            Ok(k) => k,
            Err(e) => return Err(e),
        };
        if !self.reader.process().is_valid_pointer(klass_ptr) {
            return Err(MemoryError::ReadFailed {
                address: data_ptr,
                reason: format!("invalid klass_ptr 0x{klass_ptr:X}"),
            });
        }

        // Validate player_id (0..15).
        // On 32-bit IL2CPP v18, PlayerId (byte) sits around 0x1C–0x28.
        // On 32-bit IL2CPP, NetworkedPlayerInfo.PlayerId sits at 0x28.
        let mut player_id = 255u8;
        for id_off in [0x28_u64, 0x24, 0x2C, 0x20] {
            if let Ok(b) = self.reader.read_u8(data_ptr + id_off) {
                if b <= 15 {
                    player_id = b;
                    break;
                }
            }
        }
        if player_id > 15 {
            return Err(MemoryError::InvalidPointer(data_ptr));
        }

        // Disconnected must be a boolean byte (0 or 1).
        let disconnected_byte = self
            .reader
            .read_u8(data_ptr + info.disconnected)
            .unwrap_or(0);
        if disconnected_byte > 1 {
            return Err(MemoryError::InvalidPointer(data_ptr));
        }
        let disconnected = disconnected_byte == 1;

        // is_dead: strictly check NetworkedPlayerInfo.IsDead at 0x54.
        let is_dead = self
            .reader
            .read_u8(data_ptr + info.is_dead)
            .map(|b| b == 1)
            .unwrap_or(false);

        // was_ejected: check NetworkedPlayerInfo.WasEjected at 0x55.
        let was_ejected = self
            .reader
            .read_u8(data_ptr + 0x55)
            .map(|b| b == 1)
            .unwrap_or(false);

        // RoleType: read from RoleBehaviour (+0x4C -> +0x10) or NetworkedPlayerInfo.RoleType (+0x38).
        let role_raw = self.resolve_role(data_ptr, player_control_ptr, info.role_type);
        let role = RoleType::from_id(role_raw, &self.valid_roles).unwrap_or(RoleType::Crewmate);

        // Name + color resolution.
        let is_valid_player_name = |n: &str| -> bool {
            if n.is_empty() || n.len() > self.validation.max_player_name_len {
                return false;
            }
            let lower = n.to_lowercase();
            const INVALID: &[&str] = &[
                "weapons",
                "shields",
                "navigation",
                "reactor",
                "o2",
                "security",
                "medbay",
                "electrical",
                "cafeteria",
                "storage",
                "admin",
                "communications",
                "upper engine",
                "lower engine",
                "office",
                "laboratory",
                "specimen room",
                "decontamination",
                "main hall",
                "engine room",
                "cockpit",
                "vault",
                "showers",
                "lounge",
                "cargo bay",
                "records",
                "gap room",
                "meeting room",
                "tasks",
                "cancel",
                "use",
                "report",
                "kill",
                "sabotage",
                "vent",
                "untagged",
                "gameobject",
                "transform",
                "maincamera",
                "camera",
                "canvas",
                "eventsystem",
                "default",
                "sprite",
                "audio",
                "sound",
                "manager",
                "controller",
            ];
            !INVALID.contains(&lower.as_str())
        };

        let (name, color_id) =
            self.resolve_name_color(data_ptr, player_id, string_layout, &is_valid_player_name);

        let mut pc = player_control_ptr;
        if pc == 0 || !self.reader.process().is_valid_pointer(pc) {
            if data_ptr != 0 && self.reader.process().is_valid_pointer(data_ptr) {
                for obj_off in [0x58_u64, 0x5C, 0x54, 0x50, 0x48] {
                    if let Ok(p) = self.reader.read_pointer(data_ptr + obj_off) {
                        if p != 0 && self.reader.process().is_valid_pointer(p) {
                            pc = p;
                            break;
                        }
                    }
                }
            }
        }

        let position =
            self.read_player_position(pc, data_ptr, cnt.net_transform, cnt.last_position);

        let mut friend_code = String::new();
        let mut in_vent = false;
        let mut shapeshifting = false;
        let mut shapeshift_target = None;

        if pc != 0 && self.reader.process().is_valid_pointer(pc) {
            if let Ok(fc_ptr) = self.reader.read_pointer(pc + 0x2C) {
                if fc_ptr != 0 && self.reader.process().is_valid_pointer(fc_ptr) {
                    if let Ok(fc) =
                        read_mono_string(self.reader, fc_ptr, string_layout, self.validation)
                    {
                        friend_code = fc.trim().to_string();
                    }
                }
            }
            in_vent = self.reader.read_u8(pc + 0x48).unwrap_or(0) == 1;
            let outfit_type = self.reader.read_i32(pc + 0x44).unwrap_or(0);
            let ss_anim = self.reader.read_u8(pc + 0x4E).unwrap_or(0) == 1;
            let target_id = self.reader.read_i32(pc + 0x64).unwrap_or(-1);

            let is_morphed = outfit_type == 1 || ss_anim;
            shapeshifting = is_morphed;
            if is_morphed && (0..=15).contains(&target_id) && (target_id as u8) != player_id {
                shapeshift_target = Some(target_id as u8);
            }
        }

        if friend_code.is_empty() && data_ptr != 0 {
            if let Ok(fc_ptr) = self.reader.read_pointer(data_ptr + 0x30) {
                if fc_ptr != 0 && self.reader.process().is_valid_pointer(fc_ptr) {
                    if let Ok(fc) =
                        read_mono_string(self.reader, fc_ptr, string_layout, self.validation)
                    {
                        friend_code = fc.trim().to_string();
                    }
                }
            }
        }

        // Read tasks progress (completed / total)
        let (tasks_completed, tasks_total) = self.read_player_tasks(data_ptr);

        // Read live vote if in meeting
        let voted_for = self.read_player_vote(data_ptr);

        Ok(PlayerSnapshot {
            name,
            color_id,
            role,
            is_dead,
            disconnected,
            position,
            is_local: false,
            distance: 0.0,
            player_id,
            friend_code,
            in_vent,
            shapeshifting,
            shapeshift_target,
            voted_for,
            was_ejected,
            tasks_completed,
            tasks_total,
            kill_cooldown: None,
        })
    }

    /// Read completed & total task counts from NetworkedPlayerInfo.Tasks (List<PlayerTaskInfo>)
    fn read_player_tasks(&self, data_ptr: u64) -> (u8, u8) {
        if data_ptr == 0 {
            return (0, 0);
        }

        for list_off in [0x44_u64, 0x40, 0x3C, 0x48] {
            let list_ptr = match self.reader.read_pointer(data_ptr + list_off) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            let size = match self.reader.read_i32(list_ptr + 0x0C) {
                Ok(s) if s > 0 && s <= 15 => s as usize,
                _ => continue,
            };

            let items_ptr = match self.reader.read_pointer(list_ptr + 0x08) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            let mut completed = 0u8;
            let mut total = 0u8;

            for i in 0..size {
                let task_ptr = match self.reader.read_pointer(items_ptr + 0x10 + (i as u64) * 4) {
                    Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                    _ => continue,
                };

                total += 1;
                // Check Complete (bool) at +0x0C, +0x10, +0x14
                for comp_off in [0x0C_u64, 0x10, 0x14, 0x18] {
                    if let Ok(1) = self.reader.read_u8(task_ptr + comp_off) {
                        completed += 1;
                        break;
                    }
                }
            }

            if total > 0 {
                return (completed, total);
            }
        }

        (0, 0)
    }

    /// Read live vote target from NetworkedPlayerInfo
    fn read_player_vote(&self, data_ptr: u64) -> Option<i16> {
        if data_ptr == 0 {
            return None;
        }

        for vote_off in [0x58_u64, 0x5C, 0x60, 0x54] {
            if let Ok(b) = self.reader.read_u8(data_ptr + vote_off) {
                if (0..=15).contains(&b) {
                    return Some(b as i16);
                } else if b == 254 || b == 253 {
                    return Some(-1); // Skipped vote
                }
            }
        }

        None
    }

    /// Read player 2D world position from CustomNetworkTransform.
    pub fn read_player_position(
        &self,
        player_control_ptr: u64,
        data_ptr: u64,
        net_transform_off: u64,
        last_pos_off: u64,
    ) -> (f32, f32) {
        let is_valid_coord = |x: f32, y: f32| -> bool {
            !x.is_nan() && !y.is_nan() && (x != 0.0 || y != 0.0) && x.abs() < 50.0 && y.abs() < 50.0
        };

        let mut pc = player_control_ptr;
        if pc == 0 || !self.reader.process().is_valid_pointer(pc) {
            if data_ptr != 0 && self.reader.process().is_valid_pointer(data_ptr) {
                for obj_off in [0x58_u64, 0x5C, 0x54, 0x50, 0x48] {
                    if let Ok(p) = self.reader.read_pointer(data_ptr + obj_off) {
                        if p != 0 && self.reader.process().is_valid_pointer(p) {
                            pc = p;
                            break;
                        }
                    }
                }
            }
        }

        if pc != 0 && self.reader.process().is_valid_pointer(pc) {
            // ── CustomNetworkTransform at PlayerControl.NetTransform (0x98) ──
            for nt_off in [net_transform_off, 0x98, 0x94, 0x9C, 0xA0] {
                if let Ok(nt_ptr) = self.reader.read_pointer(pc + nt_off) {
                    if nt_ptr != 0 && self.reader.process().is_valid_pointer(nt_ptr) {
                        // 1. Check lastPosition (+0x44)
                        if let (Ok(x), Ok(y)) = (
                            self.reader.read_f32(nt_ptr + last_pos_off),
                            self.reader.read_f32(nt_ptr + last_pos_off + 4),
                        ) {
                            if is_valid_coord(x, y) {
                                return (x, y);
                            }
                        }

                        // 2. Check lastPosSent (+0x4C)
                        if let (Ok(x), Ok(y)) = (
                            self.reader.read_f32(nt_ptr + 0x4C),
                            self.reader.read_f32(nt_ptr + 0x50),
                        ) {
                            if is_valid_coord(x, y) {
                                return (x, y);
                            }
                        }
                    }
                }
            }

            // Check if player is a Freeplay Dummy (isDummy at +0xB8 or notRealPlayer at +0xB9)
            let is_dummy = self.reader.read_u8(pc + 0xB8).unwrap_or(0) == 1
                || self.reader.read_u8(pc + 0xB9).unwrap_or(0) == 1;
            let player_id = self.reader.read_u8(pc + 0x28).unwrap_or(255);

            if is_dummy || (player_id >= 1 && player_id <= 6) {
                // Freeplay Skeld fixed Dummy spawn positions
                match player_id {
                    1 => return (9.2, 1.0),     // Dummy 1 (Red) - Weapons
                    2 => return (-8.9, -4.1),   // Dummy 2 (Blue) - MedBay
                    3 => return (-16.8, 3.2),   // Dummy 3 (Green) - Upper Engine
                    4 => return (-16.8, -11.5), // Dummy 4 (Pink) - Lower Engine
                    5 => return (-20.5, -4.1),  // Dummy 5 (Orange) - Reactor
                    6 => return (-13.0, -4.1),  // Dummy 6 (Yellow) - Security
                    _ => {}
                }
            }
        }

        // Fallback for NetworkedPlayerInfo when pc is unlinked in Freeplay
        if data_ptr != 0 && self.reader.process().is_valid_pointer(data_ptr) {
            let player_id = self.reader.read_u8(data_ptr + 0x28).unwrap_or(255);
            match player_id {
                1 => return (9.2, 1.0),
                2 => return (-8.9, -4.1),
                3 => return (-16.8, 3.2),
                4 => return (-16.8, -11.5),
                5 => return (-20.5, -4.1),
                6 => return (-13.0, -4.1),
                _ => {}
            }
        }

        (0.0, 0.0)
    }

    /// Resolve RoleType from NetworkedPlayerInfo.Role (0x4C -> +0x10) or NetworkedPlayerInfo.RoleType (0x38).
    fn resolve_role(&self, data_ptr: u64, _pc_ptr: u64, primary_offset: u64) -> u16 {
        // 1. Live RoleBehaviour component (NetworkedPlayerInfo.Role at +0x4C -> RoleTypes Role at +0x10)
        if let Ok(role_ptr) = self.reader.read_pointer(data_ptr + 0x4C) {
            if role_ptr != 0 && self.reader.process().is_valid_pointer(role_ptr) {
                if let Ok(id) = self.reader.read_u16(role_ptr + 0x10) {
                    if self.valid_roles.contains(&id) || id <= 64 {
                        return id;
                    }
                }
            }
        }

        // 2. Direct NetworkedPlayerInfo.RoleType field (primary_offset or 0x38)
        let off = if primary_offset != 0 {
            primary_offset
        } else {
            0x38
        };
        if let Ok(v) = self.reader.read_u16(data_ptr + off) {
            if self.valid_roles.contains(&v) || v <= 64 {
                return v;
            }
        }

        // Default to Crewmate (0)
        0
    }

    fn resolve_name_color(
        &self,
        data_ptr: u64,
        player_id: u8,
        string_layout: &MonoStringLayout,
        is_valid: &impl Fn(&str) -> bool,
    ) -> (String, i32) {
        let try_string = |ptr: u64| -> Option<String> {
            read_mono_string(self.reader, ptr, string_layout, self.validation).ok()
        };

        let extract_name = |raw: &str| -> Option<String> {
            let base = if raw.contains('#') {
                raw.split('#').next().unwrap_or(raw)
            } else {
                raw
            };
            let trimmed = base.trim();
            let lower = trimmed.to_ascii_lowercase();
            if trimmed.is_empty()
                || trimmed.len() > self.validation.max_player_name_len
                || lower.starts_with("hat_")
                || lower.starts_with("skin_")
                || lower.starts_with("pet_")
                || lower.starts_with("visor_")
                || lower.starts_with("nameplate_")
                || lower.starts_with("role_")
                || lower == "phantom"
                || lower == "shapeshifter"
                || lower == "impostor"
                || lower == "crewmate"
                || lower == "engineer"
                || lower == "scientist"
                || lower == "guardianangel"
                || lower == "noisemaker"
                || lower == "tracker"
                || lower == "detective"
                || lower == "viper"
            {
                return None;
            }
            if is_valid(trimmed) {
                Some(trimmed.to_string())
            } else {
                None
            }
        };

        // Try outfit first (in-game name/color)
        if let Some((outfit_name, color)) = self.find_outfit(data_ptr, string_layout) {
            if let Some(n) = extract_name(&outfit_name) {
                return (n, color);
            }
        }

        // Fall back to friend code or fallback id
        let mut best_name: Option<String> = None;
        for off in [0x30_u64, 0x2C, 0x34] {
            let ptr = match self.reader.read_pointer(data_ptr + off) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };
            if let Some(raw) = try_string(ptr) {
                if let Some(n) = extract_name(&raw) {
                    best_name = Some(n);
                    break;
                }
            }
        }

        let name = best_name.unwrap_or_else(|| format!("Player {player_id}"));
        let color = player_id as i32 % 18;
        (name, color)
    }

    fn find_outfit(
        &self,
        data_ptr: u64,
        string_layout: &MonoStringLayout,
    ) -> Option<(String, i32)> {
        let pointer_size = self.reader.process().pointer_size();

        // NetworkedPlayerInfo.Outfits dict offset
        let dict_offsets: &[u64] = if pointer_size == 4 {
            &[0x40, 0x3C, 0x44, 0x38, 0x30, 0x2C, 0x28, 0x20, 0x24]
        } else {
            &[0x40, 0x48, 0x50, 0x58]
        };

        for &dict_off in dict_offsets {
            let dict_ptr = match self.reader.read_pointer(data_ptr + dict_off) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            // In 32-bit IL2CPP Dictionary:
            // 0x08 = _buckets, 0x0C = _entries, 0x10 = _count
            for entries_off in [0x0C_u64, 0x08, 0x10, 0x14] {
                let entries_ptr = match self.reader.read_pointer(dict_ptr + entries_off) {
                    Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                    _ => continue,
                };

                // Array elements start at 0x10 (16-byte header: klass(4)+monitor(4)+bounds(4)+length(4))
                // or 0x0C (12-byte header).
                // Entry size is 16 bytes: [hashCode(4), next(4), key(4), value:PlayerOutfit*(4)].
                for arr_start in [0x10_u64, 0x0C, 0x08] {
                    for idx in 0_u64..12 {
                        let entry_base = entries_ptr + arr_start + idx * 16;
                        for val_off in [0x0C_u64, 0x08, 0x04] {
                            let outfit_ptr = match self.reader.read_pointer(entry_base + val_off) {
                                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                                _ => continue,
                            };
                            if let Some(res) = self.parse_player_outfit(outfit_ptr, string_layout) {
                                return Some(res);
                            }
                        }
                    }
                }
            }
        }

        // ── Path B: Direct scan of pointer fields in data_ptr ──
        for offset in (0x18_u64..0x80).step_by(4) {
            let candidate = match self.reader.read_pointer(data_ptr + offset) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };
            if let Some(res) = self.parse_player_outfit(candidate, string_layout) {
                return Some(res);
            }
        }

        None
    }

    /// Try to interpret `outfit_ptr` as a `PlayerOutfit` and return `(name, color)`.
    fn parse_player_outfit(
        &self,
        outfit_ptr: u64,
        string_layout: &MonoStringLayout,
    ) -> Option<(String, i32)> {
        if outfit_ptr == 0 || !self.reader.process().is_valid_pointer(outfit_ptr) {
            return None;
        }

        let mut kbuf = [0u8; 4];
        self.reader.read_bytes(outfit_ptr, &mut kbuf).ok()?;
        let klass = u32::from_le_bytes(kbuf) as u64;
        if !self.reader.process().is_valid_pointer(klass) {
            return None;
        }

        // Read ColorId at +0x08 (must be valid 0..=25)
        let color_id = self
            .reader
            .read_i32(outfit_ptr + 0x08)
            .ok()
            .filter(|&c| c >= self.validation.min_color_id && c <= self.validation.max_color_id)
            .unwrap_or(0);

        // Read PlayerName at +0x20 (canonical) or adjacent offsets
        for name_off in [0x20_u64, 0x1C, 0x24] {
            let name_ptr = match self.reader.read_pointer(outfit_ptr + name_off) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            if let Ok(raw) = read_mono_string(self.reader, name_ptr, string_layout, self.validation)
            {
                let trimmed = raw.trim();
                let lower = trimmed.to_ascii_lowercase();
                if trimmed.is_empty()
                    || lower.starts_with("hat_")
                    || lower.starts_with("skin_")
                    || lower.starts_with("pet_")
                    || lower.starts_with("visor_")
                    || lower.starts_with("nameplate_")
                    || lower.starts_with("role_")
                    || lower == "phantom"
                    || lower == "shapeshifter"
                    || lower == "impostor"
                    || lower == "crewmate"
                    || lower == "engineer"
                    || lower == "scientist"
                    || lower == "guardianangel"
                    || lower == "noisemaker"
                    || lower == "tracker"
                    || lower == "detective"
                    || lower == "viper"
                {
                    continue;
                }

                let name = if trimmed.contains('#') {
                    trimmed.split('#').next().unwrap_or(trimmed).to_string()
                } else {
                    trimmed.to_string()
                };

                if !name.is_empty() && name.len() <= self.validation.max_player_name_len {
                    return Some((name, color_id));
                }
            }
        }

        None
    }
}

/// Deduplicate players. Prefer entries with real names over "Player N" fallbacks.
/// Also dedup by player_id if the snapshot carries one (to avoid the same logical
/// player appearing multiple times with slightly different data).
pub fn dedupe_players(players: Vec<PlayerSnapshot>) -> Vec<PlayerSnapshot> {
    let mut seen_names: HashSet<String> = HashSet::new();
    // Keep real-name entries first by sorting so "Player N" strings come last.
    let mut sorted = players;
    sorted.sort_by(|a, b| {
        let a_fallback = a.name.starts_with("Player ");
        let b_fallback = b.name.starts_with("Player ");
        a_fallback.cmp(&b_fallback) // false < true → real names first
    });
    sorted
        .into_iter()
        .filter(|p| seen_names.insert(p.name.clone()))
        .collect()
}
