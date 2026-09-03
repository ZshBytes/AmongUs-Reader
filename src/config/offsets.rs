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
    #[serde(default)]
    #[allow(dead_code)]
    pub game_options: GameOptionsFields,
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
            // 32-bit il2cpp uses 0x5C for static_fields instead of 0xB8
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
    #[serde(default = "default_game_options_manager_type_info")]
    pub game_options_manager_type_info: u64,
    #[serde(default = "default_meeting_hud_type_info")]
    pub meeting_hud_type_info: u64,
    #[serde(default = "default_game_code_type_info")]
    pub game_code_type_info: u64,
}

fn default_game_options_manager_type_info() -> u64 {
    0x2AE987C
}

fn default_meeting_hud_type_info() -> u64 {
    0x2AC8E84
}

fn default_game_code_type_info() -> u64 {
    0x2AE8F80
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct StaticFields {
    pub player_control_all_player_controls: u64,
    pub among_us_client_instance: u64,
    pub game_data_instance: u64,
    #[serde(default)]
    pub game_options_manager_instance: u64,
    #[serde(default)]
    pub meeting_hud_instance: u64,
    #[serde(default)]
    pub game_code_v2: u64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GameOptionsFields {
    #[serde(default = "default_current_game_options")]
    pub current_game_options: u64,
    #[serde(default = "default_map_id")]
    pub map_id: u64,
    #[serde(default = "default_player_speed_mod")]
    pub player_speed_mod: u64,
    #[serde(default = "default_crew_light_mod")]
    pub crew_light_mod: u64,
    #[serde(default = "default_impostor_light_mod")]
    pub impostor_light_mod: u64,
    #[serde(default = "default_kill_cooldown")]
    pub kill_cooldown: u64,
    #[serde(default = "default_num_common_tasks")]
    pub num_common_tasks: u64,
    #[serde(default = "default_num_long_tasks")]
    pub num_long_tasks: u64,
    #[serde(default = "default_num_short_tasks")]
    pub num_short_tasks: u64,
    #[serde(default = "default_num_emergency_meetings")]
    pub num_emergency_meetings: u64,
    #[serde(default = "default_emergency_cooldown")]
    pub emergency_cooldown: u64,
    #[serde(default = "default_num_impostors")]
    pub num_impostors: u64,
    #[serde(default = "default_kill_distance")]
    pub kill_distance: u64,
    #[serde(default = "default_discussion_time")]
    pub discussion_time: u64,
    #[serde(default = "default_voting_time")]
    pub voting_time: u64,
    #[serde(default = "default_confirm_impostor")]
    pub confirm_impostor: u64,
    #[serde(default = "default_visual_tasks")]
    pub visual_tasks: u64,
    #[serde(default = "default_anonymous_votes")]
    pub anonymous_votes: u64,
    #[serde(default = "default_role_options")]
    pub role_options: u64,
}

fn default_current_game_options() -> u64 {
    0x14
}
fn default_map_id() -> u64 {
    0x14
}
fn default_player_speed_mod() -> u64 {
    0x18
}
fn default_crew_light_mod() -> u64 {
    0x1C
}
fn default_impostor_light_mod() -> u64 {
    0x20
}
fn default_kill_cooldown() -> u64 {
    0x24
}
fn default_num_common_tasks() -> u64 {
    0x28
}
fn default_num_long_tasks() -> u64 {
    0x2C
}
fn default_num_short_tasks() -> u64 {
    0x30
}
fn default_num_emergency_meetings() -> u64 {
    0x34
}
fn default_emergency_cooldown() -> u64 {
    0x38
}
fn default_num_impostors() -> u64 {
    0x3C
}
fn default_kill_distance() -> u64 {
    0x44
}
fn default_discussion_time() -> u64 {
    0x48
}
fn default_voting_time() -> u64 {
    0x4C
}
fn default_confirm_impostor() -> u64 {
    0x50
}
fn default_visual_tasks() -> u64 {
    0x51
}
fn default_anonymous_votes() -> u64 {
    0x52
}
fn default_role_options() -> u64 {
    0x60
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
        // on 32-bit il2cpp list _items is at 0x8 instead of 0x10
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
        // on 32-bit list _size is at 0xC instead of 0x18
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
        "INSERT" | "INS" => 0x2D,             // VK_INSERT
        "DELETE" | "DEL" => 0x2E,             // VK_DELETE
        "HOME" => 0x24,                       // VK_HOME
        "END" => 0x23,                        // VK_END
        "PAGEUP" | "PGUP" | "PRIOR" => 0x21,  // VK_PRIOR
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
    let search_dirs = [
        base.to_path_buf(),
        base.join("config"),
        base.join("src"),
        base.join("src").join("config"),
        Path::new(".").to_path_buf(),
        Path::new("config").to_path_buf(),
    ];

    let pick_file = |name: &str| -> Option<String> {
        for dir in &search_dirs {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    };

    DumpConfig {
        script_json: pick_file("script.json").or_else(|| pick_file("ScriptingAssemblies.json")),
        il2cpp_h: pick_file("il2cpp.h"),
        dump_cs: pick_file("dump.cs"),
    }
}

impl Offsets {
    pub const DEFAULT_CONFIG_TOML: &'static str = include_str!("../../offsets.toml");

    pub fn load(path: impl AsRef<Path>) -> Result<(Self, Vec<String>), ConfigError> {
        let path = path.as_ref();
        let (text, base) = if path.exists() {
            let content = fs::read_to_string(path).map_err(ConfigError::Io)?;
            let b = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
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

        let has_explicit_dump = dump.script_json.is_some() || dump.il2cpp_h.is_some() || dump.dump_cs.is_some();
        if has_explicit_dump || !offsets.offsets_configured() {
            if !has_explicit_dump {
                dump = discover_dump_sources(base);
            }
            if dump.script_json.is_some() || dump.il2cpp_h.is_some() || dump.dump_cs.is_some() {
                notes.extend(offsets.apply_dump_sources(&dump)?);
            }
        }

        if !offsets.offsets_configured() {
            notes.push(
                "TypeInfo offsets missing. Place script.json/dump.cs next to offsets.toml or configure manually."
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
            dump.script_json
                .as_deref()
                .map(Path::new)
                .map(std::path::Path::to_path_buf),
            Some(script.to_path_buf())
        );

        std::fs::remove_file(script).unwrap();
        std::fs::remove_dir_all(temp).unwrap();
    }
}
