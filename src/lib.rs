use egui::{Align2, Color32, CornerRadius, FontId, NumExt, TextStyle, epaint};

use egui::emath::OrderedFloat;
use egui::style::HandleShape;
use egui::{Rangef, lerp, remap, remap_clamp};
use egui::{Rect, Response, Sense, Ui, Vec2, Widget, pos2, vec2};

mod peak;

use peak::*;

const FADER_FINE_DRAG_RATIO: f32 = 0.2;
const INFINITY: f32 = f32::INFINITY;

/// Specifies the signal kind the [`Fader`] will display.
#[derive(Copy, Clone, PartialEq)]
enum SignalKind {
    Mono(f32),
    Stereo([f32; 2]),
}

/// Wrapper of [`PeakDetector`] to pass any variant of [`SignalKind`].
#[derive(Clone, Debug)]
enum FaderPeak {
    Mono(PeakDetector),
    Stereo([PeakDetector; 2]),
}

impl FaderPeak {
    pub fn next(&mut self, signal: SignalKind) -> SignalKind {
        match self {
            Self::Mono(detector) => {
                let SignalKind::Mono(signal) = signal else {
                    panic!("FaderPeak variant must match SignalKind")
                };
                SignalKind::Mono(detector.next(signal))
            }
            Self::Stereo([left_detector, right_detector]) => {
                let SignalKind::Stereo([left, right]) = signal else {
                    panic!("FaderPeak variant must match SignalKind")
                };
                SignalKind::Stereo([left_detector.next(left), right_detector.next(right)])
            }
        }
    }
}

/// See the signal and control the level of some input.
///
/// Based on an audio fader.
///
/// These faders are not strictly linear or logarithmic, they use a piecewise function with more
/// resolution around 0 and less as the value goes to negative infinity.
/// This piecewise function is here defined by a vector of intervals. This is an ascending list
/// whose values will be evenly distributed across the fader range. Values move linearly between
/// the interval values making the function continuous across the whole range.
///
/// E.g. The interval [-100, -30, -10, 0, 10] gives the first 25% of the interval to [-100, -30], the next 25% to [-30, -10] etc.
///
/// New Fader instances are created with `Fader::mono()` or `Fader::stereo()` depending on the signal
/// type.
///
/// The default (and currently only) behaviour sets the level to `NEG_INFINITY` when the
/// fader handle is at the bottom of the fader.
/// The fader consists of four parts:
///  -  The fader level showing the current level that can be interacted with.
///  -  The text showing the increment values across the range.
///  -  The signal showing the current level of the signal (either mono or stereo).
///  -  A marker indicator showing the most recent peak signal value.
///
///  ```
///  # egui::__run_test_ui(|ui| {
///  # let mut my_level: f32 = -10.0;
///  # let my_signal: f32 = -20.0;
///  # ui.add(egui_fader::Fader::mono(&mut my_level, my_signal));
///  # });
///  ```
///  Code has been adapted from [`egui::Slider`]
///  https://docs.rs/egui/latest/egui/widgets/struct.Slider.html
pub struct Fader<'a> {
    level: &'a mut f32,
    signal: SignalKind,
    increments: Vec<f32>,
    handle_shape: Option<HandleShape>,
    neutral_level: f32,
    text_size: f32,
    height: Option<f32>,
    peak_buffer_size: usize,
}

impl<'a> Fader<'a> {
    /// Creates a fader with only one channel.
    pub fn mono(level: &'a mut f32, signal: f32) -> Self {
        Self::new(level, SignalKind::Mono(signal))
    }

    /// Creates a fader with two channels.
    pub fn stereo(level: &'a mut f32, signal: [f32; 2]) -> Self {
        Self::new(level, SignalKind::Stereo(signal))
    }

    fn new(level: &'a mut f32, signal: SignalKind) -> Self {
        Self {
            level,
            signal,
            increments: vec![-100.0, -30.0, -10.0, 0.0, 10.0],
            handle_shape: None,
            neutral_level: 0.0,
            text_size: 10.0,
            height: None,
            peak_buffer_size: 60,
        }
    }

