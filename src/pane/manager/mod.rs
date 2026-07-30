//! Pane manager for coordinating pane operations within a tab
//!
//! The PaneManager owns the pane tree and provides operations for:
//! - Splitting panes horizontally and vertically
//! - Closing panes
//! - Navigating between panes
//! - Resizing panes
//!
//! # Module Organisation
//!
//! General pane-management sub-modules (not tmux-specific):
//! - [`creation`]: Pane creation and tree manipulation (split, remove).
//! - [`focus`]: Focus management and directional navigation.
//! - [`layout`]: Bounds, resize, and divider operations.
//! - [`session`]: Session restore from saved layout (session-file → pane tree).
//!
//! tmux integration sub-modules (only active when a tmux session is attached):
//! - [`tmux_layout`]: Full tmux layout integration (set, rebuild, update).
//! - [`super::tmux_helpers`]: Helper types and free functions used by `tmux_layout`.
//!
//! `session.rs` is intentionally kept in this module (rather than under a
//! `tmux/` sub-tree) because it also handles non-tmux session restore.  Only
//! `tmux_layout.rs` is exclusively tmux-specific.

mod creation;
mod focus;
mod layout;
mod session;
mod tmux_convert;
mod tmux_layout;
mod tmux_update;

use crate::config::{Config, PaneBackgroundConfig};
use crate::pane::types::{Pane, PaneBounds, PaneId, PaneNode};
use anyhow::Result;
use par_term_terminal::TerminalManager;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

/// Result of extracting a pane from the tree (returns live Pane ownership)
pub enum ExtractResult {
    /// Pane was extracted; remaining tree is returned (None if it was the only pane)
    Extracted {
        pane: Pane,
        remaining: Option<PaneNode>,
    },
    /// The target pane was the only pane in the tree
    OnlyPane(Pane),
    /// Pane was not found in the tree
    NotFound,
}

/// The ids the panes of an adopted subtree hold once
/// [`PaneManager::insert_subtree_at`] has placed them, keyed by the id each
/// pane arrived with.
///
/// Every pane of the inserted subtree appears exactly once; an entry maps to
/// itself when the arriving id was already free here. An id that was never part
/// of the subtree is absent, so a lookup miss means "not from this subtree"
/// rather than "unchanged".
pub type PaneIdRemap = HashMap<PaneId, PaneId>;

/// Internal result for recursive pane extraction (carries PaneNode on NotFound for tree reconstruction)
enum ExtractInternal {
    Extracted { pane: Pane, remaining: PaneNode },
    OnlyPane(Pane),
    NotFound(PaneNode),
}

/// Manages the pane tree within a single tab
pub struct PaneManager {
    /// Root of the pane tree (None if no panes yet)
    pub(super) root: Option<PaneNode>,
    /// ID of the currently focused pane
    pub(super) focused_pane_id: Option<PaneId>,
    /// Counter for generating unique pane IDs
    pub(super) next_pane_id: PaneId,
    /// Width of dividers between panes in pixels
    pub(super) divider_width: f32,
    /// Width of the hit area for divider drag detection
    pub(super) divider_hit_width: f32,
    /// Current total bounds available for panes
    pub(super) total_bounds: PaneBounds,
}

impl PaneManager {
    /// Create a new empty pane manager
    pub fn new() -> Self {
        Self {
            root: None,
            focused_pane_id: None,
            next_pane_id: 1,
            divider_width: 1.0,     // Default 1 pixel divider
            divider_hit_width: 8.0, // Default 8 pixel hit area
            total_bounds: PaneBounds::default(),
        }
    }

    /// Create a pane manager pre-populated with a single primary pane that
    /// wraps an already-running `TerminalManager`.
    ///
    /// This is used by `Tab::new_internal` so that every tab always starts with
    /// a `PaneManager` (R-32).  The primary pane shares the caller's `terminal`
    /// `Arc`, so no new shell process is spawned.
    ///
    /// # Arguments
    /// * `terminal` — `Arc` cloned from `Tab::terminal`
    /// * `working_directory` — Optional CWD for the primary pane
    /// * `is_active` — Shared atomic cloned from `Tab::is_active`
    pub fn new_with_existing_terminal(
        terminal: Arc<RwLock<TerminalManager>>,
        working_directory: Option<String>,
        is_active: Arc<AtomicBool>,
    ) -> Self {
        let mut manager = Self::new();
        let primary_pane_id = manager.next_pane_id;
        manager.next_pane_id += 1;

        let pane =
            Pane::new_wrapping_terminal(primary_pane_id, terminal, working_directory, is_active);

        manager.root = Some(PaneNode::leaf(pane));
        manager.focused_pane_id = Some(primary_pane_id);

        manager
    }

