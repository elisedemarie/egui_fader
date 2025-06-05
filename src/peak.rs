use egui::emath::OrderedFloat;
use std::{
    collections::VecDeque,
    fmt::{self, Debug, Formatter},
};

#[derive(Clone, Copy)]
struct BufferElement {
    index: usize,
    value: OrderedFloat<f32>,
}

impl Debug for BufferElement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("BufferElement")
            .field("index", &self.index)
            .field("value", &self.value.0)
            .finish()
    }
}

impl Default for BufferElement {
    fn default() -> Self {
        BufferElement {
            index: 0,
            value: 0.0.into(),
        }
    }
}

/// Data structure to keep track of the max value over a ring buffer.
/// Extension of a deque but for a new entry it will:
///   - Remove all elements that are now outside of the window.
///   - Remove all elements less than the new entry in value.
///   - Return the highest value.
///
/// This keeps the deque sorted and set to only the buffer giving
/// efficiently returning of the max value.
#[derive(Clone, Debug)]
pub struct PeakDetector {
    deque: VecDeque<BufferElement>,
    buffer_size: usize,
    next_index: usize,
}

impl PeakDetector {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            deque: VecDeque::default(),
            next_index: 0,
        }
    }

    /// Add new element to buffer and return highest value.
    pub fn next(&mut self, value: f32) -> f32 {
        let deque = &mut self.deque;
        let buffer_size = self.buffer_size;
        let next_index = self.next_index;
        let value = OrderedFloat(value);
        // Remove values no longer in the buffer.
        // An element will only stay in the buffer long enough to require removal if its value is
        // the max value.
        // Therefore we only need to check the max value element (back of queue).
        if deque.back().map(|it| it.index) == Some(next_index) {
            deque.pop_back();
        }
        if deque.is_empty() {
            deque.push_back(BufferElement {
                index: next_index,
                value,
            });
        } else if deque.back().unwrap().value <= value {
            // New value is larger than max value.
            // Remove all other elements.
            deque.clear();
            deque.push_back(BufferElement {
                index: next_index,
                value,
            });
        } else {
            // Add element to queue from left.
            // Remove all elements with a value less than or equal to this entry.
            // This is okay as this value is larger and newer.
            // This also keeps the queue sorted and only retaining relevant elements.
            while value >= deque.front().unwrap().value {
                deque.pop_front();
            }
            deque.push_front(BufferElement {
                index: next_index,
                value,
            });
        }
        // Update next index in ring buffer.
        self.next_index = (next_index + 1) % buffer_size;
        // Return max value.
        deque.back().unwrap().value.into_inner()
    }

    #[allow(dead_code)]
    /// Get current max value in buffer.
    pub fn current(&self) -> f32 {
        self.deque
            .back()
            .unwrap_or(&BufferElement::default())
            .value
            .into_inner()
    }
}

#[cfg(test)]
mod test {
    use super::PeakDetector;

    #[test]
    fn tracks_max_ascending_list() {
        let array = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
        let mut detector = PeakDetector::new(10);
        let detected_maxes = array.map(|it| detector.next(it));
        assert_eq!(detected_maxes, array);
    }

    #[test]
    fn tracks_max_descending_list() {
        let array = [0.5, 0.4, 0.3, 0.2, 0.1];
        let mut detector = PeakDetector::new(10);
        let detected_maxes = array.map(|it| detector.next(it));
        assert_eq!(detected_maxes, [0.5; 5])
    }

    #[test]
    fn max_outside_of_buffer_is_removed() {
        let array = [0.5, 0.4, 0.3, 0.2, 0.1];
        let mut detector = PeakDetector::new(4);
        for value in array {
            detector.next(value);
        }
        let expected = 0.4;
        assert_eq!(detector.current(), expected)
    }

    #[test]
    fn detector_handles_array_larger_than_buffer() {
        let array = [0.8, 0.1, 0.3, 0.2, 0.1, 0.6, 0.2];
        let mut detector = PeakDetector::new(4);
        for value in array {
            detector.next(value);
        }
        let expected = 0.6;
        assert_eq!(detector.current(), expected)
    }

    #[test]
    fn new_max_is_detected() {
        let array = [0.5, 0.0, 0.1, 0.0, 0.8];
        let mut detector = PeakDetector::new(10);
        for value in array {
            detector.next(value);
        }
        let expected = 0.8;
        assert_eq!(detector.current(), expected);
    }

    #[test]
    fn empty_buffer_returns_0() {
        let detector = PeakDetector::new(10);
        assert_eq!(detector.current(), 0.0);
    }
}