    /// Set the increments that will make up the faders range.
    /// Increments must be ascending order and will be evenly spaced across the range of the fader.
    /// E.g. the default increments `[-100, -30, -10, 0, 10]` split the range into four segments,
    /// the first 25% of the fader goes from -100 to -30, the next 25% goes from -30 to -10. The
    /// third from -10 to 0 and the final 25% from 0 to 10.
    /// By default, when the fader handle is at the bottom of the fader the value will be set to
    /// `NEG_INFINITY`. This does not need to be included in the intervals.
    #[inline]
    pub fn increments(mut self, increments: Vec<f32>) -> Self {
        debug_assert!(
            increments.is_sorted_by(|a, b| OrderedFloat(*a) < OrderedFloat(*b)),
            "Increments must be unique and in ascending order."
        );
        self.increments = increments;
        self
    }

    /// Set the neutral level that the fader handle will be set to when double clicked.
    #[inline]
    pub fn neutral_level(mut self, neutral_level: f32) -> Self {
        self.neutral_level = neutral_level;
        self
    }

    /// Set the shape of the fader handle to a circle shape.
    /// The default value is set by [`egui::Ui.style().visuals.handle_shape`] but can be
    /// overwritten for this widget here.
    #[inline]
    pub fn circle_handle_shape(mut self) -> Self {
        self.handle_shape = Some(HandleShape::Circle);
        self
    }

    /// Set the shape of the fader handle to a rect shape with some aspect ratio..
    /// The default value is set by [`egui::Ui.style().visuals.handle_shape`] but can be
    /// overwritten for this widget here.
    #[inline]
    pub fn rect_handle_shape(mut self, aspect_ratio: f32) -> Self {
        self.handle_shape = Some(HandleShape::Rect { aspect_ratio });
        self
    }

    /// Set the size of the text displayed on the widget.
    #[inline]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    /// Set the number of frames that will be stored in the peak buffer.
    #[inline]
    pub fn peak_buffer_size(mut self, peak_buffer_size: usize) -> Self {
        self.peak_buffer_size = peak_buffer_size;
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

    fn text_padding(&self) -> f32 {
        self.text_size * 0.25
    }

    /// The interactive element of the fader.
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
        // Shrink rect to allow for text underneath.
        let rect = response.rect;
        let bottom_padding = self.text_size + self.text_padding();
        let rect = rect
            .shrink2(vec2(0.0, bottom_padding))
            .translate(vec2(0.0, -bottom_padding * 0.5));

        // Divide response into three sections.
        let (left, right) = rect.split_left_right_at_fraction(1.0 / 5.0);
        let (middle, right) = right.split_left_right_at_fraction(0.5);
        let rail_response = response.clone().with_new_rect(left);
        self.fader_interaction(ui, &rail_response);
        self.rail_ui(ui, &rail_response);
        self.label_ui(ui, middle, &rail_response);
        self.signal_ui(ui, right, &rail_response);
    }

    fn rail_ui(&self, ui: &Ui, response: &Response) {
        // Rail for fader handle.
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

        // Fader handle.
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
        let text_pos = rect.center_bottom() + vec2(0.0, self.text_padding());
        let text_anchor = Align2::CENTER_TOP;
        let font_id = FontId::proportional(self.text_size);
        let text_colour = ui.style().visuals.text_color();
        ui.painter()
            .text(text_pos, text_anchor, level_text, font_id, text_colour);
    }

