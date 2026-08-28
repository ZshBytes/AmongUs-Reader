use std::collections::HashSet;

use crate::config::{MonoStringLayout, NetworkedPlayerInfoFields, ValidationConfig};
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
        string_layout: &MonoStringLayout,
    ) -> Result<PlayerSnapshot, MemoryError> {
        let data_ptr = self.reader.read_pointer(player_control_ptr + data_offset)?;
        self.read_player_data(data_ptr, player_control_ptr, info, string_layout)
    }

    pub fn read_player_data(
        &self,
        data_ptr: u64,
        player_control_ptr: u64,
        info: &NetworkedPlayerInfoFields,
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
        let disconnected_byte = self.reader.read_u8(data_ptr + info.disconnected).unwrap_or(0);
        if disconnected_byte > 1 {
            return Err(MemoryError::InvalidPointer(data_ptr));
        }
        let disconnected = disconnected_byte == 1;

        // is_dead: strictly check NetworkedPlayerInfo.IsDead at 0x54.
        let is_dead = self.reader.read_u8(data_ptr + info.is_dead)
            .map(|b| b == 1)
            .unwrap_or(false);

        // RoleType: read from RoleBehaviour (+0x4C -> +0x10) or NetworkedPlayerInfo.RoleType (+0x38).
        let role_raw = self.resolve_role(data_ptr, player_control_ptr, info.role_type);
        let role = RoleType::from_id(role_raw, &self.valid_roles).unwrap_or(RoleType::Crewmate);

        // Name + color resolution.
        let is_valid_player_name = |n: &str| -> bool {
            if n.is_empty() || n.len() > self.validation.max_player_name_len { return false; }
            let lower = n.to_lowercase();
            const INVALID: &[&str] = &[
                "weapons", "shields", "navigation", "reactor", "o2", "security",
                "medbay", "electrical", "cafeteria", "storage", "admin", "communications",
                "upper engine", "lower engine", "office", "laboratory", "specimen room",
                "decontamination", "main hall", "engine room", "cockpit", "vault",
                "showers", "lounge", "cargo bay", "records", "gap room", "meeting room",
                "tasks", "cancel", "use", "report", "kill", "sabotage", "vent",
                "untagged", "gameobject", "transform", "maincamera", "camera", "canvas",
                "eventsystem", "default", "sprite", "audio", "sound", "manager", "controller",
            ];
            !INVALID.contains(&lower.as_str())
        };

        let (name, color_id) = self.resolve_name_color(
            data_ptr, player_id, string_layout, &is_valid_player_name,
        );

        Ok(PlayerSnapshot {
            name,
            color_id,
            role,
            is_dead,
            disconnected,
        })
    }

    /// Resolve RoleType from NetworkedPlayerInfo.RoleType (0x38) or live RoleBehaviour (0x4C -> +0x10).
    fn resolve_role(&self, data_ptr: u64, _pc_ptr: u64, primary_offset: u64) -> u16 {
        // 1. Live RoleBehaviour component (+0x4C -> +0x10 RoleTypes Role)
        if let Ok(role_ptr) = self.reader.read_pointer(data_ptr + 0x4C) {
            if role_ptr != 0 && self.reader.process().is_valid_pointer(role_ptr) {
                if let Ok(id) = self.reader.read_u16(role_ptr + 0x10) {
                    if self.valid_roles.contains(&id) {
                        return id;
                    }
                }
            }
        }

        // 2. NetworkedPlayerInfo.RoleType (offset 0x38, ushort: 0=Crewmate, 1=Impostor, etc.)
        if let Ok(v) = self.reader.read_u16(data_ptr + primary_offset) {
            if self.valid_roles.contains(&v) {
                return v;
            }
        }

        // Default to Crewmate (0)
        0
    }

    /// Resolve the player's display name and color.
    ///
    /// Priority:
    /// 1. PlayerOutfit.PlayerName at +0x20 and ColorId at +0x08 (canonical in-game nickname & color)
    /// 2. NetworkedPlayerInfo.FriendCode at +0x30 (stripped of #tag if needed)
    /// 3. "Player {id}" / (id % 18) fallback
    fn resolve_name_color(
        &self,
        data_ptr: u64,
        player_id: u8,
        string_layout: &MonoStringLayout,
        is_valid: &impl Fn(&str) -> bool,
    ) -> (String, i32) {
        // Helper: try to read a mono string from a heap address.
        let try_string = |ptr: u64| -> Option<String> {
            read_mono_string(self.reader, ptr, string_layout, self.validation).ok()
        };

        // Extract base name from a raw string.
        let extract_name = |raw: &str| -> Option<String> {
            let base = if raw.contains('#') {
                raw.split('#').next().unwrap_or(raw)
            } else {
                raw
            };
            let trimmed = base.trim();
            if trimmed.is_empty() || trimmed.len() > self.validation.max_player_name_len {
                return None;
            }
            if is_valid(trimmed) {
                Some(trimmed.to_string())
            } else {
                None
            }
        };

        // ─── Phase 1: Try PlayerOutfit (canonical source for nickname & color) ───
        if let Some((outfit_name, color)) = self.find_outfit(data_ptr, string_layout) {
            if let Some(n) = extract_name(&outfit_name) {
                return (n, color);
            }
        }

        // ─── Phase 2: Fallback to FriendCode or Player {id} ───
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

    /// Try to find a `PlayerOutfit` object and return (name, color).
    fn find_outfit(
        &self,
        data_ptr: u64,
        string_layout: &MonoStringLayout,
    ) -> Option<(String, i32)> {
        let pointer_size = self.reader.process().pointer_size();

        // ── Path A: Dictionary traversal ──
        // NetworkedPlayerInfo.Outfits is at 0x40.
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
        let color_id = self.reader
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

            if let Ok(raw) = read_mono_string(self.reader, name_ptr, string_layout, self.validation) {
                let trimmed = raw.trim();
                if trimmed.is_empty() { continue; }
                if trimmed.starts_with("hat_") || trimmed.starts_with("skin_") || trimmed.starts_with("pet_") || trimmed.starts_with("visor_") || trimmed.starts_with("nameplate_") {
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
