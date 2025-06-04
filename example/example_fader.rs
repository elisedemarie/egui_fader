use eframe::egui;
use egui_fader::Fader;

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Fader Example",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(FaderExample::default()))),
    )
}

struct FaderExample {
    level: f32,
}

impl Default for FaderExample {
    fn default() -> Self {
        Self {
            level: -20.0
        }
    }
}

impl eframe::App for FaderExample {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let mut level = self.level;
                let signal = [0.0 + level, 0.0 + 0.5 * level];
                ui.add(Fader::stereo(&mut level, signal));
                self.level = level;
            });
        });
    }
}