    fn label_ui(&self, ui: &Ui, rect: Rect, rail_response: &Response) {
        let rail_rect = &rail_response.rect;
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

    fn channel_style(&self, ui: &Ui) -> (CornerRadius, Color32) {
        let corner = ui.style().visuals.widgets.inactive.corner_radius;
        let colour = ui.style().visuals.faint_bg_color;
        (corner, colour)
    }

    fn signal_style(&self, ui: &Ui) -> (CornerRadius, Color32) {
        let corner = ui.style().visuals.widgets.inactive.corner_radius;
        let colour = ui.style().visuals.widgets.active.fg_stroke.color;
        (corner, colour)
    }

    fn peak_style(&self, ui: &Ui) -> (CornerRadius, Color32) {
        let corner = ui.style().visuals.widgets.active.corner_radius;
        let colour = ui.style().visuals.widgets.inactive.fg_stroke.color;
        (corner, colour)
    }

    fn channel_radius(&self, ui: &Ui) -> f32 {
        ui.spacing().slider_rail_height * 0.5
    }

    fn channel_ui(&self, ui: &Ui, rect: &Rect, signal: f32, peak: f32, centre: f32) {
        let (channel_corner, channel_colour) = self.channel_style(ui);
        let (signal_corner, signal_colour) = self.signal_style(ui);
        let (peak_corner, peak_colour) = self.peak_style(ui);
        let channel_radius = self.channel_radius(ui);
        let signal = normalised_from_value(signal, self.increments.clone());
        let peak = normalised_from_value(peak, self.increments.clone());
        let peak_height = rect.size().y * peak;
        let signal_height = rect.size().y * signal;
        let signal_y = rect.bottom() - signal_height;
        let peak_y = rect.bottom() - peak_height;
        let channel_rect = Rect::from_min_max(
            pos2(centre - channel_radius, rect.top()),
            pos2(centre + channel_radius, rect.bottom()),
        );
        let signal_rect = Rect::from_min_size(
            pos2(centre - channel_radius, signal_y),
            vec2(2.0 * channel_radius, signal_height),
        );
        let peak_rect =
            Rect::from_center_size(pos2(centre, peak_y), Vec2::splat(2.0 * channel_radius));
        ui.painter()
            .rect_filled(channel_rect, channel_corner, channel_colour);
        ui.painter()
            .rect_filled(signal_rect, signal_corner, signal_colour);
        ui.painter()
            .rect_filled(peak_rect, peak_corner, peak_colour);
    }

    fn signal_ui(&self, ui: &Ui, rect: Rect, rail_response: &Response) {
        match self.signal {
            SignalKind::Mono(signal) => {
                let SignalKind::Mono(peak) = self.next_peak(ui, rail_response, self.signal) else {
                    panic!()
                };
                let centre = rect.center().x;
                self.channel_ui(ui, &rect, signal, peak, centre);
            }
            SignalKind::Stereo([left, right]) => {
                let SignalKind::Stereo([left_peak, right_peak]) =
                    self.next_peak(ui, rail_response, self.signal)
                else {
                    panic!()
                };
                let left_x = rect.left() + rect.size().x * 1.0 / 3.0;
                let right_x = rect.left() + rect.size().x * 2.0 / 3.0;
                self.channel_ui(ui, &rect, left, left_peak, left_x);
                self.channel_ui(ui, &rect, right, right_peak, right_x);

                // Text to label the left and right channels.
                let left_pos = pos2(left_x, rect.bottom() + self.text_padding());
                let right_pos = pos2(right_x, rect.bottom() + self.text_padding());
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

    /// Get the peak from the recent buffer.
    fn next_peak(&self, ui: &Ui, response: &Response, signal: SignalKind) -> SignalKind {
        let id = response.id.with("peak");
        ui.memory_mut(|mem| {
            let queue = mem
                .data
                .get_temp_mut_or_insert_with::<FaderPeak>(id, || match signal {
                    SignalKind::Mono(..) => {
                        FaderPeak::Mono(PeakDetector::new(self.peak_buffer_size))
                    }
                    SignalKind::Stereo(..) => FaderPeak::Stereo([
                        PeakDetector::new(self.peak_buffer_size),
                        PeakDetector::new(self.peak_buffer_size),
                    ]),
                });
            queue.next(signal)
        })
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let old_level = self.get_level();
        let width = 2.0
            * ui.text_style_height(&TextStyle::Body)
                .at_least(ui.spacing().interact_size.x);
        let height = self
            .height
            .unwrap_or_else(|| 1.5 * ui.spacing().slider_width);
        let size = vec2(width, height);
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

// ----------------------------------------------------------------------------

// Helpers for converting fader range to/from normalized [0-1] range.

// Convertion to piecewise interval range.

// Always clamps.

// Normalised values of 0.0 will return `NEG_INFINITY`

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
