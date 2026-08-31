use egui::Color32;
use serde::{Deserialize, Serialize};

use crate::game::role::RoleType;

pub const THEME_FILE: &str = "theme.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    pub background: [u8; 4],
    pub canvas: [u8; 4],
    pub border: [u8; 4],
    pub accent: [u8; 4],
    pub header_text: [u8; 4],
    pub local_player: [u8; 4],
    pub impostor_line: [u8; 4],
    #[serde(default = "default_crewmate")]
    pub crewmate: [u8; 4],
    #[serde(default = "default_impostor")]
    pub impostor: [u8; 4],
    #[serde(default = "default_scientist")]
    pub scientist: [u8; 4],
    #[serde(default = "default_engineer")]
    pub engineer: [u8; 4],
    #[serde(default = "default_guardian_angel")]
    pub guardian_angel: [u8; 4],
    #[serde(default = "default_shapeshifter")]
    pub shapeshifter: [u8; 4],
    #[serde(default = "default_crewmate_ghost")]
    pub crewmate_ghost: [u8; 4],
    #[serde(default = "default_impostor_ghost")]
    pub impostor_ghost: [u8; 4],
    #[serde(default = "default_phantom")]
    pub phantom: [u8; 4],
    #[serde(default = "default_tracker")]
    pub tracker: [u8; 4],
    #[serde(default = "default_noisemaker")]
    pub noisemaker: [u8; 4],
    #[serde(default = "default_detective")]
    pub detective: [u8; 4],
    #[serde(default = "default_viper")]
    pub viper: [u8; 4],
    #[serde(default = "default_judge")]
    pub judge: [u8; 4],
}

fn default_crewmate() -> [u8; 4] {
    [140, 200, 240, 255]
}
fn default_impostor() -> [u8; 4] {
    [255, 50, 50, 220]
}
fn default_scientist() -> [u8; 4] {
    [80, 200, 240, 255]
}
fn default_engineer() -> [u8; 4] {
    [240, 160, 40, 255]
}
fn default_guardian_angel() -> [u8; 4] {
    [240, 240, 240, 255]
}
fn default_shapeshifter() -> [u8; 4] {
    [255, 60, 80, 255]
}
fn default_crewmate_ghost() -> [u8; 4] {
    [160, 190, 210, 200]
}
fn default_impostor_ghost() -> [u8; 4] {
    [255, 110, 110, 200]
}
fn default_phantom() -> [u8; 4] {
    [160, 80, 240, 255]
}
fn default_tracker() -> [u8; 4] {
    [100, 240, 140, 255]
}
fn default_noisemaker() -> [u8; 4] {
    [255, 120, 220, 255]
}
fn default_detective() -> [u8; 4] {
    [80, 160, 255, 255]
}
fn default_viper() -> [u8; 4] {
    [0, 230, 120, 255]
}
fn default_judge() -> [u8; 4] {
    [255, 215, 0, 255]
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::dark_modern()
    }
}

impl ThemeConfig {
    pub fn dark_modern() -> Self {
        Self {
            name: "dark".into(),
            background: [10, 12, 18, 240],
            canvas: [6, 8, 14, 240],
            border: [40, 70, 110, 100],
            accent: [60, 140, 230, 255],
            header_text: [220, 230, 255, 255],
            local_player: [60, 220, 255, 255],
            impostor_line: [255, 50, 50, 220],
            crewmate: default_crewmate(),
            impostor: default_impostor(),
            scientist: default_scientist(),
            engineer: default_engineer(),
            guardian_angel: default_guardian_angel(),
            shapeshifter: default_shapeshifter(),
            crewmate_ghost: default_crewmate_ghost(),
            impostor_ghost: default_impostor_ghost(),
            phantom: default_phantom(),
            tracker: default_tracker(),
            noisemaker: default_noisemaker(),
            detective: default_detective(),
            viper: default_viper(),
            judge: default_judge(),
        }
    }

