# egui_fader
Based on an audio fader, a widget for viewing signal and modifying the level of inputs. Based on `egui::Slider` but adapted to display some input signal as well as modify the level using a piecewise range.

Adds some quality of life improvements:
- The fader can be double clicked and the level will return to neutral (0 by default).
- Fine dragging when holding down shift, control, or alt.
