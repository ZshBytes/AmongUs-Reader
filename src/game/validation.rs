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

        if data_ptr == 0 || !self.reader.process().is_valid_pointer(data_ptr) {
            eprintln!("[read_player] invalid data_ptr=0x{data_ptr:X} for player_control_ptr=0x{player_control_ptr:X}");
            return Err(MemoryError::InvalidPointer(data_ptr));
        }

        let disconnected = self.reader.read_bool(data_ptr + info.disconnected).unwrap_or(false);
        if disconnected {
            return Err(MemoryError::InvalidPointer(data_ptr));
        }

        let is_dead = self.reader.read_bool(data_ptr + info.is_dead).unwrap_or(false);
        let role_raw = self.reader.read_u16(data_ptr + info.role_type).unwrap_or(0);
        let role = RoleType::from_id(role_raw, &self.valid_roles)
            .unwrap_or(RoleType::Crewmate);

        // Try direct reading of name & color
        let direct_name = self
            .reader
            .read_pointer(data_ptr + info.player_name)
            .ok()
            .and_then(|ptr| read_mono_string(self.reader, ptr, string_layout, self.validation).ok());

        let direct_color = self.reader.read_i32(data_ptr + info.color_id).ok();

        let (name, color_id) = match (direct_name.clone(), direct_color) {
            (Some(n), Some(c)) if !n.is_empty() && c >= self.validation.min_color_id && c <= self.validation.max_color_id => (n, c),
            _ => {
                // Try reading from PlayerOutfit inside Outfits dictionary (0x40) or outfits pointer offsets
                match self.try_read_outfit(data_ptr, string_layout) {
                    Some(res) => res,
                    None => {
                        eprintln!("[read_player] failed to extract name/color for data_ptr=0x{data_ptr:X} (direct name={:?}, color={:?})", direct_name, direct_color);
                        return Err(MemoryError::InvalidPointer(data_ptr));
                    }
                }
            }
        };

        eprintln!("[read_player] read player: name='{name}' color={color_id} role={:?} dead={is_dead}", role);

        Ok(PlayerSnapshot {
            name,
            color_id,
            role,
            is_dead,
            disconnected: false,
        })
    }

    fn try_read_outfit(
        &self,
        data_ptr: u64,
        string_layout: &MonoStringLayout,
    ) -> Option<(String, i32)> {
        let pointer_size = self.reader.process().pointer_size();

        // 1. Try reading via NetworkedPlayerInfo.Outfits dictionary (offset 0x40)
        let outfits_dict_ptr = self.reader.read_pointer(data_ptr + 0x40).unwrap_or(0);
        if outfits_dict_ptr != 0 && self.reader.process().is_valid_pointer(outfits_dict_ptr) {
            // Dictionary fields (x86):
            // 0x00: klass
            // 0x04: monitor
            // 0x08: _buckets (int32[])
            // 0x0C: _entries (Entry[])
            // 0x10: _count (int32)
            for entries_offset in [0x0C, 0x10, 0x14, 0x18] {
                let entries_ptr = match self.reader.read_pointer(outfits_dict_ptr + entries_offset) {
                    Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                    _ => continue,
                };

                // Array object (x86):
                // 0x00: klass
                // 0x04: monitor
                // 0x08: bounds
                // 0x0C: max_length
                // 0x10: vector elements start here
                let array_start = if pointer_size == 4 { 0x10 } else { 0x20 };

                // Entry<int, PlayerOutfit*> struct (x86):
                // 0x00: hashCode (int32)
                // 0x04: next (int32)
                // 0x08: key (PlayerOutfitType / int32)
                // 0x0C: value (PlayerOutfit pointer)
                // Entry size = 16 bytes (0x10)
                for entry_idx in 0..6 {
                    let entry_addr = entries_ptr + array_start + (entry_idx as u64 * 0x10);
                    let outfit_ptr = match self.reader.read_pointer(entry_addr + 0x0C) {
                        Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                        _ => {
                            // Try key at 0x8 and value at 0x0C or alternate offsets
                            match self.reader.read_pointer(entry_addr + 0x08) {
                                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                                _ => continue,
                            }
                        }
                    };

                    if let Some(res) = self.parse_player_outfit(outfit_ptr, string_layout) {
                        return Some(res);
                    }
                }
            }
        }

        // 2. Fallback: Scan fields inside data_ptr (0x0 to 0x70) for any valid PlayerOutfit or MonoString pointers
        for offset in (0..0x68).step_by(4) {
            let candidate_ptr = match self.reader.read_pointer(data_ptr + offset) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            // Check if candidate_ptr points to PlayerOutfit struct
            if let Some(res) = self.parse_player_outfit(candidate_ptr, string_layout) {
                return Some(res);
            }

            // Check if candidate_ptr is directly a MonoString (PlayerName)
            if let Ok(name) = read_mono_string(self.reader, candidate_ptr, string_layout, self.validation) {
                if !name.is_empty() && name.len() <= self.validation.max_player_name_len {
                    // Search nearby offsets for color_id
                    for color_offset in [data_ptr + 0x8, data_ptr + 0x28, data_ptr + 0x2C, data_ptr + 0x70] {
                        if let Ok(c) = self.reader.read_i32(color_offset) {
                            if c >= self.validation.min_color_id && c <= self.validation.max_color_id {
                                return Some((name, c));
                            }
                        }
                    }
                    return Some((name, 0));
                }
            }
        }

        None
    }

    fn parse_player_outfit(
        &self,
        outfit_ptr: u64,
        string_layout: &MonoStringLayout,
    ) -> Option<(String, i32)> {
        if outfit_ptr == 0 || !self.reader.process().is_valid_pointer(outfit_ptr) {
            return None;
        }

        // PlayerOutfit layout (x86):
        // 0x00: klass
        // 0x04: monitor
        // 0x08: ColorId (int32)
        // 0x0C: HatId (string ptr)
        // 0x10: PetId (string ptr)
        // 0x14: SkinId (string ptr)
        // 0x18: VisorId (string ptr)
        // 0x1C: NamePlateId (string ptr)
        // 0x20: PlayerName (string ptr)
        let color_id = match self.reader.read_i32(outfit_ptr + 0x8) {
            Ok(c) if c >= self.validation.min_color_id && c <= self.validation.max_color_id => c,
            _ => return None,
        };

        // Try reading PlayerName from 0x20, 0x1C, 0x24, 0xC
        for name_offset in [0x20, 0x1C, 0x24, 0x0C] {
            let name_ptr = match self.reader.read_pointer(outfit_ptr + name_offset) {
                Ok(p) if p != 0 && self.reader.process().is_valid_pointer(p) => p,
                _ => continue,
            };

            if let Ok(name) = read_mono_string(self.reader, name_ptr, string_layout, self.validation) {
                if !name.is_empty() && name.len() <= self.validation.max_player_name_len {
                    return Some((name, color_id));
                }
            }
        }

        None
    }
}

pub fn dedupe_players(players: Vec<PlayerSnapshot>) -> Vec<PlayerSnapshot> {
    let mut seen = HashSet::new();
    players
        .into_iter()
        .filter(|p| seen.insert(p.name.clone()))
        .collect()
}
