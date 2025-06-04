use egui::{Align2, FontId, NumExt, TextStyle, epaint};

use egui::emath::OrderedFloat;
use egui::style::HandleShape;
use egui::{Rangef, lerp, remap, remap_clamp};
use egui::{Rect, Response, Sense, Ui, Vec2, Widget, pos2, vec2};

const FADER_FINE_DRAG_RATIO: f32 = 0.2;
const INFINITY: f32 = f32::INFINITY;

pub enum SignalKind {
    Mono(f32),
    Stereo([f32; 2]),
}

/// Widget for rendering audio level in dB.
pub struct Fader<'a> {
    signal: SignalKind,
    level: &'a mut f32,
    size: Vec2,
    increments: Vec<f32>,
    handle_shape: Option<HandleShape>,
    neutral_level: f32,
    y_padding: f32,
    x_padding: f32,
    text_size: f32,
}

impl<'a> Fader<'a> {
    pub fn new(signal: SignalKind, level: &'a mut f32) -> Self {
        Self {
            signal,
            level,
            size: vec2(75.0, 200.0),
            increments: vec![-100.0, -30.0, -10.0, 0.0, 10.0],
            handle_shape: None,
            neutral_level: 0.0,
            y_padding: 0.1,
            x_padding: 0.1,
            text_size: 10.0,
        }
    }

    #[inline]
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    #[inline]
    pub fn increments(mut self, increments: Vec<f32>) -> Self {
        debug_assert!(
            increments.is_sorted_by(|a, b| OrderedFloat(*a) < OrderedFloat(*b)),
            "Increments must be unique and in ascending order."
        );
        self.increments = increments;
        self
    }

    #[inline]
    pub fn neutral_level(mut self, neutral_level: f32) -> Self {
        self.neutral_level = neutral_level;
        self
    }

    #[inline]
    pub fn y_padding(mut self, y_padding: f32) -> Self {
        self.y_padding = y_padding;
        self
    }

    #[inline]
    pub fn x_padding(mut self, x_padding: f32) -> Self {
        self.x_padding = x_padding;
        self
    }

    #[inline]
    pub fn circle_handle_shape(mut self) -> Self {
        self.handle_shape = Some(HandleShape::Circle);
        self
    }

    #[inline]
    pub fn rect_handle_shape(mut self, aspect_ratio: f32) -> Self {
        self.handle_shape = Some(HandleShape::Rect { aspect_ratio });
        self
    }

