//! Parse Il2CppDumper output to fill offsets automatically.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::offsets::{
    AmongUsClientFields, ConfigError, NetworkedPlayerInfoFields, Offsets, PlayerControlFields,
    StaticFields, StaticPointers,
};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DumpConfig {
    pub script_json: Option<String>,
    pub il2cpp_h: Option<String>,
    pub dump_cs: Option<String>,
}

#[derive(Debug, Clone)]
struct ScriptEntry {
    name: String,
    address: Value,
}

const TYPEINFO_TARGETS: [(&str, fn(&mut StaticPointers, u64)); 3] = [
    (
        "PlayerControl_TypeInfo",
        |p, v| p.player_control_type_info = v,
    ),
    (
        "AmongUsClient_TypeInfo",
        |p, v| p.among_us_client_type_info = v,
    ),
    ("GameData_TypeInfo", |p, v| p.game_data_type_info = v),
];

const STATIC_FIELD_TARGETS: [(&str, &str, fn(&mut StaticFields, u64)); 3] = [
    (
        "PlayerControl_StaticFields",
        "AllPlayerControls",
        |s, v| s.player_control_all_player_controls = v,
    ),
    (
        "AmongUsClient_StaticFields",
        "Instance",
        |s, v| s.among_us_client_instance = v,
    ),
    (
        "GameData_StaticFields",
        "Instance",
        |s, v| s.game_data_instance = v,
    ),
];

impl Offsets {
    pub fn apply_dump_sources(&mut self, dump: &DumpConfig) -> Result<Vec<String>, ConfigError> {
        let mut notes = Vec::new();

        if let Some(path) = dump.script_json.as_deref() {
            notes.extend(apply_script_json(&mut self.static_pointers, path)?);
        }

        if let Some(path) = dump.il2cpp_h.as_deref() {
            notes.extend(apply_il2cpp_h(&mut self.static_fields, path)?);
        }

        if let Some(path) = dump.dump_cs.as_deref() {
            notes.extend(apply_static_field_offsets_from_dump_cs(
                &mut self.static_fields,
                path,
            )?);
            notes.extend(apply_dump_cs(
                &mut self.player_control,
                &mut self.networked_player_info,
                &mut self.among_us_client,
                path,
            )?);
        }

        Ok(notes)
    }
}

fn apply_script_json(
    pointers: &mut StaticPointers,
    path: &str,
) -> Result<Vec<String>, ConfigError> {
    let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
    let mut notes = Vec::new();

    let parsed = serde_json::from_str::<Value>(&text).map_err(ConfigError::JsonParse)?;
    let entries = parse_script_entries(&parsed);

    for (target, setter) in TYPEINFO_TARGETS {
        if let Some(entry) = entries.iter().find(|entry| entry.name == target) {
            let value = parse_script_address(&entry.address)?;
            setter(pointers, value);
            notes.push(format!("loaded {target} = 0x{value:X}"));
        } else {
            notes.push(format!("missing {target} in script.json"));
        }
    }
    Ok(notes)
}

fn parse_script_entries(value: &Value) -> Vec<ScriptEntry> {
    let mut entries = Vec::new();
    if let Value::Object(map) = value {
        if let Some(metadata) = map.get("ScriptMetadata") {
            collect_script_entries(metadata, &mut entries, None);
        }
    }
    collect_script_entries(value, &mut entries, None);
    entries
}

fn collect_script_entries(value: &Value, entries: &mut Vec<ScriptEntry>, parent_name: Option<&str>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_script_entries(item, entries, None);
            }
        }
        Value::Object(map) => {
            if let Some(name) = extract_script_name(map, parent_name) {
                if let Some(address) = extract_script_address(map) {
                    entries.push(ScriptEntry {
                        name: name.to_string(),
                        address: address.clone(),
                    });
                    return;
                }
            }

            for (key, child) in map {
                collect_script_entries(child, entries, Some(key.as_str()));
            }
        }
        Value::String(text) => {
            if let Some(name) = parent_name {
                entries.push(ScriptEntry {
                    name: name.to_string(),
                    address: Value::String(text.clone()),
                });
            }
        }
        Value::Number(number) => {
            if let Some(name) = parent_name {
                entries.push(ScriptEntry {
                    name: name.to_string(),
                    address: Value::Number(number.clone()),
                });
            }
        }
        Value::Bool(value) => {
            if let Some(name) = parent_name {
                entries.push(ScriptEntry {
                    name: name.to_string(),
                    address: Value::Bool(*value),
                });
            }
        }
        Value::Null => {}
    }
}

