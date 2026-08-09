use crate::game::role::RoleType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSnapshot {
    pub name: String,
    pub color_id: i32,
    pub role: RoleType,
    pub is_dead: bool,
    pub disconnected: bool,
}
