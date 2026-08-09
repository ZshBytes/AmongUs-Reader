use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::dump::DumpConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct Offsets {
    #[serde(default)]
    pub dump: DumpConfig,
    pub process: ProcessConfig,
    pub il2cpp: Il2CppConfig,
    pub static_pointers: StaticPointers,
    pub static_fields: StaticFields,
    pub among_us_client: AmongUsClientFields,
    pub game_states: GameStatesConfig,
    pub list: ListLayout,
    pub array: ArrayLayout,
    pub player_control: PlayerControlFields,
    pub networked_player_info: NetworkedPlayerInfoFields,
    pub mono_string: MonoStringLayout,
    pub validation: ValidationConfig,
    pub runtime: RuntimeConfig,
    pub overlay: OverlayConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessConfig {
    pub executable_name: String,
    pub module_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Il2CppConfig {
    pub static_fields: u64,
}

impl Il2CppConfig {
    pub fn static_fields_offset(&self, pointer_size: u8) -> u64 {
        if pointer_size == 4 && self.static_fields == 0xB8 {
            // A 32-bit IL2CPP build still uses 0x5C for Il2CppClass.static_fields,
            // even though the x64 default is 0xB8.
            return 0x5C;
        }

        if self.static_fields != 0 {
            self.static_fields
        } else if pointer_size == 4 {
            0x5C
        } else {
            0xB8
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticPointers {
    pub player_control_type_info: u64,
    pub among_us_client_type_info: u64,
    pub game_data_type_info: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticFields {
    pub player_control_all_player_controls: u64,
    pub among_us_client_instance: u64,
    pub game_data_instance: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AmongUsClientFields {
    pub game_state: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameStatesConfig {
    pub active: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListLayout {
    pub items: u64,
    pub size: u64,
}

impl ListLayout {
    pub fn items_offset(&self, pointer_size: u8) -> u64 {
        // IL2CPP object pointers include the object header before field data.
        // On x86 the list object header is 8 bytes, so _items is at 0x8.
        if pointer_size == 4 && self.items == 0x10 {
            0x8
        } else if self.items != 0 {
            self.items
        } else if pointer_size == 4 {
            0x8
        } else {
            0x10
        }
    }

    pub fn size_offset(&self, pointer_size: u8) -> u64 {
        // On x86 the list _size field lives after the object header and the _items pointer.
        if pointer_size == 4 && self.size == 0x18 {
            0xC
        } else if self.size != 0 {
            self.size
        } else if pointer_size == 4 {
            0xC
        } else {
            0x18
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArrayLayout {
    pub first_element: u64,
    pub element_size: u64,
}

impl ArrayLayout {
    pub fn first_element_offset(&self, pointer_size: u8) -> u64 {
        if pointer_size == 4 && self.first_element == 0x20 {
            0x10
        } else if self.first_element != 0 {
            self.first_element
        } else if pointer_size == 4 {
            0x10
        } else {
            0x20
        }
    }

    pub fn element_size_bytes(&self, pointer_size: u8) -> u64 {
        if pointer_size == 4 && self.element_size == 8 {
            4
        } else if self.element_size != 0 {
            self.element_size
        } else {
            pointer_size as u64
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerControlFields {
    pub data: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkedPlayerInfoFields {
    pub player_name: u64,
    pub color_id: u64,
    pub role_type: u64,
    pub disconnected: u64,
    pub is_dead: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonoStringLayout {
    pub length: u64,
    pub chars: u64,
}

impl MonoStringLayout {
    pub fn length_offset(&self, pointer_size: u8) -> u64 {
        if pointer_size == 4 && self.length == 0x10 {
            0x8
        } else {
            self.length
        }
    }

    pub fn chars_offset(&self, pointer_size: u8) -> u64 {
        if pointer_size == 4 && self.chars == 0x14 {
            0xC
        } else {
            self.chars
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidationConfig {
    pub max_players: usize,
    pub min_player_name_len: usize,
    pub max_player_name_len: usize,
    pub min_color_id: i32,
    pub max_color_id: i32,
    pub valid_role_ids: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    pub poll_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OverlayConfig {
    pub width: u32,
    pub height: u32,
    pub position_x: i32,
    pub position_y: i32,
}

fn discover_dump_sources(base: &Path) -> DumpConfig {
    let mut search_dirs = Vec::new();
    let mut pending_dirs = vec![base.to_path_buf()];

    if let Ok(cwd) = std::env::current_dir() {
        pending_dirs.push(cwd);
    }
    pending_dirs.push(base.join("config"));
    pending_dirs.push(base.join("src"));
    pending_dirs.push(base.join("src/config"));
    pending_dirs.push(base.parent().unwrap_or(base).to_path_buf());

    let mut visited = HashSet::new();
    while let Some(dir) = pending_dirs.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        if !dir.exists() {
            continue;
        }

        search_dirs.push(dir.clone());

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        pending_dirs.push(entry.path());
                    }
                }
            }
        }
    }

    if let Ok(game_dir) = std::env::var("AMONG_US_GAME_DIR") {
        search_dirs.push(Path::new(&game_dir).to_path_buf());
    }

    let steam_paths = [
        r"C:\Program Files (x86)\Steam\steamapps\common\Among Us",
        r"C:\Program Files\Steam\steamapps\common\Among Us",
        r"D:\SteamLibrary\steamapps\common\Among Us",
        r"C:\Users\szuwer\Documents\coding\rust\Among US - external",
        r"C:\Users\szuwer\Documents",
        r"C:\Users\szuwer\Desktop",
    ];
    for path in steam_paths {
        search_dirs.push(Path::new(path).to_path_buf());
    }

    let pick = |filename: &str| -> Option<String> {
        for dir in &search_dirs {
            let candidate = dir.join(filename);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    };

    let pick_any = |names: &[&str]| -> Option<String> {
        for name in names {
            if let Some(path) = pick(name) {
                return Some(path);
            }
        }
        None
    };

    DumpConfig {
        script_json: pick_any(&["script.json", "ScriptingAssemblies.json"]),
        il2cpp_h: pick_any(&["il2cpp.h"]),
        dump_cs: pick_any(&["dump.cs"]),
    }
}

impl Offsets {
    pub fn load(path: impl AsRef<Path>) -> Result<(Self, Vec<String>), ConfigError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
        let mut offsets: Offsets = toml::from_str(&text).map_err(ConfigError::Parse)?;
        offsets.validate()?;

        let mut notes = Vec::new();
        let base = path.parent().unwrap_or_else(|| Path::new("."));

        let mut dump = DumpConfig {
            script_json: offsets
                .dump
                .script_json
                .as_deref()
                .map(|p| super::dump::resolve_dump_path(base, p)),
            il2cpp_h: offsets
                .dump
                .il2cpp_h
                .as_deref()
                .map(|p| super::dump::resolve_dump_path(base, p)),
            dump_cs: offsets
                .dump
                .dump_cs
                .as_deref()
                .map(|p| super::dump::resolve_dump_path(base, p)),
        };

        if dump.script_json.is_none() && dump.il2cpp_h.is_none() && dump.dump_cs.is_none() {
            dump = discover_dump_sources(base);
        }

        if dump.script_json.is_some() || dump.il2cpp_h.is_some() || dump.dump_cs.is_some() {
            notes.extend(offsets.apply_dump_sources(&dump)?);
        }

        if !offsets.offsets_configured() {
            notes.push(
                "TypeInfo offsets still missing — place script.json / il2cpp.h / dump.cs next to offsets.toml or add [dump] paths"
                    .into(),
            );
        } else {
            notes.push(format!(
                "offsets ready (PlayerControl=0x{:X}, AmongUsClient=0x{:X})",
                offsets.static_pointers.player_control_type_info,
                offsets.static_pointers.among_us_client_type_info,
            ));
        }

        Ok((offsets, notes))
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.validation.valid_role_ids.is_empty() {
            return Err(ConfigError::MissingOffset(
                "validation.valid_role_ids must not be empty".into(),
            ));
        }
        Ok(())
    }

    pub fn offsets_configured(&self) -> bool {
        self.static_pointers.player_control_type_info != 0
            && self.static_pointers.among_us_client_type_info != 0
    }

    pub fn active_game_states(&self) -> HashSet<i32> {
        self.game_states.active.iter().copied().collect()
    }

    pub fn valid_roles(&self) -> HashSet<u16> {
        self.validation.valid_role_ids.iter().copied().collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read offsets file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse offsets file: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to parse dump JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("invalid offsets configuration: {0}")]
    MissingOffset(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_dump_sources_finds_nested_files() {
        let temp = std::env::temp_dir().join(format!("among-us-offsets-{}", std::process::id()));
        let nested = temp.join("src/config");
        std::fs::create_dir_all(&nested).unwrap();
        let script = nested.join("script.json");
        std::fs::write(&script, r#"[]"#).unwrap();

        let dump = discover_dump_sources(&temp);
        assert_eq!(dump.script_json.as_deref(), Some(script.to_str().unwrap()));

        std::fs::remove_file(script).unwrap();
        std::fs::remove_dir_all(temp).unwrap();
    }
}
