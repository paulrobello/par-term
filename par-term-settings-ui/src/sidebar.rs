//! Vertical sidebar navigation for settings tabs.
//!
//! This component provides a vertical tab list on the left side of the settings UI,
//! replacing the previous horizontal tab bar for better organization.
//!
//! While a search is active the sidebar shows the results, grouped
//! `Tab › Section › Control` (UX.md SQ4), above the tab list. Tabs with no
//! result are dimmed but stay clickable (SQ5).

use super::SettingsUI;
use crate::search::Hit;

// --- Sidebar color palette ---

/// Text color for tabs that do not match the current search query (dimmed).
const COLOR_TAB_DIMMED: egui::Color32 = egui::Color32::from_rgb(110, 110, 110);

/// Results listed before "N more" (the rest are one click away on their tab).
const MAX_RESULTS: usize = 60;

/// Text color for the currently selected tab (bright white).
const COLOR_TAB_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);

/// Text color for unselected tabs that match the search query.
const COLOR_TAB_NORMAL: egui::Color32 = egui::Color32::from_rgb(180, 180, 180);

/// Background fill for the selected tab row.
const COLOR_TAB_SELECTED_BG: egui::Color32 = egui::Color32::from_rgb(60, 60, 70);

/// Border/stroke color drawn around the selected tab row.
const COLOR_TAB_SELECTED_BORDER: egui::Color32 = egui::Color32::from_rgb(100, 100, 120);

/// Width of the border stroke drawn around the selected tab row (pixels).
const TAB_BORDER_WIDTH: f32 = 1.0;

/// Width of each tab button in the sidebar (pixels).
const TAB_BUTTON_WIDTH: f32 = 140.0;

/// Height of each tab button in the sidebar (pixels).
const TAB_BUTTON_HEIGHT: f32 = 32.0;

/// Vertical spacing added above and below the tab list.
const TAB_LIST_PADDING: f32 = 8.0;

/// The twelve top-level Settings tabs (UX.md 15.2). Each has sub-pages,
/// listed in [`crate::layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Appearance,
    WindowsAndTabs,
    Panes,
    Sessions,
    Profiles,
    Keys,
    Pointer,
    Effects,
    Automation,
    Assistant,
    Advanced,
}

impl SettingsTab {
    /// Get the display name for this tab.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::WindowsAndTabs => "Windows & Tabs",
            Self::Panes => "Panes",
            Self::Sessions => "Sessions",
            Self::Profiles => "Profiles",
            Self::Keys => "Keys",
            Self::Pointer => "Pointer",
            Self::Effects => "Effects & Shaders",
            Self::Automation => "Automation",
            Self::Assistant => "Assistant & Agents",
            Self::Advanced => "Advanced",
        }
    }

    /// Get the icon for this tab. Every glyph is in egui's bundled fonts.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::General => "🏠",
            Self::Appearance => "🎨",
            Self::WindowsAndTabs => "🗗",
            Self::Panes => "◫",
            Self::Sessions => "🔗",
            Self::Profiles => "👤",
            Self::Keys => "⌨",
            Self::Pointer => "🖱",
            Self::Effects => "✨",
            Self::Automation => "⚡",
            Self::Assistant => "💬",
            Self::Advanced => "⚙",
        }
    }

    /// Get all available tabs in order.
    pub fn all() -> &'static [Self] {
        &[
            Self::General,
            Self::Appearance,
            Self::WindowsAndTabs,
            Self::Panes,
            Self::Sessions,
            Self::Profiles,
            Self::Keys,
            Self::Pointer,
            Self::Effects,
            Self::Automation,
            Self::Assistant,
            Self::Advanced,
        ]
    }

    /// Position of this tab in [`Self::all`].
    pub fn index(&self) -> usize {
        Self::all()
            .iter()
            .position(|t| t == self)
            .unwrap_or_default()
    }

    /// Titles of this tab's sub-pages, in order.
    pub fn page_titles(&self) -> Vec<&'static str> {
        crate::layout::pages(*self)
            .iter()
            .map(|p| p.title)
            .collect()
    }
}

/// Render the sidebar: search results (while searching), then the tabs.
///
/// Returns true if the selected tab changed.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI) -> bool {
    let before = settings.selected_tab;
    if !settings.search_query.trim().is_empty() {
        show_results(ui, settings);
        ui.separator();
    }
    show_tabs(ui, settings);
    settings.selected_tab != before
}

