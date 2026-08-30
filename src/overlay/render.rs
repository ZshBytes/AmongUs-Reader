use std::collections::HashMap;
use std::time::Instant;

use egui::{Align2, Color32, FontId, FullOutput, Pos2, RawInput, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};

use crate::game::role::{color_name, color_rgb};
use crate::game::state::OverlayStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayAction {
    None,
    Close,
    DragWindow,
    ToggleStreamProof,
    ChangeToggleKey(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOrigin {
    LocalPlayer,
    BottomCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerFilter {
    All,
    ImpostorsOnly,
    CrewmatesOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayTab {
    Tracers,
    Players,
    CheatSheet,
}

#[derive(Debug, Clone)]
pub struct RadarState {
    pub scale: f32,
    pub show_tracers: bool,
    pub filter: PlayerFilter,
    pub origin: LineOrigin,
    pub selected_tab: OverlayTab,
    pub smoothed_positions: HashMap<u8, (f32, f32)>,
    pub last_frame: Option<Instant>,
}

impl Default for RadarState {
    fn default() -> Self {
        Self {
            scale: 90.0,
            show_tracers: true,
            filter: PlayerFilter::All,
            origin: LineOrigin::LocalPlayer,
            selected_tab: OverlayTab::Tracers,
            smoothed_positions: HashMap::new(),
            last_frame: None,
        }
    }
}

pub fn draw_overlay(
    ctx: &egui::Context,
    raw_input: RawInput,
    state: &OverlayStatus,
    toggle_key: &str,
    is_editing_key: &mut bool,
    key_buffer: &mut String,
    radar: &mut RadarState,
) -> (FullOutput, OverlayAction) {
    ctx.request_repaint();
    let mut action = OverlayAction::None;
    let output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_unmultiplied(10, 12, 18, 240))
                    .inner_margin(10.0)
                    .rounding(8.0),
            )
            .show(ctx, |ui| {
                draw_header(ui, state, toggle_key, is_editing_key, key_buffer, &mut action);
                ui.separator();

                draw_tab_bar(ui, radar);
                ui.add_space(4.0);

                match radar.selected_tab {
                    OverlayTab::Tracers => {
                        if !state.players.is_empty() {
                            draw_tracer_controls(ui, radar);
                            ui.add_space(4.0);
                            draw_tracers_canvas(ui, state, radar);
                        } else {
                            draw_idle(ui, state);
                        }
                    }
                    OverlayTab::Players => {
                        if !state.players.is_empty() {
                            draw_player_list(ui, state);
                        } else {
                            draw_idle(ui, state);
                        }
                    }
                    OverlayTab::CheatSheet => {
                        draw_cheat_sheet(ui);
                    }
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
    let top_rect = ui.available_rect_before_wrap();
    let drag_rect = egui::Rect::from_min_size(top_rect.min, Vec2::new(top_rect.width() - 60.0, 32.0));
    let drag_resp = ui.interact(drag_rect, ui.id().with("window_drag_bar"), Sense::drag());
    if drag_resp.drag_started() || (drag_resp.dragged() && ui.input(|i| i.pointer.primary_down())) {
        *action = OverlayAction::DragWindow;
    }

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Among Us External (by szuwer)")
                .strong()
                .size(13.5)
                .color(Color32::from_rgb(220, 230, 255)),
        );

        ui.add_space(4.0);

        let (sp_color, sp_text) = if state.stream_proof {
            (Color32::from_rgb(80, 220, 120), "Stream-Proof: ON")
        } else {
            (Color32::from_rgb(230, 110, 90), "Stream-Proof: OFF")
        };

        if ui.button(RichText::new(sp_text).small().color(sp_color)).clicked() {
            *action = OverlayAction::ToggleStreamProof;
        }

        ui.add_space(2.0);

        if *is_editing_key {
            let response = ui.add(
                egui::TextEdit::singleline(key_buffer)
                    .desired_width(55.0)
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
            btn.on_hover_text("Click to customize toggle key (e.g. Delete, F1-F12, Insert, Home)");
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close_btn = ui.add(
                egui::Button::new(RichText::new(" X ").strong().size(11.0).color(Color32::WHITE))
                    .fill(Color32::from_rgb(180, 45, 45)),
            );
            if close_btn.clicked() {
                *action = OverlayAction::Close;
            }
            close_btn.on_hover_text("Close Overlay");

            ui.add_space(4.0);

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

fn draw_tab_bar(ui: &mut Ui, radar: &mut RadarState) {
    ui.horizontal(|ui| {
        let btn_tracers = ui.selectable_label(radar.selected_tab == OverlayTab::Tracers, "Line Tracers / ESP");
        if btn_tracers.clicked() {
            radar.selected_tab = OverlayTab::Tracers;
        }

        let btn_players = ui.selectable_label(radar.selected_tab == OverlayTab::Players, "Players List");
        if btn_players.clicked() {
            radar.selected_tab = OverlayTab::Players;
        }

        let btn_cheat = ui.selectable_label(radar.selected_tab == OverlayTab::CheatSheet, "Cheat Sheet");
        if btn_cheat.clicked() {
            radar.selected_tab = OverlayTab::CheatSheet;
        }
    });
}

fn draw_tracer_controls(ui: &mut Ui, radar: &mut RadarState) {
    ui.horizontal(|ui| {
        let tracer_color = if radar.show_tracers {
            Color32::from_rgb(80, 220, 120)
        } else {
            Color32::from_rgb(180, 180, 180)
        };
        if ui.button(RichText::new(if radar.show_tracers { "Tracers: ON" } else { "Tracers: OFF" }).small().color(tracer_color)).clicked() {
            radar.show_tracers = !radar.show_tracers;
        }

        ui.add_space(3.0);

        let (filter_text, filter_color) = match radar.filter {
            PlayerFilter::All => ("Filter: All Players", Color32::from_rgb(170, 200, 240)),
            PlayerFilter::ImpostorsOnly => ("Filter: Impostors Only", Color32::from_rgb(255, 90, 90)),
            PlayerFilter::CrewmatesOnly => ("Filter: Crewmates Only", Color32::from_rgb(90, 220, 150)),
        };
        if ui.button(RichText::new(filter_text).small().color(filter_color)).clicked() {
            radar.filter = match radar.filter {
                PlayerFilter::All => PlayerFilter::ImpostorsOnly,
                PlayerFilter::ImpostorsOnly => PlayerFilter::CrewmatesOnly,
                PlayerFilter::CrewmatesOnly => PlayerFilter::All,
            };
        }

        ui.add_space(3.0);

        let origin_text = match radar.origin {
            LineOrigin::LocalPlayer => "Origin: Local Player",
            LineOrigin::BottomCenter => "Origin: Bottom Center",
        };
        if ui.button(RichText::new(origin_text).small().color(Color32::from_rgb(140, 190, 240))).clicked() {
            radar.origin = match radar.origin {
                LineOrigin::LocalPlayer => LineOrigin::BottomCenter,
                LineOrigin::BottomCenter => LineOrigin::LocalPlayer,
            };
        }

        ui.add_space(3.0);

        if ui.button("-").on_hover_text("Decrease Scale / FOV").clicked() {
            radar.scale = (radar.scale - 10.0).max(20.0);
        }
        ui.label(RichText::new(format!("{:.0}px", radar.scale)).small());
        if ui.button("+").on_hover_text("Increase Scale / FOV").clicked() {
            radar.scale = (radar.scale + 10.0).min(300.0);
        }
    });
}

fn draw_tracers_canvas(ui: &mut Ui, state: &OverlayStatus, radar: &mut RadarState) {
    let now = Instant::now();
    let dt = if let Some(last) = radar.last_frame {
        now.duration_since(last).as_secs_f32().min(0.05)
    } else {
        0.016
    };
    radar.last_frame = Some(now);

    let lerp_factor = 1.0 - (-14.0 * dt).exp();

    for player in &state.players {
        let (tx, ty) = player.position;
        if tx == 0.0 && ty == 0.0 && radar.smoothed_positions.contains_key(&player.player_id) {
            continue;
        }

        let entry = radar.smoothed_positions.entry(player.player_id).or_insert((tx, ty));
        let jump_dist = ((tx - entry.0).powi(2) + (ty - entry.1).powi(2)).sqrt();
        if jump_dist > 25.0 {
            *entry = (tx, ty);
        } else {
            entry.0 += (tx - entry.0) * lerp_factor;
            entry.1 += (ty - entry.1) * lerp_factor;
        }
    }

    let avail_w = ui.available_width();
    let avail_h = ui.available_height().max(260.0);
    let size = Vec2::new(avail_w, avail_h);
    let (rect, _response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(6, 8, 14, 240));
    painter.rect_stroke(rect, 6.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(40, 70, 110, 100)));

    let center = rect.center();
    let origin = match radar.origin {
        LineOrigin::LocalPlayer => center,
        LineOrigin::BottomCenter => Pos2::new(center.x, rect.bottom() - 10.0),
    };

    painter.circle_filled(origin, 5.0, Color32::from_rgb(60, 220, 255));
    painter.circle_stroke(origin, 7.0, Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(60, 220, 255, 140)));
    painter.text(
        Pos2::new(origin.x, origin.y + 8.0),
        Align2::CENTER_TOP,
        "[YOU]",
        FontId::proportional(10.0),
        Color32::from_rgb(60, 220, 255),
    );

    let local_player = state.players.iter().find(|p| p.is_local);
    let local_pos = if let Some(lp) = local_player {
        radar.smoothed_positions.get(&lp.player_id).copied().unwrap_or(lp.position)
    } else {
        (0.0, 0.0)
    };

    for player in &state.players {
        if player.is_local {
            continue;
        }

        let is_imp = player.role.is_impostor_team();
        match radar.filter {
            PlayerFilter::ImpostorsOnly if !is_imp => continue,
            PlayerFilter::CrewmatesOnly if is_imp => continue,
            _ => {}
        }

        let smoothed_pos = radar.smoothed_positions.get(&player.player_id).copied().unwrap_or(player.position);
        let dx = smoothed_pos.0 - local_pos.0;
        let dy = smoothed_pos.1 - local_pos.1;
        let distance = (dx * dx + dy * dy).sqrt();

        let target_x = center.x + (dx * (radar.scale * 0.5));
        let target_y = match radar.origin {
            LineOrigin::LocalPlayer => center.y - (dy * (radar.scale * 0.5)),
            LineOrigin::BottomCenter => origin.y - 20.0 - (dy.max(0.0) * (radar.scale * 0.5) + (dx.abs() * 0.2 * (radar.scale * 0.5))),
        };

        let pad = 24.0;
        let clamped_x = target_x.clamp(rect.left() + pad, rect.right() - pad);
        let clamped_y = target_y.clamp(rect.top() + pad, rect.bottom() - pad);
        let target_pt = Pos2::new(clamped_x, clamped_y);

        let (r, g, b) = color_rgb(player.color_id);
        let player_col = Color32::from_rgb(r, g, b);

        if radar.show_tracers {
            let line_color = if is_imp {
                Color32::from_rgba_unmultiplied(255, 50, 50, 220)
            } else if player.is_dead {
                Color32::from_rgba_unmultiplied(180, 80, 80, 140)
            } else {
                Color32::from_rgba_unmultiplied(r, g, b, 180)
            };
            painter.line_segment([origin, target_pt], Stroke::new(1.6_f32, line_color));
        }

        painter.circle_filled(target_pt, 5.0, player_col);
        let outline_col = if is_imp {
            Color32::from_rgb(255, 40, 40)
        } else {
            Color32::WHITE
        };
        painter.circle_stroke(target_pt, 5.0, Stroke::new(1.2_f32, outline_col));

        let label_text = if player.is_dead {
            format!("{} (DEAD) - {:.1}m", player.name, distance)
        } else {
            format!("{} [{}] - {:.1}m", player.name, player.role, distance)
        };

        let text_color = if is_imp {
            Color32::from_rgb(255, 90, 90)
        } else if player.is_dead {
            Color32::from_rgb(220, 120, 120)
        } else {
            Color32::from_rgb(230, 230, 230)
        };

        painter.text(
            Pos2::new(target_pt.x, target_pt.y - 12.0),
            Align2::CENTER_BOTTOM,
            label_text,
            FontId::proportional(11.0),
            text_color,
        );
    }
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
    ui.label(RichText::new(format!("Players ({num_players})")).strong());
    ui.add_space(4.0);

    let avail_w = ui.available_width();
    let num_cols = if avail_w >= 640.0 {
        3
    } else if avail_w >= 360.0 {
        2
    } else {
        1
    };

    let max_h = ui.available_height().max(180.0);

    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        egui::Grid::new("player_grid")
            .num_columns(num_cols)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                for (idx, player) in state.players.iter().enumerate() {
                    ui.group(|ui| {
                        let item_w = ((avail_w - (num_cols as f32 - 1.0) * 12.0) / num_cols as f32).max(160.0);
                        ui.set_min_width(item_w);
                        let (r, g, b) = color_rgb(player.color_id);
                        let player_color = Color32::from_rgb(r, g, b);

                        ui.horizontal(|ui| {
                            let (sq_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(sq_rect, 3.0, player_color);
                            ui.painter().rect_stroke(sq_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 80)));

                            ui.vertical(|ui| {
                                let name_text = if player.is_dead {
                                    RichText::new(format!("{} (DEAD)", player.name))
                                        .strong()
                                        .strikethrough()
                                        .color(Color32::from_rgb(230, 80, 80))
                                } else if player.is_local {
                                    RichText::new(format!("{} [YOU]", player.name))
                                        .strong()
                                        .color(Color32::from_rgb(60, 220, 255))
                                } else {
                                    RichText::new(&player.name).strong()
                                };
                                ui.label(name_text);

                                ui.label(format!(
                                    "{} | {}",
                                    color_name(player.color_id),
                                    player.role
                                ));

                                if player.is_local {
                                    ui.label(
                                        RichText::new(format!("Pos: ({:.1}, {:.1})", player.position.0, player.position.1))
                                            .small()
                                            .color(Color32::from_rgb(130, 210, 240)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("Pos: ({:.1}, {:.1}) | Dist: {:.1}m", player.position.0, player.position.1, player.distance))
                                            .small()
                                            .color(Color32::from_rgb(180, 190, 210)),
                                    );
                                }

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

fn draw_cheat_sheet(ui: &mut Ui) {
    ui.label(RichText::new("Task Timing Cheat Sheet (Faking Guide)").strong().size(13.0));
    ui.add_space(4.0);

    let max_h = ui.available_height().max(200.0);
    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        // Section 1: Short Tasks
        ui.group(|ui| {
            ui.label(
                RichText::new("Instant / Very Short Tasks (1 to 3 seconds)")
                    .strong()
                    .color(Color32::from_rgb(100, 220, 150)),
            );
            ui.add_space(2.0);

            draw_task_item(ui, "Swipe Card / Insert Keys", "2 to 3 seconds", Some("allow extra time if you want to fake a failed first attempt"));
            draw_task_item(ui, "Clean Vent / Chart Course", "2 to 3 seconds", None);
            draw_task_item(ui, "Divert Power", "1 to 2 seconds per stage", None);
        });

        ui.add_space(6.0);

        // Section 2: Medium Tasks
        ui.group(|ui| {
            ui.label(
                RichText::new("Standard / Medium Tasks (4 to 8 seconds)")
                    .strong()
                    .color(Color32::from_rgb(240, 200, 80)),
            );
            ui.add_space(2.0);

            draw_task_item(ui, "Fix Wiring", "4 to 5 seconds per panel", None);
            draw_task_item(ui, "Empty Garbage", "3 to 4 seconds", None);
            draw_task_item(ui, "Fuel Engines", "4 to 6 seconds per stage", None);
            draw_task_item(ui, "Enter ID Code", "5 to 6 seconds", None);
        });

        ui.add_space(6.0);

        // Section 3: Long Tasks
        ui.group(|ui| {
            ui.label(
                RichText::new("Longer Tasks (9 to 20+ seconds)")
                    .strong()
                    .color(Color32::from_rgb(255, 120, 100)),
            );
            ui.add_space(2.0);

            draw_task_item(ui, "Download / Upload Data", "Around 9 to 10 seconds", None);
            draw_task_item(ui, "Submit MedBay Scan", "Exactly 10 seconds", Some("visual animation"));
            draw_task_item(ui, "Clear Asteroids", "Around 15 seconds", Some("varies based on shots"));
            draw_task_item(ui, "Start Reactor / Unlock Manifolds", "15 to 20 seconds", None);
        });
    });
}

fn draw_task_item(ui: &mut Ui, name: &str, timing: &str, note: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("•").strong().color(Color32::from_rgb(140, 170, 220)));
        ui.label(RichText::new(name).strong().color(Color32::from_rgb(220, 230, 245)));
        ui.label(RichText::new(format!(": {timing}")).color(Color32::from_rgb(180, 210, 250)));
        if let Some(n) = note {
            ui.label(RichText::new(format!("({n})")).small().italics().color(Color32::from_rgb(150, 160, 180)));
        }
    });
}