fn extract_script_name(map: &serde_json::Map<String, Value>, parent_name: Option<&str>) -> Option<String> {
    map.get("Name")
        .or_else(|| map.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(parent_name.map(str::to_string))
}

fn extract_script_address(map: &serde_json::Map<String, Value>) -> Option<&Value> {
    map.get("Address")
        .or_else(|| map.get("address"))
        .or_else(|| map.get("Value"))
        .or_else(|| map.get("value"))
}

fn apply_il2cpp_h(fields: &mut StaticFields, path: &str) -> Result<Vec<String>, ConfigError> {
    let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
    let mut notes = Vec::new();

    for (struct_name, field_name, setter) in STATIC_FIELD_TARGETS {
        match parse_struct_field_offset(&text, struct_name, field_name) {
            Some(offset) => {
                setter(fields, offset);
                notes.push(format!(
                    "loaded {struct_name}.{field_name} = 0x{offset:X}"
                ));
            }
            None => {}
        }
    }
    Ok(notes)
}

fn apply_static_field_offsets_from_dump_cs(
    fields: &mut StaticFields,
    path: &str,
) -> Result<Vec<String>, ConfigError> {
    let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
    let mut notes = Vec::new();

    for (class_name, field_name, setter) in [
        (
            "PlayerControl",
            "AllPlayerControls",
            set_player_control_all_player_controls as fn(&mut StaticFields, u64),
        ),
        (
            "AmongUsClient",
            "Instance",
            set_among_us_client_instance as fn(&mut StaticFields, u64),
        ),
        (
            "GameData",
            "Instance",
            set_game_data_instance as fn(&mut StaticFields, u64),
        ),
    ] {
        if let Some(offset) = parse_dump_field(&text, class_name, field_name) {
            setter(fields, offset);
            notes.push(format!("loaded {class_name}.{field_name} = 0x{offset:X}"));
        } else {
            notes.push(format!("missing {class_name}.{field_name} in dump.cs"));
        }
    }

    Ok(notes)
}

fn set_player_control_all_player_controls(fields: &mut StaticFields, value: u64) {
    fields.player_control_all_player_controls = value;
}

fn set_among_us_client_instance(fields: &mut StaticFields, value: u64) {
    fields.among_us_client_instance = value;
}

fn set_game_data_instance(fields: &mut StaticFields, value: u64) {
    fields.game_data_instance = value;
}

fn apply_dump_cs(
    player_control: &mut PlayerControlFields,
    info: &mut NetworkedPlayerInfoFields,
    client: &mut AmongUsClientFields,
    path: &str,
) -> Result<Vec<String>, ConfigError> {
    let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
    let mut notes = Vec::new();

    if let Some(o) = parse_dump_field(&text, "PlayerControl", "CachedPlayerData;")
        .or_else(|| parse_dump_field(&text, "PlayerControl", "NetworkedPlayerInfo Data;"))
        .or_else(|| parse_dump_field(&text, "PlayerControl", "Data;"))
    {
        player_control.data = o;
        notes.push(format!("PlayerControl.Data = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "NetworkedPlayerInfo.PlayerOutfit", "PlayerName;")
        .or_else(|| parse_dump_field(&text, "NetworkedPlayerInfo", "PlayerName;"))
        .or_else(|| parse_dump_field(&text, "PlayerOutfit", "PlayerName;"))
    {
        info.player_name = o;
        notes.push(format!("NetworkedPlayerInfo.PlayerName = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "NetworkedPlayerInfo.PlayerOutfit", "ColorId;")
        .or_else(|| parse_dump_field(&text, "NetworkedPlayerInfo", "ColorId;"))
        .or_else(|| parse_dump_field(&text, "PlayerOutfit", "ColorId;"))
    {
        info.color_id = o;
        notes.push(format!("NetworkedPlayerInfo.ColorId = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "NetworkedPlayerInfo", "RoleType;") {
        info.role_type = o;
        notes.push(format!("NetworkedPlayerInfo.RoleType = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "NetworkedPlayerInfo", "Disconnected;") {
        info.disconnected = o;
        notes.push(format!("NetworkedPlayerInfo.Disconnected = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "NetworkedPlayerInfo", "IsDead;") {
        info.is_dead = o;
        notes.push(format!("NetworkedPlayerInfo.IsDead = 0x{o:X}"));
    }

    if let Some(o) = parse_dump_field(&text, "AmongUsClient", "GameState;") {
        client.game_state = o;
        notes.push(format!("AmongUsClient.GameState = 0x{o:X}"));
    } else if let Some(o) = parse_dump_field(&text, "InnerNetClient", "GameState;") {
        client.game_state = o;
        notes.push(format!("InnerNetClient.GameState = 0x{o:X}"));
    }

    Ok(notes)
}

fn parse_struct_field_offset(source: &str, struct_name: &str, field_name: &str) -> Option<u64> {
    let marker = format!("struct {struct_name}");
    let start = source.find(&marker)?;
    let body_start = source[start..].find('{')? + start + 1;
    let body_end = source[body_start..].find('}')? + body_start;
    let body = &source[body_start..body_end];

    for line in body.lines() {
        if line.contains(field_name) {
            if let Some(offset) = parse_trailing_hex_offset(line) {
                return Some(offset);
            }
        }
    }
    None
}

fn parse_dump_field(source: &str, class_name: &str, field_signature: &str) -> Option<u64> {
    let class_token = format!("class {class_name}");
    let lines: Vec<&str> = source.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let declaration = line.trim().trim_end_matches('\r').replace('\t', " ");
        let is_decl = declaration.contains(&class_token)
            && !declaration.contains(&format!("{class_token}."))
            && !declaration.contains(&format!("{class_token}<"));

        if !is_decl {
            continue;
        }

        let mut depth = 0usize;
        let mut found_opening_brace = false;

        for body_line in lines.iter().skip(idx) {
            let normalized = body_line.trim().trim_end_matches('\r');
            let mut line_depth = 0usize;
            for ch in normalized.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        found_opening_brace = true;
                        line_depth += 1;
                    }
                    '}' => {
                        if depth > 0 {
                            depth -= 1;
                            if depth == 0 {
                                return None;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !found_opening_brace {
                continue;
            }

            let normalized = normalized.replace('\t', " ");
            if normalized.contains(field_signature)
                || normalized.contains(&field_signature.replace(' ', ""))
            {
                if let Some(offset) = parse_trailing_hex_offset(&normalized) {
                    return Some(offset);
                }
            }

            if line_depth > 0 {
                continue;
            }
        }

        return None;
    }

    None
}

fn parse_script_address(value: &Value) -> Result<u64, ConfigError> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| ConfigError::MissingOffset(format!("invalid script address: {value}"))),
        Value::String(text) => parse_numeric_literal(text)
            .ok_or_else(|| ConfigError::MissingOffset(format!("invalid script address: {text}"))),
        _ => Err(ConfigError::MissingOffset(format!(
            "unsupported script address value: {value}"
        ))),
    }
}

fn parse_numeric_literal(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        let hex = trimmed.trim_start_matches("0x").trim_start_matches("0X");
        return u64::from_str_radix(hex, 16).ok();
    }
    trimmed.parse::<u64>().ok()
}

fn parse_trailing_hex_offset(line: &str) -> Option<u64> {
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find("0x") {
        let start = cursor + offset;
        let prefix = &line[start..start + 2];
        let body_start = start + 2;
        let remainder = &line[body_start..];
        let end = remainder.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(remainder.len());
        let hex = &remainder[..end];
        if let Ok(value) = u64::from_str_radix(hex, 16) {
            return Some(value);
        }
        cursor = start + 2;
        if prefix.eq_ignore_ascii_case("0x") {
            continue;
        }
    }
    None
}

pub fn resolve_dump_path(base: &Path, maybe_relative: &str) -> String {
    let path = Path::new(maybe_relative);
    if path.is_absolute() {
        return maybe_relative.to_string();
    }
    if path.exists() {
        return maybe_relative.to_string();
    }
    let beside = base.join(maybe_relative);
    if beside.exists() {
        return beside.to_string_lossy().into_owned();
    }
    maybe_relative.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_comment() {
        assert_eq!(
            parse_trailing_hex_offset("    foo; // 0x78"),
            Some(0x78)
        );
    }

    #[test]
    fn applies_script_json_with_string_addresses() {
        let mut pointers = StaticPointers {
            player_control_type_info: 0,
            among_us_client_type_info: 0,
            game_data_type_info: 0,
        };
        let text = r#"[
            {"Name": "PlayerControl_TypeInfo", "Address": "0x1234"},
            {"Name": "AmongUsClient_TypeInfo", "Address": "0x5678"},
            {"Name": "GameData_TypeInfo", "Address": "0x9ABC"}
        ]"#;
        let path = std::env::temp_dir().join("script.json");
        std::fs::write(&path, text).unwrap();

        let notes = apply_script_json(&mut pointers, path.to_str().unwrap()).unwrap();

        assert!(notes.iter().any(|note| note.contains("loaded PlayerControl_TypeInfo")));
        assert_eq!(pointers.player_control_type_info, 0x1234);
        assert_eq!(pointers.among_us_client_type_info, 0x5678);
        assert_eq!(pointers.game_data_type_info, 0x9ABC);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn applies_script_json_with_object_mapping() {
        let mut pointers = StaticPointers {
            player_control_type_info: 0,
            among_us_client_type_info: 0,
            game_data_type_info: 0,
        };
        let text = r#"{
            "PlayerControl_TypeInfo": "0x1234",
            "AmongUsClient_TypeInfo": "0x5678",
            "GameData_TypeInfo": "0x9ABC"
        }"#;
        let path = std::env::temp_dir().join("script-object.json");
        std::fs::write(&path, text).unwrap();

        let notes = apply_script_json(&mut pointers, path.to_str().unwrap()).unwrap();

        assert!(notes.iter().any(|note| note.contains("loaded PlayerControl_TypeInfo")));
        assert_eq!(pointers.player_control_type_info, 0x1234);
        assert_eq!(pointers.among_us_client_type_info, 0x5678);
        assert_eq!(pointers.game_data_type_info, 0x9ABC);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn applies_static_field_offsets_from_dump_cs() {
        let text = r#"
public class PlayerControl {
    public static List<PlayerControl> AllPlayerControls; // 0x4
}
public class AmongUsClient {
    public static AmongUsClient Instance; // 0x0
}
public class GameData {
    public static GameData Instance; // 0x0
}
"#;
        let path = std::env::temp_dir().join("dump-static-fields.cs");
        std::fs::write(&path, text).unwrap();

        let mut fields = StaticFields {
            player_control_all_player_controls: 0,
            among_us_client_instance: 0,
            game_data_instance: 0,
        };
        let notes = apply_static_field_offsets_from_dump_cs(&mut fields, path.to_str().unwrap())
            .unwrap();

        assert!(notes.iter().any(|note| note.contains("PlayerControl.AllPlayerControls")));
        assert_eq!(fields.player_control_all_player_controls, 0x4);
        assert_eq!(fields.among_us_client_instance, 0x0);
        assert_eq!(fields.game_data_instance, 0x0);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_among_us_client_game_state_offset_with_enum_declaration() {
        let text = r#"
public class AmongUsClient {
    public static AmongUsClient Instance; // 0x0
    public static GameStates GameState; // 0x5C
}
public enum GameStates {
    NotStarted = 0,
    Started = 1,
    Ended = 2,
}
"#;
        let path = std::env::temp_dir().join("dump-game-state.cs");
        std::fs::write(&path, text).unwrap();

        let mut client = AmongUsClientFields { game_state: 0 };
        let notes = apply_dump_cs(&mut PlayerControlFields { data: 0 }, &mut NetworkedPlayerInfoFields {
            player_name: 0,
            color_id: 0,
            role_type: 0,
            disconnected: 0,
            is_dead: 0,
        }, &mut client, path.to_str().unwrap())
            .unwrap();

        assert!(notes.iter().any(|note| note.contains("AmongUsClient.GameState")));
        assert_eq!(client.game_state, 0x5C);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn ignores_nested_class_declarations_when_parsing_dump_cs() {
        let text = r#"
public class PlayerControl.ColliderComparer {
    public static readonly PlayerControl.ColliderComparer Instance; // 0x0
}
public class PlayerControl : InnerNetObject {
    public static List<PlayerControl> AllPlayerControls; // 0x4
}
public class AmongUsClient : InnerNetClient {
    public static AmongUsClient Instance; // 0x0
}
"#;
        let path = std::env::temp_dir().join("dump-nested-classes.cs");
        std::fs::write(&path, text).unwrap();

        let mut fields = StaticFields {
            player_control_all_player_controls: 0,
            among_us_client_instance: 0,
            game_data_instance: 0,
        };
        let notes = apply_static_field_offsets_from_dump_cs(&mut fields, path.to_str().unwrap())
            .unwrap();

        assert!(notes.iter().any(|note| note.contains("PlayerControl.AllPlayerControls")));
        assert_eq!(fields.player_control_all_player_controls, 0x4);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parses_real_dump_cs_fields() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = Path::new(&manifest_dir).join("dump.cs");
        if !path.exists() {
            return;
        }
        let text = std::fs::read_to_string(&path).unwrap();

        assert!(parse_dump_field(&text, "PlayerControl", "AllPlayerControls").is_some());
        assert!(parse_dump_field(&text, "AmongUsClient", "Instance").is_some());
        assert!(parse_dump_field(&text, "GameData", "Instance").is_some());
    }
}
