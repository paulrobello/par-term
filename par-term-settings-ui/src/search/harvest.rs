//! Build the registry by rendering every tab once, headlessly (UX.md SQ1).
//!
//! Sections declare their title, id, and keywords once, at the
//! `collapsing_section` call that draws them. Controls declare nothing: the
//! harvest renders each tab on a private egui context with AccessKit on and
//! every collapsible, popup, and tooltip forced visible, then reads each
//! widget's caption, tooltip, and combo options out of the AccessKit tree. A
//! control compiled out for this platform is never drawn, so it never
//! registers (SQ6), and every caption the registry holds is text the user
//! can see.
//!
//! AccessKit stores a plain label's text as its *value*; every other role
//! keeps its caption in *label* and its current value (a slider's number and
//! unit, a text field's contents, a combo's selection) in *value*. Only
//! captions are indexed, so a unit or the user's own text never matches in
//! place of a label.

use std::collections::{HashMap, HashSet};

use egui::accesskit::{Node, NodeId, Role};

use super::registry::{ControlEntry, SectionEntry};
use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

/// Labels longer than this are descriptions, not captions: they add search
/// text to the control before them instead of becoming a result.
const DESCRIPTION_LEN: usize = 72;

/// Tooltips a single widget may stack in one pass that the harvest reads.
const TOOLTIPS_PER_WIDGET: usize = 3;

#[derive(Clone, Default)]
struct Pending {
    sections: Vec<PendingSection>,
    tags: HashMap<NodeId, Vec<String>>,
}

#[derive(Clone)]
struct PendingSection {
    id: String,
    title: String,
    keywords: Vec<String>,
    header: NodeId,
    body: NodeId,
}

fn pending_id() -> egui::Id {
    egui::Id::new("settings_search_harvest")
}

/// Whether `ctx` is a harvest context.
pub(crate) fn harvesting(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(pending_id().with("on")))
        .unwrap_or(false)
}

/// Record a section drawn during the harvest.
pub(crate) fn record_section(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    keywords: &[&str],
    header: egui::Id,
    body: egui::Id,
) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Pending>(pending_id())
            .sections
            .push(PendingSection {
                id: id.to_string(),
                title: title.to_string(),
                keywords: keywords.iter().map(|k| k.to_string()).collect(),
                header: header.accesskit_id(),
                body: body.accesskit_id(),
            });
    });
}

/// Attach search terms to a control: its YAML key, or a synonym found in no
/// label or tooltip. Terms on a widget with no caption of its own (a
/// slider, a combo box) go to the caption before it. Free outside the
/// harvest.
pub fn tag(response: &egui::Response, terms: &[&str]) {
    if !harvesting(&response.ctx) {
        return;
    }
    response.ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Pending>(pending_id())
            .tags
            .entry(response.id.accesskit_id())
            .or_default()
            .extend(terms.iter().map(|t| t.to_string()));
    });
}

/// `.search_tag(&["yaml_key"])` on a widget's response: see [`tag`].
pub trait SearchTag {
    /// Attach search terms and pass the response through.
    fn search_tag(self, terms: &[&str]) -> Self;
}

impl SearchTag for egui::Response {
    fn search_tag(self, terms: &[&str]) -> Self {
        tag(&self, terms);
        self
    }
}

/// Render one tab on a fresh harvest context and return its sections, with
/// `parent` indices local to the returned list.
pub(crate) fn harvest_tab(settings: &mut SettingsUI, tab: SettingsTab) -> Vec<SectionEntry> {
    settings.selected_tab = tab;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    ctx.options_mut(|o| o.max_passes = std::num::NonZeroUsize::MIN);
    ctx.memory_mut(|m| m.set_everything_is_visible(true));
    ctx.data_mut(|d| d.insert_temp(pending_id().with("on"), true));
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 100_000.0),
        )),
        ..Default::default()
    };
    let mut changes = false;
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut collapsed = std::mem::take(&mut settings.collapsed_sections);
            crate::settings_ui::show_tab_body(ui, settings, tab, &mut changes, &mut collapsed);
            settings.collapsed_sections = collapsed;
        });
    });
    output.textures_delta.clear();

    let pending = ctx
        .data_mut(|d| d.remove_temp::<Pending>(pending_id()))
        .unwrap_or_default();
    let Some(update) = output.platform_output.accesskit_update.take() else {
        return Vec::new();
    };
    let egui_ids: HashMap<NodeId, egui::Id> = ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, rects)| rects.iter().map(|w| (w.id.accesskit_id(), w.id)))
            .collect()
    });
    let tree = Tree::new(update.nodes);
    collect(tab, &tree, &pending, &egui_ids)
}