    /// Create a pane manager wrapping an existing `Pane` as the single root node.
    ///
    /// Used by `Tab::new_from_pane()` when promoting a pane to its own tab.
    /// The pane's ID is preserved and `next_pane_id` is advanced past it.
    pub fn new_with_pane(pane: Pane) -> Self {
        let pane_id = pane.id;
        let mut manager = Self::new();
        manager.next_pane_id = pane_id + 1;
        manager.root = Some(PaneNode::leaf(pane));
        manager.focused_pane_id = Some(pane_id);
        manager
    }

    /// Create a pane manager with an initial pane
    pub fn with_initial_pane(
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> Result<Self> {
        let mut manager = Self::new();
        manager.divider_width = config.panes.pane_divider_width.unwrap_or(1.0);
        manager.divider_hit_width = config.panes.pane_divider_hit_width;
        manager.create_initial_pane(config, runtime, working_directory)?;
        Ok(manager)
    }

    /// Get the next pane ID that will be assigned
    pub fn next_pane_id(&self) -> PaneId {
        self.next_pane_id
    }

    /// Get a pane by ID
    pub fn get_pane(&self, id: PaneId) -> Option<&Pane> {
        self.root.as_ref()?.find_pane(id)
    }

    /// Get a mutable pane by ID
    pub fn get_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.root.as_mut()?.find_pane_mut(id)
    }

    /// Get all panes
    pub fn all_panes(&self) -> Vec<&Pane> {
        self.root
            .as_ref()
            .map(|r| r.all_panes())
            .unwrap_or_default()
    }

    /// Get all panes mutably
    pub fn all_panes_mut(&mut self) -> Vec<&mut Pane> {
        self.root
            .as_mut()
            .map(|r| r.all_panes_mut())
            .unwrap_or_default()
    }

    /// Collect current per-pane background settings for config persistence
    ///
    /// Returns a `Vec<PaneBackgroundConfig>` containing only panes that have
    /// a custom background image set. The `index` field corresponds to the
    /// pane's position in the tree traversal order.
    pub fn collect_pane_backgrounds(&self) -> Vec<PaneBackgroundConfig> {
        self.all_panes()
            .iter()
            .enumerate()
            .filter_map(|(index, pane)| {
                pane.background
                    .image_path
                    .as_ref()
                    .map(|path| PaneBackgroundConfig {
                        index,
                        image: path.clone(),
                        mode: pane.background.mode,
                        opacity: pane.background.opacity,
                        darken: pane.background.darken,
                    })
            })
            .collect()
    }

