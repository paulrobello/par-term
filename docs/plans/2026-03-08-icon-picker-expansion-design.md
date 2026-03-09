# Icon Picker Expansion + Scrollbar Fix — Design

**Date**: 2026-03-08
**Status**: Approved

## Overview

Expand the curated Nerd Font icon presets in the tab icon picker with more technical language/platform icons and two new fun categories (Weather & Nature, Fun & Seasonal). Also fix a scrollbar overlap bug where the scrollbar obscures icons in the rightmost column of the picker popup.

## Changes

### 1. Scrollbar Padding Fix (`src/tab_bar_ui/context_menu.rs`)

The `ScrollArea` inside the icon picker popup renders icons directly against the edge, causing the egui scrollbar to overlap the rightmost icons when the list exceeds the max height.

**Fix**: Wrap the scroll area inner content in a `Frame::NONE` with a right `inner_margin` to reserve scrollbar width (~8px). This leaves visible space for the scrollbar without clipping icons.

### 2. Icon Additions (`par-term-settings-ui/src/nerd_font.rs`)

#### Expand existing categories

**Dev & Tools** (+11 icons):
TypeScript, Go, C, C++, Angular, Vue.js, Svelte, HTML5, CSS3, Haskell, Scala

**OS & Platforms** (+5 icons):
Ubuntu, Arch Linux, Raspberry Pi, Alpine Linux, Android

**Git & VCS** (+3 icons):
Pull Request, Bitbucket, Diff

#### New categories

**Weather & Nature** (~12 icons):
Sun, Moon, Snowflake, Cloud+Lightning, Umbrella, Leaf, Tree, Mountain, Wind, Thermometer, Stars/Night, Sunrise

**Fun & Seasonal** (~16 icons):
Trophy, Crown, Birthday Cake, Gift, Ghost, Skull, Cat, Dog, Paw, Coffee, Beer, Pizza, Dragon, Magic Wand, Bomb, Dice

#### Total

~47 new icons, bringing total from 192 → ~239 across 14 categories (up from 12).

## Files Changed

| File | Change |
|------|--------|
| `par-term-settings-ui/src/nerd_font.rs` | Add icons to existing categories; add 2 new categories |
| `src/tab_bar_ui/context_menu.rs` | Add right-side padding inside scroll area |
