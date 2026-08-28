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
        if !valid.contains(&id) {
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
        0 => (0xD7, 0x1E, 0x22),
        1 => (0x1D, 0x3C, 0xE9),
        2 => (0x11, 0x7F, 0x2D),
        3 => (0xED, 0x54, 0xBA),
        4 => (0xEF, 0x7D, 0x0E),
        5 => (0xF5, 0xF5, 0x57),
        6 => (0x3F, 0x47, 0x4F),
        7 => (0xE8, 0xE8, 0xE8),
        8 => (0x6B, 0x2F, 0xBC),
        9 => (0x71, 0x49, 0x1E),
        10 => (0x38, 0xFD, 0xC1),
        11 => (0x53, 0xF5, 0x57),
        12 => (0x5F, 0x15, 0x1E),
        13 => (0xEC, 0x75, 0x7A),
        14 => (0xF5, 0xE6, 0x85),
        15 => (0x94, 0x97, 0x97),
        16 => (0x92, 0x7A, 0x4C),
        17 => (0xF9, 0x6D, 0x5C),
        _ => (0xAA, 0xAA, 0xAA),
    }
}
