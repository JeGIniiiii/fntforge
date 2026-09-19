# FntForge

面向 **Cocos2d-x Lua** 的开源位图字体生成器。

在一个窗口里做好描边、投影、发光，然后导出 AngelCode BMFont（`.fnt` + PNG），可直接给 `cc.Label:createWithBMFont` 用。

- Windows / macOS / Linux 原生 GUI（可双击运行）
- 类 Photoshop 图层样式：填充、描边、投影、内阴影、外/内发光、颜色叠加
- 字符集预设、等宽数字、实时图集与 `.fnt` 预览
- 命令行批量导出
- GitHub Actions 自动编译：Windows x64、macOS arm64、macOS x64、Linux x64

## 下载可执行程序

到 [Releases](https://github.com/JeGIniiiii/fntforge/releases) 下载对应平台压缩包：

| 平台 | 怎么运行 |
|---|---|
| Windows | 解压后双击 `FntForge.exe` |
| macOS（Apple 芯片） | 解压后打开 `FntForge.app`。若被拦截：系统设置 → 隐私与安全性 → 仍要打开 |
| macOS（Intel） | 同上，选 `fntforge-macos-x64` |
| Linux | 运行 `FntForge` |

## Cocos2d-x Lua

```lua
local label = cc.Label:createWithBMFont("fonts/gold.fnt", "金币 +1280", cc.TEXT_ALIGNMENT_CENTER)
self:addChild(label)
```

把 `gold.fnt` 和 `gold.png` 放在同一目录。`page file` 写的是文件名，Cocos 就是这样读的。

## 命令行

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

无参数启动即为图形界面。

## 源码结构

```
crates/fntforge-core   栅格化、度量、装箱、.fnt 写出
crates/fntforge-fx     图层特效合成
crates/fntforge-app    egui 图形界面 + clap 命令行
```

从源码编译：

```bash
cargo build --release -p fntforge-app
```

## 许可

MIT。测试字体为 DejaVu Sans（Bitstream Vera / DejaVu）。界面中文字体为文泉驿正黑子集。
