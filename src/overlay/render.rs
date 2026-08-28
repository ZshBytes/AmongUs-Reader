// Author: @szuwer
// Overlay rendering and UI components

use egui::{Color32, FullOutput, RawInput, RichText, ScrollArea, Ui};

use crate::game::role::{color_name, color_rgb};
use crate::game::state::OverlayStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayAction {
    None,
    ToggleStreamProof,
    ChangeToggleKey(String),
}

pub fn draw_overlay(
    ctx: &egui::Context,
    raw_input: RawInput,
    state: &OverlayStatus,
    toggle_key: &str,
    is_editing_key: &mut bool,
    key_buffer: &mut String,
) -> (FullOutput, OverlayAction) {
    ctx.request_repaint();
    let mut action = OverlayAction::None;
    let output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(10, 10, 16, 210))
                    .inner_margin(12.0)
                    .rounding(8.0),
            )
            .show(ctx, |ui| {
                draw_header(ui, state, toggle_key, is_editing_key, key_buffer, &mut action);
                ui.separator();
                if !state.players.is_empty() {
                    draw_player_list(ui, state);
                } else {
                    draw_idle(ui, state);
                }
            });
    });
    (output, action)
}

fn draw_header(
    ui: &mut Ui,
    state: &OverlayStatus,
    toggle_key: &str,
    is_editing_key: &mut bool,
    key_buffer: &mut String,
    action: &mut OverlayAction,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Among Us External cheat (made with <3 by szuwer)")
                .strong()
                .size(15.0),
        );

        ui.add_space(8.0);

        let (sp_color, sp_text) = if state.stream_proof {
            (Color32::from_rgb(80, 220, 120), "Stream-Proof: ON")
        } else {
            (Color32::from_rgb(230, 110, 90), "Stream-Proof: OFF")
        };

        if ui.button(RichText::new(sp_text).small().color(sp_color)).clicked() {
            *action = OverlayAction::ToggleStreamProof;
        }

        ui.add_space(6.0);

        if *is_editing_key {
            let response = ui.add(
                egui::TextEdit::singleline(key_buffer)
                    .desired_width(70.0)
                    .hint_text("Key...")
            );
            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let trimmed = key_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    *action = OverlayAction::ChangeToggleKey(trimmed);
                }
                *is_editing_key = false;
            }
            if ui.button("OK").clicked() {
                let trimmed = key_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    *action = OverlayAction::ChangeToggleKey(trimmed);
                }
                *is_editing_key = false;
            }
        } else {
            let btn_text = format!("[{toggle_key}: Hide/Show]");
            let btn = ui.button(
                RichText::new(btn_text)
                    .small()
                    .color(Color32::from_rgb(170, 190, 230)),
            );
            if btn.clicked() {
                *is_editing_key = true;
                *key_buffer = toggle_key.to_string();
            }
            btn.on_hover_text("Click to customize the toggle key (e.g. Delete, F1-F12, Home, End)");
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (dot_color, label) = match state.game_state {
                1 => (Color32::from_rgb(80, 180, 240), "LOBBY"),
                2 => (Color32::from_rgb(80, 220, 120), "IN MATCH"),
                3 => (Color32::from_rgb(240, 190, 60), "ENDED"),
                _ => if state.connected {
                    (Color32::from_rgb(240, 190, 60), "CONNECTED")
                } else {
                    (Color32::from_rgb(200, 80, 80), "OFFLINE")
                },
            };
            ui.label(RichText::new(label).small().color(dot_color));
        });
    });
}

fn draw_idle(ui: &mut Ui, state: &OverlayStatus) {
    let message = if !state.connected {
        if state.status_message.is_empty() {
            "Waiting for Among Us to launch...".to_string()
        } else {
            state.status_message.clone()
        }
    } else if !state.status_message.is_empty() {
        format!("Connected to Among Us — {}", state.status_message)
    } else {
        "Connected to Among Us — Waiting for players (join a lobby or match)".to_string()
    };
    ui.label(RichText::new(message).italics().color(Color32::from_rgb(220, 200, 100)));
}

fn draw_player_list(ui: &mut Ui, state: &OverlayStatus) {
    let num_players = state.players.len();
    ui.label(
        RichText::new(format!("Players ({num_players})"))
            .strong(),
    );
    ui.add_space(4.0);

    let avail_w = ui.available_width();
    let num_cols = if avail_w >= 640.0 {
        3
    } else if avail_w >= 380.0 {
        2
    } else {
        1
    };

    let max_h = ui.available_height().max(200.0);

    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        egui::Grid::new("player_grid")
            .num_columns(num_cols)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (idx, player) in state.players.iter().enumerate() {
                    ui.group(|ui| {
                        let item_w = ((avail_w - (num_cols as f32 - 1.0) * 12.0) / num_cols as f32).max(170.0);
                        ui.set_min_width(item_w);
                        let (r, g, b) = color_rgb(player.color_id);
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                Color32::from_rgb(r, g, b),
                                RichText::new("#").size(16.0).strong(),
                            );
                            ui.vertical(|ui| {
                                let name_text = if player.is_dead {
                                    RichText::new(format!("{} (DEAD)", player.name))
                                        .strong()
                                        .strikethrough()
                                        .color(Color32::from_rgb(230, 80, 80))
                                } else {
                                    RichText::new(&player.name).strong()
                                };
                                ui.label(name_text);

                                ui.label(format!(
                                    "{} | {}",
                                    color_name(player.color_id),
                                    player.role
                                ));

                                let flags = [
                                    if player.is_dead { Some("DEAD") } else { None },
                                    if player.role.is_impostor_team() {
                                        Some("IMP TEAM")
                                    } else {
                                        None
                                    },
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(" | ");
                                if !flags.is_empty() {
                                    ui.label(RichText::new(flags).small().color(Color32::LIGHT_RED));
                                }
                            });
                        });
                    });

                    if (idx + 1) % num_cols == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}
