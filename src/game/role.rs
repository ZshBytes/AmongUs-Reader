use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleType {
    Crewmate,
    Impostor,
    Scientist,
    Engineer,
    GuardianAngel,
    Shapeshifter,
    CrewmateGhost,
    ImpostorGhost,
    Phantom,
    Tracker,
    Noisemaker,
    Detective,
    Viper,
    Judge,
    Unknown(u16),
}

impl RoleType {
    pub fn from_id(id: u16, valid: &std::collections::HashSet<u16>) -> Option<Self> {
        if !valid.contains(&id) && id > 64 {
            return None;
        }

        Some(match id {
            0 => RoleType::Crewmate,
            1 => RoleType::Impostor,
            2 => RoleType::Scientist,
            3 => RoleType::Engineer,
            4 => RoleType::GuardianAngel,
            5 => RoleType::Shapeshifter,
            6 => RoleType::CrewmateGhost,
            7 => RoleType::ImpostorGhost,
            8 => RoleType::Noisemaker,
            9 => RoleType::Phantom,
            10 => RoleType::Tracker,
            11 => RoleType::Tracker,
            12 => RoleType::Detective,
            18 => RoleType::Viper,
            19 => RoleType::Judge,
            other => RoleType::Unknown(other),
        })
    }

    pub fn is_impostor_team(&self) -> bool {
        matches!(
            self,
            RoleType::Impostor
                | RoleType::Shapeshifter
                | RoleType::ImpostorGhost
                | RoleType::Phantom
                | RoleType::Viper
        )
    }
}

impl fmt::Display for RoleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            RoleType::Crewmate => "Crewmate",
            RoleType::Impostor => "Impostor",
            RoleType::Scientist => "Scientist",
            RoleType::Engineer => "Engineer",
            RoleType::GuardianAngel => "Guardian Angel",
            RoleType::Shapeshifter => "Shapeshifter",
            RoleType::CrewmateGhost => "Crewmate Ghost",
            RoleType::ImpostorGhost => "Impostor Ghost",
            RoleType::Phantom => "Phantom",
            RoleType::Tracker => "Tracker",
            RoleType::Noisemaker => "Noisemaker",
            RoleType::Detective => "Detective",
            RoleType::Viper => "Viper",
            RoleType::Judge => "Judge",
            RoleType::Unknown(id) => return write!(f, "Unknown({id})"),
        };
        f.write_str(label)
    }
}

pub fn color_name(color_id: i32) -> &'static str {
    match color_id {
        0 => "Red",
        1 => "Blue",
        2 => "Green",
        3 => "Pink",
        4 => "Orange",
        5 => "Yellow",
        6 => "Black",
        7 => "White",
        8 => "Purple",
        9 => "Brown",
        10 => "Cyan",
        11 => "Lime",
        12 => "Maroon",
        13 => "Rose",
        14 => "Banana",
        15 => "Gray",
        16 => "Tan",
        17 => "Coral",
        _ => "Unknown",
    }
}

pub fn color_rgb(color_id: i32) -> (u8, u8, u8) {
    match color_id {
        0 => (0xC5, 0x11, 0x11),  // Red (#C51111)
        1 => (0x13, 0x2E, 0xD1),  // Blue (#132ED1)
        2 => (0x11, 0x7F, 0x2D),  // Green (#117F2D)
        3 => (0xED, 0x54, 0xBA),  // Pink (#ED54BA)
        4 => (0xEF, 0x7D, 0x0E),  // Orange (#EF7D0E)
        5 => (0xF5, 0xF5, 0x38),  // Yellow (#F5F538)
        6 => (0x3F, 0x47, 0x4E),  // Black (#3F474E)
        7 => (0xD6, 0xE0, 0xF0),  // White (#D6E0F0)
        8 => (0x6B, 0x2F, 0xBB),  // Purple (#6B2FBB)
        9 => (0x71, 0x49, 0x1D),  // Brown (#71491D)
        10 => (0x38, 0xFE, 0xDD), // Cyan (#38FEDD)
        11 => (0x51, 0xF4, 0x37), // Lime (#51F437)
        12 => (0x6C, 0x2B, 0x3C), // Maroon (#6C2B3C)
        13 => (0xEC, 0xC0, 0xD3), // Rose (#ECC0D3)
        14 => (0xFF, 0xFF, 0xBE), // Banana (#FFFFBE)
        15 => (0x70, 0x84, 0x96), // Gray (#708496)
        16 => (0x92, 0x87, 0x76), // Tan (#928776)
        17 => (0xEC, 0x76, 0x78), // Coral (#EC7678)
        _ => (0x70, 0x84, 0x96),
    }
}
