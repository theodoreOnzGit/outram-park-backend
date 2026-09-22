//! Browser-style back/forward history (GitHub issue #242, epic #241).
//!
//! What belongs here: [`NavHistory`], a GUI-free back/forward stack with
//! web-browser rules, generic over whatever a "location" is for its caller.
//! The desktop app stores a page plus the concept and paper open on it (see
//! `crate::app`); a terminal front end could store something simpler. Keeping
//! it generic and free of `egui` means it builds everywhere this crate does,
//! Android and wasm included, and is tested headlessly.
//!
//! What does not belong here: deciding what counts as a location, reading one
//! from the app's state, or applying one back to it. That is the caller's
//! job, because only the caller knows its pages.
//!
//! # The rules, as a browser has them
//!
//! 1. [`NavHistory::visit`] of a new location pushes the current one onto the
//!    back stack and **clears the forward stack**, exactly as following a link
//!    after pressing Back discards the pages you had gone back from.
//! 2. Visiting the location you are already on records nothing, so a view
//!    that reports its location every frame does not flood the history.
//! 3. [`NavHistory::back`] and [`NavHistory::forward`] move between the two
//!    stacks and return the location to show; they return `None`, and change
//!    nothing, when there is nowhere to go.
//! 4. The back stack holds at most [`MAX_BACK_ENTRIES`]; the oldest entries
//!    are dropped first.

/// Most entries the back stack keeps before dropping the oldest.
///
/// A bound on memory for a long session, not a behaviour anyone should reach
/// in practice: 200 steps back is far more than a person walks by hand. A UX
/// parameter, nothing more.
pub const MAX_BACK_ENTRIES: usize = 200;

/// Back/forward history over locations of type `L`.
///
/// There is always a current location. `L` only needs to be cloneable and
/// comparable, so the "already here" rule can be checked.
#[derive(Debug, Clone, PartialEq)]
pub struct NavHistory<L> {
    back: Vec<L>,
    current: L,
    forward: Vec<L>,
}

impl<L: Clone + PartialEq> NavHistory<L> {
    /// A history that starts at `start`, with nothing behind or ahead.
    pub fn new(start: L) -> Self {
        Self {
            back: Vec::new(),
            current: start,
            forward: Vec::new(),
        }
    }

    /// The location being shown.
    pub fn current(&self) -> &L {
        &self.current
    }

    /// Go to `location`. Returns `true` if a history step was recorded, and
    /// `false` if `location` is where we already are (rule 2). A recorded
    /// step clears the forward stack (rule 1).
    pub fn visit(&mut self, location: L) -> bool {
        if location == self.current {
            return false;
        }
        let previous = std::mem::replace(&mut self.current, location);
        self.back.push(previous);
        if self.back.len() > MAX_BACK_ENTRIES {
            let excess = self.back.len() - MAX_BACK_ENTRIES;
            self.back.drain(..excess);
        }
        self.forward.clear();
        true
    }

    /// Step back one location and return it, or `None` if there is nothing
    /// behind.
    pub fn back(&mut self) -> Option<&L> {
        let previous = self.back.pop()?;
        let left = std::mem::replace(&mut self.current, previous);
        self.forward.push(left);
        Some(&self.current)
    }

    /// Step forward one location and return it, or `None` if there is nothing
    /// ahead.
    pub fn forward(&mut self) -> Option<&L> {
        let next = self.forward.pop()?;
        let left = std::mem::replace(&mut self.current, next);
        self.back.push(left);
        Some(&self.current)
    }

    /// Whether [`Self::back`] would move.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    /// Whether [`Self::forward`] would move.
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Locations behind the current one, most recent last.
    pub fn back_entries(&self) -> &[L] {
        &self.back
    }

    /// Locations ahead of the current one, the next one last.
    pub fn forward_entries(&self) -> &[L] {
        &self.forward
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_history_can_go_nowhere() {
        let mut h = NavHistory::new("home");
        assert_eq!(*h.current(), "home");
        assert!(!h.can_go_back());
        assert!(!h.can_go_forward());
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), None);
        assert_eq!(*h.current(), "home", "a refused step changes nothing");
    }

    #[test]
    fn back_and_forward_walk_the_visits_in_order() {
        let mut h = NavHistory::new("a");
        assert!(h.visit("b"));
        assert!(h.visit("c"));
        assert_eq!(h.back(), Some(&"b"));
        assert_eq!(h.back(), Some(&"a"));
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), Some(&"b"));
        assert_eq!(h.forward(), Some(&"c"));
        assert_eq!(h.forward(), None);
    }

    /// Rule 1: following a new link after going back discards the pages you
    /// had gone back from.
    #[test]
    fn a_new_visit_clears_forward() {
        let mut h = NavHistory::new("a");
        h.visit("b");
        h.visit("c");
        h.back();
        assert!(h.can_go_forward());
        h.visit("d");
        assert!(!h.can_go_forward(), "forward is gone after a new visit");
        assert_eq!(h.back_entries(), &["a", "b"]);
        assert_eq!(h.back(), Some(&"b"));
    }

    /// Rule 2: reporting the same location every frame records one step.
    #[test]
    fn revisiting_the_current_location_records_nothing() {
        let mut h = NavHistory::new("a");
        h.visit("b");
        for _ in 0..10 {
            assert!(!h.visit("b"));
        }
        assert_eq!(h.back_entries(), &["a"]);
    }

    /// Rule 4: the back stack is bounded, dropping the oldest.
    #[test]
    fn the_back_stack_is_bounded() {
        let mut h = NavHistory::new(0usize);
        for i in 1..=(MAX_BACK_ENTRIES + 50) {
            h.visit(i);
        }
        assert_eq!(h.back_entries().len(), MAX_BACK_ENTRIES);
        assert_eq!(h.back_entries()[0], 50, "the 50 oldest were dropped");
        assert_eq!(*h.current(), MAX_BACK_ENTRIES + 50);
    }
}