    /// Get the number of panes
    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map(|r| r.pane_count()).unwrap_or(0)
    }

    /// Check if there are multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.pane_count() > 1
    }

    /// Get access to the root node (for rendering)
    pub fn root(&self) -> Option<&PaneNode> {
        self.root.as_ref()
    }

    /// Get mutable access to the root node
    pub fn root_mut(&mut self) -> Option<&mut PaneNode> {
        self.root.as_mut()
    }

    /// Insert a `PaneNode` subtree into the tree by splitting the target pane.
    ///
    /// The target leaf is replaced with a `Split` containing the original pane
    /// as one child and the `subtree` as the other. Bounds are recalculated.
    ///
    /// On success, returns the ids the adopted panes hold afterwards, which are
    /// **not** always the ids they arrived with.
    ///
    /// On failure — there is no tree here to insert into, or no pane holds
    /// `target_pane_id` — the tree is left untouched and the `subtree` is handed
    /// **back**, carrying the ids it arrived with. Handing it back is the point
    /// of the `Err`, not a courtesy: a caller gets a subtree to pass here by
    /// detaching it from wherever it was living, so this is the only remaining
    /// reference to those panes, and dropping it closes them and kills their
    /// PTYs. The caller must put it back somewhere.
    ///
    /// A subtree arriving from another tab ("Demote Tab to Pane") brings that
    /// tab's ids with it, and ids are allocated per `PaneManager` — that is,
    /// per tab — so an incoming id can already belong to a pane here: tab A's
    /// panes 1 and 2 landing in a tab that already holds 1 and 2. `get_pane` is
    /// a search over the tree, so a duplicate would make every id-keyed lookup
    /// resolve to whichever pane the walk reaches first, and the adopted pane
    /// would be unreachable. Colliding panes are therefore renumbered from this
    /// manager's counter.
    ///
    /// Advancing the counter past every adopted id closes the other half: this
    /// tab must not later hand a *new* pane an id one of the adopted panes is
    /// already using.
    ///
    /// A caller holding an id from the subtree across this call must translate
    /// it through the returned map; the pre-move id may now name a different
    /// pane, or no pane at all.
    #[must_use = "on success a renumbered pane is only reachable through the returned \
                  remap; on failure the returned subtree owns live terminals and dropping \
                  it kills them"]
    pub fn insert_subtree_at(
        &mut self,
        target_pane_id: PaneId,
        mut subtree: PaneNode,
        direction: crate::pane::types::SplitDirection,
        ratio: f32,
    ) -> Result<PaneIdRemap, PaneNode> {
        let Some(root) = self.root.take() else {
            return Err(subtree);
        };

        // Reconcile before the insert, not after: `insert_subtree_at_node`
        // searches for `target_pane_id`, and a subtree pane still carrying that
        // id could be found first and split instead of the intended target.
        // Still cheaper than walking the tree twice to pre-check that the target
        // exists; a failed insert undoes the renumbering below instead.
        let remap = self.reconcile_adopted_ids(&root, &mut subtree);

        match Self::insert_subtree_at_node(root, target_pane_id, subtree, direction, ratio) {
            Ok(new_root) => {
                self.root = Some(new_root);
                self.recalculate_bounds();

                // Guard the mutation rather than `get_pane`: this is the only
                // path by which a pane this manager did not allocate enters the
                // tree, and the lookups run per frame and inside loops.
                debug_assert!(
                    self.pane_ids_are_unique(),
                    "pane ids are not unique after adopting a subtree: {:?}",
                    self.root.as_ref().map(PaneNode::all_pane_ids)
                );

                Ok(remap)
            }
            Err((original_root, mut subtree)) => {
                self.root = Some(original_root);
                Self::undo_reconcile(&mut subtree, &remap);
                Err(subtree)
            }
        }
    }

    /// Put back the ids [`Self::reconcile_adopted_ids`] replaced, so a subtree
    /// handed back after a failed insert is the subtree that arrived.
    ///
    /// Without this the caller would be restoring panes that carry ids drawn
    /// from *this* manager's counter into a tree that never agreed to them —
    /// the collision this whole path exists to prevent, reintroduced by the
    /// recovery. The ids a subtree arrived with are always safe to restore,
    /// because they are the ids it already held wherever it came from.
    ///
    /// Inverting the map is sound because `reconcile_adopted_ids` gives every
    /// pane a distinct id, so no two entries share a value. Panes are matched
    /// by id rather than by traversal position, which keeps this independent of
    /// the order `all_panes_mut` happens to walk in.
    ///
    /// `next_pane_id` is deliberately left advanced: the subtree goes back to
    /// the tab it came from, whose counter is already past its own ids, and ids
    /// skipped here cost nothing.
    fn undo_reconcile(subtree: &mut PaneNode, remap: &PaneIdRemap) {
        let arrived_as: HashMap<PaneId, PaneId> = remap
            .iter()
            .map(|(&arrived, &now)| (now, arrived))
            .collect();

        for pane in subtree.all_panes_mut() {
            if let Some(&arrived) = arrived_as.get(&pane.id) {
                pane.id = arrived;
            }
        }
    }

    /// Give every pane of an incoming subtree an id that is free in this tab,
    /// and move `next_pane_id` past all of them.
    ///
    /// `existing_root` is this manager's tree, passed in because
    /// [`Self::insert_subtree_at`] has already taken it out of `self`.
    fn reconcile_adopted_ids(
        &mut self,
        existing_root: &PaneNode,
        subtree: &mut PaneNode,
    ) -> PaneIdRemap {
        let existing: HashSet<PaneId> = existing_root.all_pane_ids().into_iter().collect();

        // Seed above every id in *both* trees so each allocation is free
        // without searching: a replacement must dodge the ids the subtree keeps
        // as well as the ids already here (target {1}, subtree {2,1} would
        // otherwise renumber the incoming 1 onto the incoming 2).
        let mut next = self.next_pane_id;
        for id in existing.iter().copied().chain(subtree.all_pane_ids()) {
            next = next.max(id.saturating_add(1));
        }

        let mut remap = PaneIdRemap::new();
        for pane in subtree.all_panes_mut() {
            let arrived_as = pane.id;
            if existing.contains(&arrived_as) {
                log::info!(
                    "Adopted pane id {} is already taken in this tab; renumbering it to {}",
                    arrived_as,
                    next
                );
                pane.id = next;
                next = next.saturating_add(1);
            }
            remap.insert(arrived_as, pane.id);
        }

        self.next_pane_id = next;
        remap
    }

    /// Whether no two panes in the tree share an id.
    fn pane_ids_are_unique(&self) -> bool {
        let ids = self
            .root
            .as_ref()
            .map(PaneNode::all_pane_ids)
            .unwrap_or_default();
        ids.iter().collect::<HashSet<_>>().len() == ids.len()
    }

    /// Extract a pane from the tree by ID, returning ownership of the live `Pane`.
    ///
    /// Unlike `remove_pane()` which drops the pane, this returns the pane
    /// intact so it can be transferred to another tab or pane tree.
    /// All processes in the pane's PTY continue running.
    pub fn extract_pane(&mut self, target_id: PaneId) -> ExtractResult {
        if let Some(root) = self.root.take() {
            match Self::extract_pane_from_node(root, target_id) {
                ExtractInternal::Extracted { pane, remaining } => {
                    self.root = Some(remaining);
                    if let Some(id) = self.focused_pane_id
                        && id == target_id
                    {
                        self.focused_pane_id = self.root.as_ref().and_then(|r| r.first_pane_id());
                    }
                    ExtractResult::Extracted {
                        pane,
                        remaining: self.root.take(),
                    }
                }
                ExtractInternal::OnlyPane(pane) => {
                    self.root = None;
                    self.focused_pane_id = None;
                    ExtractResult::OnlyPane(pane)
                }
                ExtractInternal::NotFound(node) => {
                    self.root = Some(node);
                    ExtractResult::NotFound
                }
            }
        } else {
            ExtractResult::NotFound
        }
    }

    /// Take ownership of the root node, leaving `None` in its place.
    ///
    /// Used by demote (tab → pane) to extract the entire pane tree for
    /// transplantation into another tab.
    pub fn take_root(&mut self) -> Option<PaneNode> {
        self.focused_pane_id = None;
        self.root.take()
    }

    /// Replace the root node (drops any existing tree).
    ///
    /// Used after `extract_pane` when the remaining tree needs to be
    /// restored and the default `ExtractResult::Extracted.remaining`
    /// has already been taken by the caller.
    pub fn set_root(&mut self, node: PaneNode) {
        self.root = Some(node);
        self.recalculate_bounds();
    }
}

