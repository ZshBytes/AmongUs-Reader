use crate::game::role::RoleType;

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSnapshot {
    pub name: String,
    pub color_id: i32,
    pub role: RoleType,
    pub is_dead: bool,
    pub disconnected: bool,
    pub position: (f32, f32),
    pub is_local: bool,
    pub distance: f32,
    pub player_id: u8,
    pub friend_code: String,
    pub in_vent: bool,
    pub shapeshifting: bool,
    pub shapeshift_target: Option<u8>,
    pub voted_for: Option<i16>,
    pub was_ejected: bool,
    pub is_vanished: bool,
}