    #[inline]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    fn set_level(&mut self, level: f32) {
        *self.level = level
    }

    fn get_level(&self) -> f32 {
        *self.level
    }

    fn handle_radius(&self, rect: &Rect) -> f32 {
        rect.width() / 2.5
    }

    fn handle_shape(&self, ui: &Ui) -> HandleShape {
        self.handle_shape
            .unwrap_or_else(|| ui.style().visuals.handle_shape)
    }

    fn set_to_neutral(&mut self) {
        let min = self.increments[0];
        let max = self.increments[self.increments.len() - 1];
        self.set_level(self.neutral_level.clamp(min, max))
    }

    fn position_range(&self, rect: &Rect, handle_shape: &HandleShape) -> Rangef {
        let handle_radius = self.handle_radius(rect);
        let handle_radius = match handle_shape {
            HandleShape::Circle => handle_radius,
            HandleShape::Rect { aspect_ratio } => handle_radius * aspect_ratio,
        };
        rect.y_range().shrink(handle_radius).flip()
    }

    fn value_from_position(&self, position: f32, position_range: Rangef) -> f32 {
        let normalised = remap_clamp(position, position_range, 0.0..=1.0);
        value_from_normalised(normalised, self.increments.clone())
    }

    fn position_from_value(&self, value: f32, position_range: Rangef) -> f32 {
        let normalised = normalised_from_value(value, self.increments.clone());
        lerp(position_range, normalised)
    }

    fn fader_interaction(&mut self, ui: &Ui, response: &Response) {
        if response.interact(Sense::click()).double_clicked() {
            self.set_to_neutral();
        };
        let rect = &response.rect;
        let handle_shape = self.handle_shape(ui);
        let position_range = self.position_range(rect, &handle_shape);

        if response.dragged() {
            let mut delta = response.drag_delta().y;
            ui.input(|input| {
                if input.modifiers.ctrl || input.modifiers.shift || input.modifiers.alt {
                    delta *= FADER_FINE_DRAG_RATIO
                };
            });
            let centre = self
                .position_from_value(self.get_level(), self.position_range(rect, &handle_shape));
            let new_value = self.value_from_position(centre + delta, position_range);
            self.set_level(new_value)
        }
    }

    fn fader_ui(&mut self, ui: &Ui, response: &Response) {
        // Shrink rect by padding.
        let rect = response.rect;
        let bottom_padding = rect.size().y * 0.1;
        let rect = rect
            .shrink2(vec2(0.0, bottom_padding))
            .translate(vec2(0.0, -bottom_padding * 0.5));

        // Divide response into three sections.
        let (left, right) = rect.split_left_right_at_fraction(1.0 / 5.0);
        let (middle, right) = right.split_left_right_at_fraction(0.5);
        let rail_response = response.clone().with_new_rect(left);
        self.fader_interaction(ui, &rail_response);
        self.rail_ui(ui, &rail_response);
        self.label_ui(ui, middle, &rail_response.rect);
        self.signal_ui(ui, right);
    }

    fn rail_ui(&self, ui: &Ui, response: &Response) {
        // Rail for fader knob.
        let visuals = ui.style().interact(response);
        let rect = response.rect;
        let rail_radius = ui.spacing().slider_rail_height * 0.5;
        let rail_rect = Rect::from_min_max(
            pos2(rect.center().x - rail_radius, rect.top()),
            pos2(rect.center().x + rail_radius, rect.bottom()),
        );
        let rail_corner = ui.visuals().widgets.inactive.corner_radius;
        let rail_style = ui.visuals().widgets.inactive.bg_fill;
        ui.painter().rect_filled(rail_rect, rail_corner, rail_style);

        // Fader knob.
        let handle_radius = self.handle_radius(&rect);
        let handle_shape = self.handle_shape(ui);
        let center = pos2(
            rect.center().x,
            self.position_from_value(self.get_level(), self.position_range(&rect, &handle_shape)),
        );

        match handle_shape {
            HandleShape::Circle => {
                ui.painter().add(epaint::CircleShape {
                    center,
                    radius: handle_radius + visuals.expansion,
                    fill: visuals.bg_fill,
                    stroke: visuals.fg_stroke,
                });
            }
            HandleShape::Rect { aspect_ratio } => {
                let v = Vec2::new(handle_radius, handle_radius * aspect_ratio);
                let v = v + Vec2::splat(visuals.expansion);
                let rect = Rect::from_center_size(center, 2.0 * v);
                ui.painter().rect(
                    rect,
                    visuals.corner_radius,
                    visuals.bg_fill,
                    visuals.fg_stroke,
                    epaint::StrokeKind::Inside,
                );
            }
        }

        // Level text
        let level_text = format!("{:.1}", self.get_level());
        let text_pos = rect.center_bottom();
        let text_anchor = Align2::CENTER_TOP;
        let font_id = FontId::proportional(self.text_size);
        let text_colour = ui.style().visuals.text_color();
        ui.painter()
            .text(text_pos, text_anchor, level_text, font_id, text_colour);
    }

    fn label_ui(&self, ui: &Ui, rect: Rect, rail_rect: &Rect) {
        let handle_shape = self.handle_shape(ui);
        let text_anchor = Align2::CENTER_CENTER;
        let font_id = FontId::proportional(self.text_size);
        let text_colour = ui.style().visuals.text_color();
        for value in self.increments.clone() {
            let text_y =
                self.position_from_value(value, self.position_range(rail_rect, &handle_shape));
            let text_pos = pos2(rect.center().x, text_y);
            let text = format!("{value}");
            ui.painter()
                .text(text_pos, text_anchor, text, font_id.clone(), text_colour);
        }
    }

    fn signal_ui(&self, ui: &Ui, rect: Rect) {
        // Channel to display signal
        let channel_radius = ui.spacing().slider_rail_height * 0.5;
        let channel_corner = ui.style().visuals.widgets.inactive.corner_radius;
        let channel_style = ui.style().visuals.faint_bg_color;
        // Signal
        let signal_corner = ui.style().visuals.widgets.inactive.corner_radius;
        let signal_style = ui.style().visuals.widgets.active.fg_stroke.color;
        match self.signal {
            SignalKind::Mono(signal) => {
                let signal = normalised_from_value(signal, self.increments.clone());
                let signal_height = rect.size().y * signal;
                let signal_y = rect.bottom() - signal_height;
                let channel_rect = Rect::from_min_max(
                    pos2(rect.center().x - channel_radius, rect.top()),
                    pos2(rect.center().x + channel_radius, rect.bottom()),
                );
                let signal_rect = Rect::from_min_size(
                    pos2(rect.center().x - channel_radius, signal_y),
                    vec2(2.0 * channel_radius, signal_height),
                );
                ui.painter()
                    .rect_filled(channel_rect, channel_corner, channel_style);
                ui.painter()
                    .rect_filled(signal_rect, signal_corner, signal_style);
            }
            SignalKind::Stereo([left, right]) => {
                let left = normalised_from_value(left, self.increments.clone());
                let right = normalised_from_value(right, self.increments.clone());
                let left_height = rect.size().y * left;
                let right_height = rect.size().y * right;
                let left_y = rect.bottom() - left_height;
                let right_y = rect.bottom() - right_height;
                let left_x = rect.left() + rect.size().x * 1.0 / 3.0;
                let right_x = rect.left() + rect.size().x * 2.0 / 3.0;
                let left_channel = Rect::from_min_max(
                    pos2(left_x - channel_radius, rect.top()),
                    pos2(left_x + channel_radius, rect.bottom()),
                );
                let right_channel = Rect::from_min_max(
                    pos2(right_x - channel_radius, rect.top()),
                    pos2(right_x + channel_radius, rect.bottom()),
                );
                let left_signal = Rect::from_min_size(
                    pos2(left_x - channel_radius, left_y),
                    vec2(2.0 * channel_radius, left_height),
                );
                let right_signal = Rect::from_min_size(
                    pos2(right_x - channel_radius, right_y),
                    vec2(2.0 * channel_radius, right_height),
                );
                ui.painter()
                    .rect_filled(left_channel, channel_corner, channel_style);
                ui.painter()
                    .rect_filled(right_channel, channel_corner, channel_style);
                ui.painter()
                    .rect_filled(left_signal, signal_corner, signal_style);
                ui.painter()
                    .rect_filled(right_signal, signal_corner, signal_style);
                let left_pos = pos2(left_x, rect.bottom());
                let right_pos = pos2(right_x, rect.bottom());
                let text_anchor = Align2::CENTER_TOP;
                let font_id = FontId::proportional(self.text_size);
                let text_colour = ui.style().visuals.text_color();
                ui.painter()
                    .text(left_pos, text_anchor, "L", font_id.clone(), text_colour);
                ui.painter()
                    .text(right_pos, text_anchor, "R", font_id.clone(), text_colour);
            }
        }
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let old_level = self.get_level();
        let width = 2.0
            * ui.text_style_height(&TextStyle::Body)
                .at_least(ui.spacing().interact_size.x);
        let size = vec2(width, 1.5 * ui.spacing().slider_width);
        let mut response = ui.allocate_response(size, Sense::drag());
        self.fader_ui(ui, &response);
        if self.get_level() != old_level {
            response.mark_changed();
        }
        response
    }
}