impl Default for PaneManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::types::SplitDirection;

    // Note: Full tests would require mocking TerminalManager
    // These are placeholder tests for the manager logic

    #[test]
    fn test_pane_manager_new() {
        let manager = PaneManager::new();
        assert!(manager.root.is_none());
        assert_eq!(manager.pane_count(), 0);
        assert!(!manager.has_multiple_panes());
    }

    /// A pane whose terminal has no shell spawned, so it runs without a PTY on
    /// every supported platform.
    ///
    /// `working_directory` carries the marker: `insert_subtree_at` rewrites
    /// only `bounds` (via `recalculate_bounds`), so the marker survives the
    /// insert and identifies *which* pane a lookup resolved to.
    fn stub_pane(id: PaneId, marker: &str) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(80, 24, 100)
            .expect("stub terminal creation without a shell");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            Some(marker.to_string()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn marker_of(manager: &PaneManager, id: PaneId) -> Option<String> {
        manager.get_pane(id)?.working_directory.clone()
    }

    /// A two-pane manager holding ids 1 and 2, as a tab that has been split
    /// once would.
    fn manager_with_two_panes(first: &str, second: &str) -> PaneManager {
        let mut manager = PaneManager::new();
        manager.root = Some(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub_pane(1, first)),
            PaneNode::leaf(stub_pane(2, second)),
        ));
        manager.focused_pane_id = Some(1);
        manager.next_pane_id = 3;
        manager
    }

    #[test]
    fn an_adopted_subtree_does_not_shadow_panes_that_already_hold_its_ids() {
        // Tab A holds panes {1,2} and tab B holds {1,2}; the user demotes A
        // into B. Ids are allocated per tab, so B is handed two panes claiming
        // ids it already uses. `get_pane` searches the tree, so without
        // renumbering both lookups would resolve to B's *original* panes and
        // the demoted terminals would be unreachable — a resize, a paste, or a
        // close would hit the wrong terminal.
        const TARGET_1: &str = "/target/one";
        const TARGET_2: &str = "/target/two";
        const MOVED_1: &str = "/moved/one";
        const MOVED_2: &str = "/moved/two";

        let mut target = manager_with_two_panes(TARGET_1, TARGET_2);

        // The subtree arriving from another tab, carrying that tab's ids.
        let subtree = PaneNode::split(
            SplitDirection::Horizontal,
            0.5,
            PaneNode::leaf(stub_pane(1, MOVED_1)),
            PaneNode::leaf(stub_pane(2, MOVED_2)),
        );

        let Ok(remap) = target.insert_subtree_at(1, subtree, SplitDirection::Vertical, 0.5) else {
            panic!("target pane 1 exists, so the insert succeeds");
        };

        assert_eq!(target.pane_count(), 4, "all four panes must survive");

        // The lookups are the point: id distinctness below is only the means.
        for (arrived_as, marker) in [(1, MOVED_1), (2, MOVED_2)] {
            let now = remap[&arrived_as];
            assert_eq!(
                marker_of(&target, now).as_deref(),
                Some(marker),
                "get_pane(remap[{arrived_as}]) must resolve to the pane that moved in, \
                 not to the pane that was already holding that id"
            );
            assert_ne!(now, arrived_as, "a colliding id must not be kept");
        }

        for (id, marker) in [(1, TARGET_1), (2, TARGET_2)] {
            assert_eq!(
                marker_of(&target, id).as_deref(),
                Some(marker),
                "the pane that already held id {id} must keep it"
            );
        }
    }

    #[test]
    fn an_adopted_pane_keeps_an_id_that_is_free() {
        let mut target = manager_with_two_panes("/target/one", "/target/two");

        let Ok(remap) = target.insert_subtree_at(
            1,
            PaneNode::leaf(stub_pane(9, "/moved")),
            SplitDirection::Vertical,
            0.5,
        ) else {
            panic!("target pane 1 exists, so the insert succeeds");
        };

        assert_eq!(remap[&9], 9, "an id that is free must be kept as-is");
        assert_eq!(marker_of(&target, 9).as_deref(), Some("/moved"));
    }

    #[test]
    fn an_adopted_pane_id_is_never_handed_out_again() {
        // A subtree moved in from another tab brings that tab's ids. Without
        // advancing the counter this tab would later allocate the same id for
        // an unrelated pane, and every id-keyed lookup would then be ambiguous.
        let mut target = manager_with_two_panes("/target/one", "/target/two");

        let inserted = target.insert_subtree_at(
            1,
            PaneNode::leaf(stub_pane(9, "/moved")),
            SplitDirection::Vertical,
            0.5,
        );
        assert!(
            inserted.is_ok(),
            "target pane 1 exists, so the insert succeeds"
        );

        let fresh = target.next_pane_id();
        let live = target
            .root
            .as_ref()
            .map(PaneNode::all_pane_ids)
            .unwrap_or_default();
        assert!(
            live.iter().all(|id| fresh > *id),
            "next id {fresh} must be past every adopted id {live:?}"
        );
        assert!(
            !live.contains(&fresh),
            "the next id {fresh} must be free, but the tree holds {live:?}"
        );
    }

    #[test]
    fn a_replacement_id_dodges_the_ids_the_subtree_keeps() {
        // Target {1}, subtree {2,1}: the incoming 1 collides and the incoming 2
        // does not, so a counter seeded only from the target's ids would
        // renumber the 1 straight onto the 2.
        let mut target = PaneManager::new();
        target.root = Some(PaneNode::leaf(stub_pane(1, "/target")));
        target.focused_pane_id = Some(1);
        target.next_pane_id = 2;

        let subtree = PaneNode::split(
            SplitDirection::Horizontal,
            0.5,
            PaneNode::leaf(stub_pane(2, "/moved/two")),
            PaneNode::leaf(stub_pane(1, "/moved/one")),
        );

        let Ok(remap) = target.insert_subtree_at(1, subtree, SplitDirection::Vertical, 0.5) else {
            panic!("target pane 1 exists, so the insert succeeds");
        };

        assert_eq!(remap[&2], 2, "the non-colliding incoming id is kept");
        assert_ne!(remap[&1], 2, "the replacement must not land on the kept id");
        assert_eq!(marker_of(&target, remap[&1]).as_deref(), Some("/moved/one"));
        assert_eq!(marker_of(&target, 2).as_deref(), Some("/moved/two"));
        assert_eq!(marker_of(&target, 1).as_deref(), Some("/target"));
    }

    #[test]
    fn a_failed_insert_leaves_the_tree_untouched() {
        let mut target = manager_with_two_panes("/target/one", "/target/two");

        let result = target.insert_subtree_at(
            99,
            PaneNode::leaf(stub_pane(1, "/moved")),
            SplitDirection::Vertical,
            0.5,
        );

        assert!(result.is_err(), "no such target pane, so no insertion");
        assert_eq!(target.pane_count(), 2, "the original tree is restored");
        assert_eq!(marker_of(&target, 1).as_deref(), Some("/target/one"));
        assert_eq!(marker_of(&target, 2).as_deref(), Some("/target/two"));
    }

    #[test]
    fn a_failed_insert_hands_the_subtree_back_alive() {
        // Dropping the subtree here closes its panes and kills their PTYs, and
        // the caller reaches this function by detaching the subtree from
        // somewhere else — so this is the only reference to those terminals.
        let mut target = manager_with_two_panes("/target/one", "/target/two");

        let subtree = PaneNode::split(
            SplitDirection::Horizontal,
            0.5,
            PaneNode::leaf(stub_pane(1, "/moved/one")),
            PaneNode::leaf(stub_pane(2, "/moved/two")),
        );

        // Held across the call so the terminal's liveness is observable
        // independently of whether the subtree comes back.
        let kept_terminal = Arc::clone(&subtree.all_panes()[0].terminal);
        let refs_before = Arc::strong_count(&kept_terminal);
        let counter_before = target.next_pane_id();

        let Err(returned) = target.insert_subtree_at(99, subtree, SplitDirection::Vertical, 0.5)
        else {
            panic!("no pane holds id 99, so the insert must fail");
        };

        assert_eq!(
            Arc::strong_count(&kept_terminal),
            refs_before,
            "the moved terminal must still be referenced by its pane, not dropped"
        );
        assert_eq!(returned.pane_count(), 2, "both moved panes must come back");

        // Ids must come back as they arrived. The replacements handed out
        // during reconciliation were drawn from *this* manager's counter, and
        // the caller is about to restore the subtree to a tree that never
        // agreed to them — restoring renumbered panes would reintroduce exactly
        // the collision reconciliation exists to prevent.
        let mut returned_ids = returned.all_pane_ids();
        returned_ids.sort_unstable();
        assert_eq!(
            returned_ids,
            vec![1, 2],
            "the subtree must carry the ids it arrived with"
        );
        for (id, marker) in [(1, "/moved/one"), (2, "/moved/two")] {
            assert_eq!(
                returned
                    .find_pane(id)
                    .and_then(|p| p.working_directory.clone())
                    .as_deref(),
                Some(marker),
                "restored id {id} must name the pane that originally held it"
            );
        }

        // The target's own panes are untouched, and its counter stays advanced:
        // the subtree is going back to a tab whose counter is already past
        // those ids, so ids skipped here cost nothing.
        assert_eq!(marker_of(&target, 1).as_deref(), Some("/target/one"));
        assert_eq!(marker_of(&target, 2).as_deref(), Some("/target/two"));
        assert!(
            target.next_pane_id() >= counter_before,
            "the target's counter must not rewind onto an id its own panes hold"
        );
    }
}