/// The grouped result list, or the "No settings match" message.
fn show_results(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    let hits: Vec<Hit> = settings.search_hits().to_vec();
    let registry = settings.search_registry();
    ui.add_space(TAB_LIST_PADDING);
    if hits.is_empty() {
        ui.label(
            egui::RichText::new(no_match_message(settings.search_query.trim()))
                .color(COLOR_TAB_NORMAL),
        );
        ui.label(
            egui::RichText::new("Try fewer words, or the command palette for actions.")
                .small()
                .color(COLOR_TAB_DIMMED),
        );
        return;
    }
    let mut chosen = None;
    let mut last_page = None;
    let mut last_section = None;
    for hit in hits.iter().take(MAX_RESULTS) {
        let section = &registry.sections[hit.section];
        if last_page != Some((section.tab, section.page)) {
            last_page = Some((section.tab, section.page));
            last_section = None;
            ui.label(
                egui::RichText::new(format!(
                    "{} {} \u{203a} {}",
                    section.tab.icon(),
                    section.tab.display_name(),
                    crate::layout::page_at(section.tab, section.page).title
                ))
                .strong()
                .color(COLOR_TAB_SELECTED),
            );
        }
        let row = match registry.control(*hit) {
            None => {
                last_section = Some(hit.section);
                section_row(ui, &section.title)
            }
            Some(control) => {
                if last_section != Some(hit.section) {
                    last_section = Some(hit.section);
                    section_row(ui, &section.title);
                }
                ui.horizontal(|ui| {
                    ui.add_space(RESULT_INDENT * 2.0);
                    ui.add(
                        egui::Button::new(egui::RichText::new(&control.label).small())
                            .frame(false)
                            .truncate(),
                    )
                })
                .inner
            }
        };
        if row.clicked() {
            chosen = Some(*hit);
        }
    }
    if hits.len() > MAX_RESULTS {
        ui.label(
            egui::RichText::new(format!("{} more", hits.len() - MAX_RESULTS))
                .small()
                .color(COLOR_TAB_DIMMED),
        );
    }
    if let Some(hit) = chosen {
        settings.go_to_result(ui.ctx(), hit);
    }
}

/// Indent of section and control rows under their tab heading.
const RESULT_INDENT: f32 = 8.0;

fn section_row(ui: &mut egui::Ui, title: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(RESULT_INDENT);
        ui.add(
            egui::Button::new(egui::RichText::new(title).color(COLOR_TAB_NORMAL))
                .frame(false)
                .truncate(),
        )
    })
    .inner
}

/// The empty-result message (UX.md SQ5).
pub fn no_match_message(query: &str) -> String {
    format!("No settings match \u{201c}{query}\u{201d}")
}

fn show_tabs(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    // Add some vertical spacing at the top
    ui.add_space(TAB_LIST_PADDING);

    for tab in SettingsTab::all() {
        let is_selected = settings.selected_tab == *tab;

        let has_matches = settings.tab_has_results(*tab);

        // Dim tabs with no result; they stay clickable (UX.md SQ5).
        let text_color = if !has_matches {
            COLOR_TAB_DIMMED
        } else if is_selected {
            COLOR_TAB_SELECTED
        } else {
            COLOR_TAB_NORMAL
        };

        let bg_color = if is_selected {
            COLOR_TAB_SELECTED_BG
        } else {
            egui::Color32::TRANSPARENT
        };

        // Create a selectable button-like widget
        let response = ui.add_sized(
            [TAB_BUTTON_WIDTH, TAB_BUTTON_HEIGHT],
            egui::Button::new(
                egui::RichText::new(format!("{} {}", tab.icon(), tab.display_name()))
                    .color(text_color),
            )
            .fill(bg_color)
            .stroke(if is_selected {
                egui::Stroke::new(TAB_BORDER_WIDTH, COLOR_TAB_SELECTED_BORDER)
            } else {
                egui::Stroke::NONE
            }),
        );

        if response.clicked() && settings.selected_tab != *tab {
            settings.set_selected_tab(*tab);
        }

        // Show tooltip with tab contents summary
        response.on_hover_text(tab_contents_summary(*tab));
    }

    ui.add_space(TAB_LIST_PADDING);
}

/// The tab's tooltip: its sub-pages, read from the layout table so it
/// cannot drift from what the tab shows.
fn tab_contents_summary(tab: SettingsTab) -> String {
    tab.page_titles().join(" \u{b7} ")
}

/// Where the user was in Settings: tab, each tab's page, and the search
/// (UX.md B54). The host keeps it across a close and reopen of the window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsNav {
    tab: SettingsTab,
    pages: [usize; 12],
    search_query: String,
}

impl SettingsUI {
    /// Snapshot the tab, pages, and search to restore on the next open.
    pub fn nav_state(&self) -> SettingsNav {
        SettingsNav {
            tab: self.selected_tab,
            pages: self.selected_pages,
            search_query: self.search_query.clone(),
        }
    }

    /// Reopen where [`Self::nav_state`] left off.
    pub fn restore_nav(&mut self, nav: SettingsNav) {
        self.selected_tab = nav.tab;
        for (tab, page) in SettingsTab::all().iter().zip(nav.pages) {
            self.selected_pages[tab.index()] = page.min(crate::layout::pages(*tab).len() - 1);
        }
        self.search_query = nav.search_query;
    }

    /// Get the current selected tab.
    pub fn selected_tab(&self) -> SettingsTab {
        self.selected_tab
    }

    /// Select a tab on the page last shown for it (its first page if none).
    pub fn set_selected_tab(&mut self, tab: SettingsTab) {
        self.selected_tab = tab;
    }

    /// The selected page of the selected tab.
    pub fn selected_page(&self) -> usize {
        self.selected_pages[self.selected_tab.index()]
    }

    /// Select a tab and one of its pages.
    pub fn select_page(&mut self, tab: SettingsTab, page: usize) {
        self.selected_tab = tab;
        let last = crate::layout::pages(tab).len() - 1;
        self.selected_pages[tab.index()] = page.min(last);
    }
}