fn collect(
    tab: SettingsTab,
    tree: &Tree,
    pending: &Pending,
    egui_ids: &HashMap<NodeId, egui::Id>,
) -> Vec<SectionEntry> {
    let body_to_section: HashMap<NodeId, usize> = pending
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| (s.body, i))
        .collect();
    let headers: HashSet<NodeId> = pending.sections.iter().map(|s| s.header).collect();
    let section_of = |id: NodeId| {
        tree.ancestors(id)
            .find_map(|a| body_to_section.get(&a).copied())
    };

    let mut sections: Vec<SectionEntry> = pending
        .sections
        .iter()
        .map(|p| SectionEntry {
            tab,
            id: p.id.clone(),
            title: p.title.clone(),
            keywords: p.keywords.clone(),
            parent: section_of(p.header),
            controls: Vec::new(),
            ..Default::default()
        })
        .collect();

    // The control that captions, tooltips, and tags attach to, per section.
    let mut current: HashMap<usize, usize> = HashMap::new();
    for (id, node) in tree.walk() {
        if headers.contains(&id) {
            continue;
        }
        let Some(si) = section_of(id) else {
            continue;
        };
        let section = &mut sections[si];
        if let Some(text) = caption(node) {
            if text.chars().count() > DESCRIPTION_LEN {
                match current.get(&si) {
                    Some(&ci) => section.controls[ci].extra.push(text),
                    None => section.keywords.push(text),
                }
            } else {
                let ci = push_control(&mut section.controls, text);
                current.insert(si, ci);
            }
        }
        let Some(&ci) = current.get(&si) else {
            continue;
        };
        let mut extra: Vec<String> = pending.tags.get(&id).cloned().unwrap_or_default();
        if let Some(widget) = egui_ids.get(&id) {
            for root in attached_roots(*widget) {
                extra.extend(tree.captions_under(root));
            }
        }
        let control = &mut section.controls[ci];
        for term in extra {
            if term != control.label && !control.extra.contains(&term) {
                control.extra.push(term);
            }
        }
    }
    sections
}

/// Roots of the tooltip and popup areas egui opens for a widget. Their
/// content is parented to the AccessKit root, not the widget, so it is
/// found by id.
fn attached_roots(widget: egui::Id) -> impl Iterator<Item = NodeId> {
    (0..TOOLTIPS_PER_WIDGET)
        .map(move |n| egui::Tooltip::tooltip_id(widget, n).accesskit_id())
        .chain(std::iter::once(
            egui::Id::new(widget).with("popup").accesskit_id(),
        ))
        .chain(std::iter::once(widget.with("popup").accesskit_id()))
}

/// A widget's visible caption, or `None` for widgets without one.
pub(crate) fn caption(node: &Node) -> Option<String> {
    let text = match node.role() {
        Role::Label => node.value(),
        Role::Window | Role::GenericContainer | Role::Unknown | Role::ScrollBar => None,
        _ => node.label(),
    }?;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.chars().any(char::is_alphabetic).then_some(text)
}

fn push_control(controls: &mut Vec<ControlEntry>, label: String) -> usize {
    if let Some(at) = controls.iter().position(|c| c.label == label) {
        return at;
    }
    controls.push(ControlEntry {
        label,
        ..Default::default()
    });
    controls.len() - 1
}

/// The AccessKit tree of one pass, indexed for parent lookups.
pub(crate) struct Tree {
    nodes: HashMap<NodeId, Node>,
    parent: HashMap<NodeId, NodeId>,
    order: Vec<NodeId>,
}

impl Tree {
    pub(crate) fn new(nodes: Vec<(NodeId, Node)>) -> Self {
        let mut parent = HashMap::new();
        for (id, node) in &nodes {
            for child in node.children() {
                parent.insert(*child, *id);
            }
        }
        let nodes: HashMap<NodeId, Node> = nodes.into_iter().collect();
        let order = descendants(&nodes, egui::accesskit_root_id().accesskit_id());
        Self {
            nodes,
            parent,
            order,
        }
    }

    /// Nodes in document order (depth first, children in order).
    fn walk(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.order
            .iter()
            .filter_map(|id| self.nodes.get(id).map(|n| (*id, n)))
    }

    fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(self.parent.get(&id).copied(), |p| {
            self.parent.get(p).copied()
        })
    }

    /// Captions of every node under `root` (tooltip text, combo options).
    fn captions_under(&self, root: NodeId) -> Vec<String> {
        if !self.nodes.contains_key(&root) {
            return Vec::new();
        }
        descendants(&self.nodes, root)
            .into_iter()
            .filter_map(|id| self.nodes.get(&id).and_then(caption))
            .collect()
    }
}

/// `root` and everything under it, depth first in child order.
pub(crate) fn descendants(nodes: &HashMap<NodeId, Node>, root: NodeId) -> Vec<NodeId> {
    let mut order = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        order.push(id);
        if let Some(node) = nodes.get(&id) {
            stack.extend(node.children().iter().rev().copied());
        }
    }
    order
}
