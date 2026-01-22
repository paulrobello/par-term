use std::time::Instant;

/// State management for scrolling behavior
#[derive(Debug)]
pub struct ScrollState {
    /// Current scroll position (0 = bottom, showing current content)
    pub offset: usize,
    /// Target scroll position for smooth animation
    pub target_offset: usize,
    /// Current animated scroll position (interpolated)
    pub animated_offset: f64,
    /// When scroll animation started
    pub animation_start: Option<Instant>,
    /// Whether currently dragging the scrollbar
    pub dragging: bool,
    /// Distance between cursor and thumb top when dragging
    pub drag_offset: f32,
    /// Last time scroll input happened (for autohide)
    pub last_activity: Instant,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Create a new scroll state
    pub fn new() -> Self {
        Self {
            offset: 0,
            target_offset: 0,
            animated_offset: 0.0,
            animation_start: None,
            dragging: false,
            drag_offset: 0.0,
            last_activity: Instant::now(),
        }
    }

    /// Set scroll target and initiate smooth interpolation animation.
    /// Returns true if the target actually changed.
    pub fn set_target(&mut self, new_offset: usize) -> bool {
        if new_offset != self.target_offset {
            self.target_offset = new_offset;
            self.animation_start = Some(Instant::now());
            self.last_activity = Instant::now();
            true
        } else {
            false
        }
    }

    /// Update smooth scroll animation via interpolation.
    /// Returns true if the animation is still in progress.
    pub fn update_animation(&mut self) -> bool {
        if let Some(start_time) = self.animation_start {
            const ANIMATION_DURATION: f64 = 0.15; // 150ms for snappy feel
            let elapsed = start_time.elapsed().as_secs_f64();

            if elapsed >= ANIMATION_DURATION {
                // Animation finished: snap to exact target
                self.animated_offset = self.target_offset as f64;
                self.offset = self.target_offset;
                self.animation_start = None;
                return false;
            }

            // Easing: ease-out-cubic (1 - (1 - t)^3)
            // Provides fast start and smooth deceleration
            let t = elapsed / ANIMATION_DURATION;
            let eased = 1.0 - (1.0 - t).powi(3);

            let start = self.offset as f64;
            let target = self.target_offset as f64;
            self.animated_offset = start + (target - start) * eased;

            // Update discrete offset for logic that requires integer rows
            self.offset = self.animated_offset.round() as usize;

            return true;
        }

        false
    }

    /// Clamp scroll offset to available scrollback length
    pub fn clamp_to_scrollback(&mut self, max_scroll: usize) {
        if self.offset > max_scroll {
            self.offset = max_scroll;
        }
        // Also clamp target/animation if they exceed max
        if self.target_offset > max_scroll {
            self.target_offset = max_scroll;
            self.animated_offset = max_scroll as f64;
            self.animation_start = None;
        }
    }

    /// Apply a scroll delta
    /// Returns new target offset
    pub fn apply_scroll(&mut self, lines: i32, max_scroll: usize) -> usize {
        if lines > 0 {
            // Scrolling up (into history)
            let new_offset = self.target_offset.saturating_add(lines as usize);
            new_offset.min(max_scroll)
        } else if lines < 0 {
            // Scrolling down (toward current)
            self.target_offset
                .saturating_sub(lines.unsigned_abs() as usize)
        } else {
            self.target_offset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_defaults() {
        let state = ScrollState::new();
        assert_eq!(state.offset, 0);
        assert_eq!(state.target_offset, 0);
        assert!(!state.dragging);
    }

    #[test]
    fn test_set_target() {
        let mut state = ScrollState::new();
        assert!(state.set_target(10));
        assert_eq!(state.target_offset, 10);
        assert!(state.animation_start.is_some());

        // Setting same target should return false
        assert!(!state.set_target(10));
    }

    #[test]
    fn test_clamp_to_scrollback() {
        let mut state = ScrollState::new();
        state.offset = 100;
        state.target_offset = 100;

        state.clamp_to_scrollback(50);

        assert_eq!(state.offset, 50);
        assert_eq!(state.target_offset, 50);
        assert!(state.animation_start.is_none());
    }

    #[test]
    fn test_apply_scroll() {
        let mut state = ScrollState::new();
        let max_scroll = 100;

        // Scroll up (positive)
        assert_eq!(state.apply_scroll(10, max_scroll), 10);
        state.target_offset = 10;

        // Scroll down (negative)
        assert_eq!(state.apply_scroll(-5, max_scroll), 5);
        state.target_offset = 5;

        // Cap at max
        state.target_offset = 95;
        assert_eq!(state.apply_scroll(10, max_scroll), 100);

        // Cap at 0
        state.target_offset = 5;
        assert_eq!(state.apply_scroll(-10, max_scroll), 0);
    }
}