    pub fn midnight_purple() -> Self {
        Self {
            name: "purple".into(),
            background: [18, 10, 28, 240],
            canvas: [12, 6, 20, 240],
            border: [130, 60, 200, 120],
            accent: [170, 80, 255, 255],
            header_text: [240, 210, 255, 255],
            local_player: [210, 120, 255, 255],
            impostor_line: [255, 40, 120, 220],
            crewmate: [180, 170, 255, 255],
            impostor: [255, 50, 140, 255],
            scientist: [120, 210, 255, 255],
            engineer: [255, 180, 60, 255],
            guardian_angel: [255, 255, 255, 255],
            shapeshifter: [255, 50, 130, 255],
            crewmate_ghost: [190, 170, 220, 200],
            impostor_ghost: [255, 100, 160, 200],
            phantom: [200, 100, 255, 255],
            tracker: [130, 255, 160, 255],
            noisemaker: [255, 140, 230, 255],
            detective: [140, 160, 255, 255],
            viper: [40, 240, 180, 255],
            judge: [255, 225, 60, 255],
        }
    }

    pub fn cyberpunk_neon() -> Self {
        Self {
            name: "neon".into(),
            background: [12, 14, 20, 245],
            canvas: [8, 10, 14, 245],
            border: [255, 220, 0, 130],
            accent: [255, 220, 0, 255],
            header_text: [0, 240, 255, 255],
            local_player: [0, 240, 255, 255],
            impostor_line: [255, 30, 90, 230],
            crewmate: [0, 220, 255, 255],
            impostor: [255, 30, 90, 255],
            scientist: [0, 240, 255, 255],
            engineer: [255, 220, 0, 255],
            guardian_angel: [240, 240, 255, 255],
            shapeshifter: [255, 30, 90, 255],
            crewmate_ghost: [120, 240, 255, 200],
            impostor_ghost: [255, 60, 120, 200],
            phantom: [180, 50, 255, 255],
            tracker: [50, 255, 120, 255],
            noisemaker: [255, 0, 200, 255],
            detective: [0, 180, 255, 255],
            viper: [0, 255, 140, 255],
            judge: [255, 240, 0, 255],
        }
    }

    pub fn matrix_emerald() -> Self {
        Self {
            name: "green".into(),
            background: [8, 18, 12, 240],
            canvas: [4, 12, 8, 240],
            border: [40, 180, 80, 120],
            accent: [50, 220, 100, 255],
            header_text: [120, 255, 160, 255],
            local_player: [80, 255, 140, 255],
            impostor_line: [255, 70, 70, 220],
            crewmate: [100, 240, 160, 255],
            impostor: [255, 60, 60, 255],
            scientist: [70, 230, 200, 255],
            engineer: [220, 200, 50, 255],
            guardian_angel: [230, 255, 240, 255],
            shapeshifter: [255, 60, 60, 255],
            crewmate_ghost: [140, 220, 180, 200],
            impostor_ghost: [255, 100, 100, 200],
            phantom: [150, 80, 230, 255],
            tracker: [60, 255, 130, 255],
            noisemaker: [230, 120, 200, 255],
            detective: [80, 220, 200, 255],
            viper: [0, 255, 100, 255],
            judge: [230, 240, 60, 255],
        }
    }

    pub fn crimson_blood() -> Self {
        Self {
            name: "crimson".into(),
            background: [20, 10, 12, 240],
            canvas: [14, 6, 8, 240],
            border: [190, 40, 50, 120],
            accent: [240, 60, 70, 255],
            header_text: [255, 200, 205, 255],
            local_player: [255, 100, 110, 255],
            impostor_line: [255, 30, 30, 230],
            crewmate: [220, 180, 180, 255],
            impostor: [255, 30, 30, 255],
            scientist: [100, 200, 255, 255],
            engineer: [255, 140, 50, 255],
            guardian_angel: [255, 240, 240, 255],
            shapeshifter: [255, 30, 40, 255],
            crewmate_ghost: [200, 160, 160, 200],
            impostor_ghost: [255, 80, 80, 200],
            phantom: [190, 40, 180, 255],
            tracker: [110, 240, 130, 255],
            noisemaker: [255, 100, 180, 255],
            detective: [120, 150, 240, 255],
            viper: [220, 40, 80, 255],
            judge: [255, 180, 40, 255],
        }
    }

