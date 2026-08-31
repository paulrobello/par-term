# Inline Graphics

par-term renders images directly in the terminal through three inline graphics protocols: Sixel, iTerm2 inline images (OSC 1337), and the Kitty graphics protocol. This page covers what each protocol provides, how Kitty placement geometry sizes, crops, and offsets images, and where to look when an image does not appear.

## Table of Contents

- [Overview](#overview)
- [Protocol Support](#protocol-support)
- [Kitty Graphics Protocol](#kitty-graphics-protocol)
  - [Transmission](#transmission)
  - [Placement Geometry](#placement-geometry)
  - [Scrolling and Clipping](#scrolling-and-clipping)
  - [Retransmit and Delete Semantics](#retransmit-and-delete-semantics)
  - [Stream-Order Processing](#stream-order-processing)
- [Displaying Images from the Shell](#displaying-images-from-the-shell)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Related Documentation](#related-documentation)

## Overview

Programs that want to draw images inside the terminal emit escape sequences carrying the image data and placement instructions. par-term decodes all three protocols to RGBA on the CPU, uploads them to GPU textures, and composites them between the cell layer and the UI overlay. Each decoded graphic gets its own GPU texture, cached by the graphic's internal id and shared between its screen and scrollback appearances — placing the same Kitty image at several positions, or retransmitting it, creates separate graphics, each with its own texture.

par-term sets `KITTY_WINDOW_ID` in the environment so tools can detect Kitty graphics protocol support (see the [Environment Variables reference](../guides/ENVIRONMENT_VARIABLES.md)).

## Protocol Support

| Protocol | Transport | Notes |
|----------|-----------|-------|
| Sixel | DCS escape sequences | Raster graphics; decoded and cached like the other protocols |
| iTerm2 inline images | OSC 1337 `File` with `inline=1` | Also used by the `pt-imgcat` shell utility in its default mode |
| Kitty graphics protocol | APC `_G` escape sequences | Direct transmission, deletes, and full placement geometry (see below) |

All three are enabled by default; there is no protocol toggle in config.

## Kitty Graphics Protocol

### Transmission

par-term accepts direct transmission (`a=T`) with PNG (`f=100`), 32-bit RGBA (`f=32`), and 24-bit RGB (`f=24`) payloads. Large payloads arrive as multiple APCs: every chunk except the last carries `m=1`, the final chunk carries `m=0`, and par-term assembles them in order. Responses are suppressed by emitters with `q=2`. zlib-compressed payloads (`o=z`) are decompressed transparently.

The easiest way to emit these sequences from a shell is `pt-imgcat --format kitty` (see [Displaying Images from the Shell](#displaying-images-from-the-shell)).

### Placement Geometry

Kitty placement control keys are honored in any order — crop, offsets, and footprint combine order-independently, so a program may send placement keys before or after crop keys with the same result.

| Key | Unit | Meaning |
|-----|------|---------|
| `c`, `r` | cells | Footprint of the placed image: columns wide, rows tall |
| `x`, `y` | pixels | Origin of the source region within the image |
| `w`, `h` | pixels | Size of the source region (the crop) |
| `X`, `Y` | pixels | Destination offset from the cursor cell's top-left corner |

Behavior details:

- **Cell footprint**: the image is drawn into a `c` x `r` cell rectangle. If only one axis is given, the other is derived from the image's aspect ratio per axis — the omitted axis is computed, not defaulted.
- **Source crop**: `x`/`y`/`w`/`h` select a pixel rectangle of the source image. The renderer maps the crop to the drawn quad's UV coordinates, so no intermediate copy is made.
- **Destination offsets**: `X`/`Y` shift the placement by pixels within (or beyond) the cursor cell, independent of the cell grid.
- **Zero-size crop**: a crop that resolves to zero area produces a zero-size (invisible) placement. It does not fall back to the full image.
- **Virtual placements** (`U=1`) render exactly as in previous releases.

### Scrolling and Clipping

Placed images scroll with the terminal content. Clipping is computed with signed-pixel precision: when an image is partially scrolled past the top or bottom edge of the pane, the visible fragment is clipped at the exact pixel row, including sub-row fragments smaller than one cell height.

### Retransmit and Delete Semantics

Transmitting an image whose id already has placements **replaces** it: the old placements are deleted first, so retransmitting never leaves ghost copies at stale positions. Delete commands (`a=d`) take effect immediately in stream order — a delete followed by a re-place leaves the new placement, while the reverse order removes it. Key order within a single APC (including the components of a `d=` target) does not matter.

### Stream-Order Processing

Kitty APC commands are processed in stream order, with cursor moves between APCs honored. When an APC completes, par-term records the passthrough offset of the stream position, so a program that interleaves cursor moves with image placements gets each placement anchored at the cursor position it named, not at the cursor position of the final APC.

## Displaying Images from the Shell

The `pt-imgcat` utility (installed with shell integration) emits Kitty APCs for you:

```bash
# PNG via the Kitty graphics protocol
pt-imgcat --format kitty photo.png

# Cell-unit sizing becomes c=/r= placement keys
pt-imgcat --format kitty --width 80 --height 24 diagram.png

# Explicit stdin marker
pt-imgcat -
```

Kitty mode requires PNG input. JPEG, GIF, and WebP are auto-converted via `sips` (macOS) or ImageMagick `convert` when available; without a converter the script fails with guidance to use `--format iterm2`. Pixel-unit sizes (`--width 400px`) are rejected in Kitty mode, which supports cells only. The full option reference lives in [File Transfers](FILE_TRANSFERS.md#pt-imgcat----display-inline-images).

## Configuration

Inline graphics respect two image settings from `config.yaml`:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `image_scaling_mode` | enum | `linear` | Texture filtering: `nearest` (sharp, pixel art) or `linear` (smooth) |
| `image_preserve_aspect_ratio` | bool | `true` | Keep aspect ratio when an image is drawn into a requested size |

See the [Config Reference](../CONFIG_REFERENCE.md) for the complete field list.

## Troubleshooting

If images do not appear, start with [Inline Graphics Not Displaying](../guides/TROUBLESHOOTING.md#inline-graphics-not-displaying) in the troubleshooting guide. When an image renders blank but is present, run with `--log-level debug`: the terminal layer logs bounded inline-image payload diagnostics at the upload boundary (first/middle/last RGBA samples and nonzero-alpha counts), which distinguishes a decode or transparency problem from a placement problem. See [Debug Logging](../LOGGING.md).

## Related Documentation

- [File Transfers](FILE_TRANSFERS.md) - `pt-dl`, `pt-ul`, and the full `pt-imgcat` option reference
- [Debug Logging](../LOGGING.md) - Log levels, precedence, and inline-image payload diagnostics
- [Troubleshooting](../guides/TROUBLESHOOTING.md) - Inline graphics and rendering issue resolution
- [Compositor](../architecture/COMPOSITOR.md) - Where the inline graphics layer sits in the render stack
