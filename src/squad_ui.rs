use crate::squad::{Squad, SquadAi, SquadStates};
use bevy::prelude::*;
use bevy_egui::egui::{self, Color32};

pub fn draw_squad_uis(
    mut egui: bevy_egui::EguiContexts,
    squad_states: Res<SquadStates>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    squad_ais: Query<(&Squad, &GlobalTransform), With<SquadAi>>,
) {
    let ctx = egui.ctx_mut();
    let (camera, camera_tfm) = cameras.single();
    let Some(viewport_rect) = camera.logical_viewport_rect() else {
        return;
    };

    let transparent_white = Color32::from_rgba_unmultiplied(255, 255, 255, 64);
    let stroke = egui::Stroke::new(3.0, transparent_white);

    for (squad, tfm) in &squad_ais {
        let to_egui_pos = |v: Vec2| egui::pos2(v.x, v.y);
        let dbg_painter = ctx.layer_painter(egui::LayerId::debug());

        let ai_pos = tfm.translation();
        let Some(ai_viewport_pos) = camera.world_to_viewport(camera_tfm, ai_pos) else {
            continue;
        };
        let ai_window_pos = ai_viewport_pos + viewport_rect.min;

        dbg_painter.circle(
            to_egui_pos(ai_window_pos),
            10.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 32),
            stroke,
        );

        let state = &squad_states.squads[squad.squad as usize];
        let text = format!(
            "players: {}\nballs: {}% ({})",
            state.num_players,
            state.ball_percent(),
            state.num_holding_balls
        );
        let alignment = egui::Align2::LEFT_TOP;
        dbg_painter.debug_text(
            (to_egui_pos(ai_window_pos).to_vec2() - alignment.to_sign() * egui::vec2(20.0, 20.0))
                .to_pos2(),
            alignment,
            egui::Color32::WHITE,
            text,
        );
    }
}