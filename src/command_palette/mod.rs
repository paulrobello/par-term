//! Command palette: a summonable, fuzzy-searchable launcher over every
//! dispatchable action.
//!
//! Ranking lives in [`fuzzy`] as pure functions so it can be tested without
//! standing up an egui context or a `WindowState`.

pub(crate) mod fuzzy;