impl Widget for Fader<'_> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}

fn normalised_from_value(value: f32, increments: Vec<f32>) -> f32 {
    if value == -INFINITY {
        return 0.0;
    }
    let index = match increments.binary_search_by(|it| OrderedFloat(*it).cmp(&OrderedFloat(value)))
    {
        Ok(index) => index,
        Err(index) => index,
    };
    if index == increments.len() {
        1.0
    } else if index == 0 {
        0.0
    } else {
        let left = increments[index - 1];
        let right = increments[index];
        remap(value, left..=right, (index - 1) as f32..=(index) as f32)
            / (increments.len() - 1) as f32
    }
}

fn value_from_normalised(normalised: f32, increments: Vec<f32>) -> f32 {
    if normalised >= 1.0 {
        increments[increments.len() - 1]
    } else if normalised <= 0.0 {
        -INFINITY
    } else {
        let float_index = normalised * (increments.len() - 1) as f32;
        let index = float_index as usize;
        let left = increments[index];
        let right = increments[index + 1];
        lerp(
            left..=right,
            remap(float_index, index as f32..=(index + 1) as f32, 0.0..=1.0),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn neg_inf_is_normalised_as_0() {
        let increments = vec![-10.0, 0.0];
        assert_eq!(normalised_from_value(-INFINITY, increments), 0.0);
    }

    #[test]
    fn zero_norm_becomes_neg_inf() {
        let increments = vec![-10.0, 0.0];
        assert_eq!(value_from_normalised(0.0, increments), -INFINITY);
    }

    #[test]
    fn asymetric_increments_normalise_equally() {
        let increments = vec![-20.0, -3.0, 0.0, 2.0, 10.0];
        let normals: Vec<_> = increments
            .iter()
            .map(|value| normalised_from_value(*value, increments.clone()))
            .collect();
        assert_eq!(normals, [0.0, 0.25, 0.5, 0.75, 1.0])
    }

    #[test]
    fn midpoint_values_of_asymetric_increments_normalise_equally() {
        let increments = vec![-20.0, -6.0, 0.0, 2.0, 10.0];
        let midpoints = vec![-13.0, -3.0, 1.0, 6.0];
        let normals: Vec<_> = midpoints
            .iter()
            .map(|value| normalised_from_value(*value, increments.clone()))
            .collect();
        assert_eq!(normals, [0.125, 0.375, 0.625, 0.875])
    }

    #[test]
    fn normals_at_asymetric_increments_convert() {
        let normals = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let increments = vec![-20.0, -3.0, 0.0, 2.0, 10.0];
        let values: Vec<_> = normals
            .iter()
            .map(|normal| value_from_normalised(*normal, increments.clone()))
            .collect();
        assert_eq!(values, [-INFINITY, -3.0, 0.0, 2.0, 10.0]);
    }

    #[test]
    fn midpoint_normals_of_asymetric_increments_convert() {
        let normals = vec![0.125, 0.375, 0.625, 0.875];
        let increments = vec![-20.0, -6.0, 0.0, 2.0, 10.0];
        let values: Vec<_> = normals
            .iter()
            .map(|normal| value_from_normalised(*normal, increments.clone()))
            .collect();
        assert_eq!(values, [-13.0, -3.0, 1.0, 6.0]);
    }
}
