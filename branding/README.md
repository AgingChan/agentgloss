# AgentGloss brand assets

## Mark

A folded page whose corner is cut into a speech-bubble tail: a document with a
review note attached. The amber line marks the gloss — the comment or suggested
edit anchored to the text.

The mark is deliberately not an eye or sparkle. It should read as
"annotated document", not "preview" or "AI".

## Palette

| Token | Hex | Use |
| --- | --- | --- |
| Ink | `#101827` | Tile background, wordmark on light |
| Border | `#2B374B` | Tile edge on dark |
| Paper | `#F7F1E3` | Page fill, wordmark on dark |
| Paper shade | `#E2D5BD` | Folded corner |
| Amber | `#E9A23B` | Gloss / annotation accent |
| Toolbar ink (light) | `#3B3B3B` | 16 px icon on light themes |
| Toolbar ink (dark) | `#D7D7D7` | 16 px icon on dark themes |

## Files

- `agentgloss-icon.svg` — rounded product tile for surfaces that support
  transparency.
- `agentgloss-icon-square.svg` — full-bleed square mark for the Marketplace and
  other hosts that crop extension icons to rounded corners.
- `agentgloss-logo-light.svg` — horizontal lockup for light backgrounds.
- `agentgloss-logo-dark.svg` — horizontal lockup for dark backgrounds.
- `vscode/media/icon.png` — 128 px extension icon rendered from the full-bleed
  square mark, with no padding or transparent corners.
- `vscode/media/review-icon-light.svg` / `review-icon-dark.svg` — 16 px
  editor-title icons.

## Usage

- Keep clear space around the tile equal to at least one quarter of its width.
- Use the rounded tile in docs and app surfaces; use the full-bleed square for
  Marketplace and OS icons, which often crop the corners themselves. A
  transparent or padded canvas shows a white notch there.
- Use the square mark down to 24 px; use the 16 px toolbar variants in UI
  chrome rather than shrinking the full tile.
- Use the light or dark lockup for the surrounding background; do not recolor
  the paper or amber.
- The wordmark is set in Inter / system sans, lowercase, tight tracking.

## Regenerate the PNG

The extension icon is rendered directly by resvg at exactly 128 × 128, so the
artwork keeps its intended margins and the ink reaches all four corners:

```sh
cargo run --quiet --example render_brand_icon --target aarch64-apple-darwin
```

The example also fails if any corner is not opaque ink, catching padding or a
transparent canvas before a release.
