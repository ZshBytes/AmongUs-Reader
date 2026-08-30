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
    #[serde(default)]
    pub custom_network_transform: CustomNetworkTransformFields,
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
pub struct CustomNetworkTransformFields {
    #[serde(default = "default_net_transform")]
    pub net_transform: u64,
    #[serde(default = "default_last_position")]
    pub last_position: u64,
}

fn default_net_transform() -> u64 {
    0x98
}

fn default_last_position() -> u64 {
    0x44
}

impl Default for CustomNetworkTransformFields {
    fn default() -> Self {
        Self {
            net_transform: default_net_transform(),
            last_position: default_last_position(),
        }
    }
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
    #[serde(default = "default_toggle_key")]
    pub toggle_key: String,
}

fn default_toggle_key() -> String {
    "Insert".to_string()
}

impl OverlayConfig {
    pub fn toggle_key_vk(&self) -> i32 {
        parse_vk_key(&self.toggle_key)
    }
}

pub fn parse_vk_key(name: &str) -> i32 {
    match name.trim().to_uppercase().as_str() {
        "INSERT" | "INS" => 0x2D, // VK_INSERT
        "DELETE" | "DEL" => 0x2E, // VK_DELETE
        "HOME" => 0x24,           // VK_HOME
        "END" => 0x23,            // VK_END
        "PAGEUP" | "PGUP" | "PRIOR" => 0x21, // VK_PRIOR
        "PAGEDOWN" | "PGDN" | "NEXT" => 0x22, // VK_NEXT
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        "F11" => 0x7A,
        "F12" => 0x7B,
        "TAB" => 0x09,
        "CAPSLOCK" | "CAPS" => 0x14,
        "ESCAPE" | "ESC" => 0x1B,
        "SPACE" => 0x20,
        "BACKSPACE" | "BACK" => 0x08,
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            if c.is_ascii_alphanumeric() {
                c as i32
            } else {
                0x2D
            }
        }
        _ => 0x2D,
    }
}

fn discover_dump_sources(base: &Path) -> DumpConfig {
    let mut search_dirs = Vec::new();
    let mut pending_dirs = std::collections::VecDeque::new();
    pending_dirs.push_back(base.to_path_buf());
    pending_dirs.push_back(base.join("config"));
    pending_dirs.push_back(base.join("src"));
    pending_dirs.push_back(base.join("src").join("config"));
    if let Ok(cwd) = std::env::current_dir() {
        if cwd != base {
            pending_dirs.push_back(cwd);
        }
    }
    if let Some(parent) = base.parent() {
        pending_dirs.push_back(parent.to_path_buf());
    }

    let mut visited = HashSet::new();
    while let Some(dir) = pending_dirs.pop_front() {
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
                        pending_dirs.push_back(entry.path());
                    }
                }
            }
        }
    }

    if let Ok(game_dir) = std::env::var("AMONG_US_GAME_DIR") {
        search_dirs.push(Path::new(&game_dir).to_path_buf());
    }

    let drives = ["C", "D", "E", "F", "G", "H"];
    for drive in drives {
        let steam_paths = [
            format!(r"{drive}:\Steam\steamapps\common\Among Us"),
            format!(r"{drive}:\Program Files (x86)\Steam\steamapps\common\Among Us"),
            format!(r"{drive}:\Program Files\Steam\steamapps\common\Among Us"),
            format!(r"{drive}:\SteamLibrary\steamapps\common\Among Us"),
            format!(r"{drive}:\Epic Games\AmongUs"),
            format!(r"{drive}:\XboxGames\Among Us\Content"),
        ];
        for path in steam_paths {
            search_dirs.push(Path::new(&path).to_path_buf());
        }
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
    pub const DEFAULT_CONFIG_TOML: &'static str = include_str!("../../offsets.toml");

    pub fn load(path: impl AsRef<Path>) -> Result<(Self, Vec<String>), ConfigError> {
        let path = path.as_ref();
        let (text, base) = if path.exists() {
            let content = fs::read_to_string(path).map_err(ConfigError::Io)?;
            let b = path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
            (content, b)
        } else {
            let b = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            (Self::DEFAULT_CONFIG_TOML.to_string(), b)
        };

        let mut offsets: Offsets = toml::from_str(&text).map_err(ConfigError::Parse)?;
        offsets.validate()?;

        let mut notes = Vec::new();
        let base = base.as_path();

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
        let nested = temp.join("src").join("config");
        std::fs::create_dir_all(&nested).unwrap();
        let script = nested.join("script.json");
        std::fs::write(&script, r#"[]"#).unwrap();

        let dump = discover_dump_sources(&temp);
        assert_eq!(
            dump.script_json.as_deref().map(Path::new).map(std::path::Path::to_path_buf),
            Some(script.to_path_buf())
        );

        std::fs::remove_file(script).unwrap();
        std::fs::remove_dir_all(temp).unwrap();
    }
}