    pub fn load() -> Self {
        if let Ok(contents) = std::fs::read_to_string(THEME_FILE) {
            if let Ok(theme) = toml::from_str::<ThemeConfig>(&contents) {
                return theme;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = std::fs::write(THEME_FILE, serialized);
        }
    }

    pub fn bg_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.background[0],
            self.background[1],
            self.background[2],
            self.background[3],
        )
    }

    pub fn canvas_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.canvas[0],
            self.canvas[1],
            self.canvas[2],
            self.canvas[3],
        )
    }

    pub fn border_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.border[0],
            self.border[1],
            self.border[2],
            self.border[3],
        )
    }

    pub fn accent_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.accent[0],
            self.accent[1],
            self.accent[2],
            self.accent[3],
        )
    }

    pub fn header_text_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.header_text[0],
            self.header_text[1],
            self.header_text[2],
            self.header_text[3],
        )
    }

    pub fn local_player_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.local_player[0],
            self.local_player[1],
            self.local_player[2],
            self.local_player[3],
        )
    }

    pub fn impostor_line_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.impostor_line[0],
            self.impostor_line[1],
            self.impostor_line[2],
            self.impostor_line[3],
        )
    }

    pub fn role_color32(&self, role: &RoleType) -> Color32 {
        match role {
            RoleType::Crewmate => Color32::from_rgba_unmultiplied(
                self.crewmate[0],
                self.crewmate[1],
                self.crewmate[2],
                self.crewmate[3],
            ),
            RoleType::Impostor => Color32::from_rgba_unmultiplied(
                self.impostor[0],
                self.impostor[1],
                self.impostor[2],
                self.impostor[3],
            ),
            RoleType::Scientist => Color32::from_rgba_unmultiplied(
                self.scientist[0],
                self.scientist[1],
                self.scientist[2],
                self.scientist[3],
            ),
            RoleType::Engineer => Color32::from_rgba_unmultiplied(
                self.engineer[0],
                self.engineer[1],
                self.engineer[2],
                self.engineer[3],
            ),
            RoleType::GuardianAngel => Color32::from_rgba_unmultiplied(
                self.guardian_angel[0],
                self.guardian_angel[1],
                self.guardian_angel[2],
                self.guardian_angel[3],
            ),
            RoleType::Shapeshifter => Color32::from_rgba_unmultiplied(
                self.shapeshifter[0],
                self.shapeshifter[1],
                self.shapeshifter[2],
                self.shapeshifter[3],
            ),
            RoleType::CrewmateGhost => Color32::from_rgba_unmultiplied(
                self.crewmate_ghost[0],
                self.crewmate_ghost[1],
                self.crewmate_ghost[2],
                self.crewmate_ghost[3],
            ),
            RoleType::ImpostorGhost => Color32::from_rgba_unmultiplied(
                self.impostor_ghost[0],
                self.impostor_ghost[1],
                self.impostor_ghost[2],
                self.impostor_ghost[3],
            ),
            RoleType::Phantom => Color32::from_rgba_unmultiplied(
                self.phantom[0],
                self.phantom[1],
                self.phantom[2],
                self.phantom[3],
            ),
            RoleType::Tracker => Color32::from_rgba_unmultiplied(
                self.tracker[0],
                self.tracker[1],
                self.tracker[2],
                self.tracker[3],
            ),
            RoleType::Noisemaker => Color32::from_rgba_unmultiplied(
                self.noisemaker[0],
                self.noisemaker[1],
                self.noisemaker[2],
                self.noisemaker[3],
            ),
            RoleType::Detective => Color32::from_rgba_unmultiplied(
                self.detective[0],
                self.detective[1],
                self.detective[2],
                self.detective[3],
            ),
            RoleType::Viper => Color32::from_rgba_unmultiplied(
                self.viper[0],
                self.viper[1],
                self.viper[2],
                self.viper[3],
            ),
            RoleType::Judge => Color32::from_rgba_unmultiplied(
                self.judge[0],
                self.judge[1],
                self.judge[2],
                self.judge[3],
            ),
            RoleType::Unknown(_) => Color32::from_rgb(180, 180, 180),
        }
    }
}
