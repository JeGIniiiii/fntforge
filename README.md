# FntForge

Open-source bitmap font generator for **Cocos2d-x Lua**.

Build a styled glyph atlas in one window, then export AngelCode BMFont (`.fnt` + PNG) that `cc.Label:createWithBMFont` can load directly.

- Native GUI on Windows and macOS (Linux too)
- Photoshop-inspired layer styles: fill, stroke, drop shadow, inner shadow, outer/inner glow, color overlay
- Character-set presets, tabular digits, live atlas + `.fnt` preview
- CLI for CI / batch export
- GitHub Actions builds release binaries for Windows x64, macOS arm64, macOS x64, and Linux x64

## Cocos2d-x Lua

```lua
local label = cc.Label:createWithBMFont("fonts/gold.fnt", "金币 +1280", cc.TEXT_ALIGNMENT_CENTER)
self:addChild(label)
```

Put `gold.fnt` and `gold.png` in the same folder. The `page file` entry is a bare filename, which is what Cocos expects.

## Install

Download a release artifact from GitHub Actions / Releases, or build from source:

```bash
cargo build --release -p fntforge-app
./target/release/fntforge
```

## CLI

```bash
fntforge export \
  --font MyFont.ttf \
  --size 48 \
  --chars "0123456789金币+-" \
  --stroke 2 --stroke-color 14181c \
  --fill eceff1 \
  --shadow \
  --out ./out \
  --name gold
```

## GUI

Launch with no arguments:

```bash
fntforge
```

Left: font, charset, packing. Center: atlas and `.fnt` text. Right: layer styles. Export writes `.fnt` + PNG next to the path you pick.

## Project layout

```
crates/fntforge-core   rasterize, metrics, MaxRects pack, .fnt writer
crates/fntforge-fx     layer-effect compositor
crates/fntforge-app    egui GUI + clap CLI
```

## License

MIT. Bundled test font is DejaVu Sans (Bitstream Vera / DejaVu license).
