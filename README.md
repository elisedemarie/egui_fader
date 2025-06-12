# egui_fader
Interactable fader for [egui](https://github.com/emilk/egui).

Based on an audio fader, a widget that shows input signal and allows modifying the level of inputs. The interactable component uses code from `egui::Slider` but allows the range to be set with a piecewise function.

![image](https://github.com/user-attachments/assets/4a2a78b9-9936-4977-ba53-7fcefde56743)

## Other Features
- The most recent peak is shown on the fader.
- Double click returns the level to neutral (0 by default).
- Fine dragging when holding down shift, control, or alt.
