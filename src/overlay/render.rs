// Author: @szuwer
// Overlay rendering and UI components

use egui::{Color32, FullOutput, RawInput, RichText, ScrollArea, Ui};

use crate::game::role::{color_name, color_rgb};
use crate::game::state::OverlayStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    None,
    ToggleStreamProof,
}

pub fn draw_overlay(
    ctx: &egui::Context,
    raw_input: RawInput,
    state: &OverlayStatus,
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
                draw_header(ui, state, &mut action);
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

fn draw_header(ui: &mut Ui, state: &OverlayStatus, action: &mut OverlayAction) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Among Us Live Overlay (@szuwer)")
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
    let message = if state.status_message.is_empty() {
        "No active match — player cache flushed.".to_string()
    } else {
        state.status_message.clone()
    };
    ui.label(RichText::new(message).italics().color(Color32::GRAY));
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
