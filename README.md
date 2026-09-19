# FntForge

面向 **Cocos2d-x Lua** 的开源位图字体生成器。

在一个窗口里做好描边、投影、发光、渐变、斜面，然后导出 AngelCode BMFont（`.fnt` + PNG），可直接给 `cc.Label:createWithBMFont` 用。

当前版本 **v0.4.0**（M5：加字向导 / 保存即工程 / 游戏预览）。

- Windows / macOS / Linux 原生 GUI（可双击运行），界面中文
- 类 Photoshop 图层样式：填充/渐变、描边、投影、内外发光、颜色叠加、斜面浮雕、光泽
- 可导入 `.asl` / `.psd`（不启动 Photoshop）
- 标题美工字体缺「+」时自动用后备字体，避免画成圆环
- 字符集预设（含 GB2312 一级常用字）、从 lua/txt 抽取、缺字报告
- 工程文件 `.fntproj`、CLI 批处理、1x/@2x 同时导出
- 导入已有 `.fnt` 抽字符集并加字，导出 `.style.json` 给同事套同一套样式
- GitHub Actions 自动编译：Windows x64、macOS arm64、macOS x64、Linux x64

## 下载可执行程序

到 [Releases](https://github.com/JeGIniiiii/fntforge/releases) 下载对应平台压缩包：

| 平台 | 怎么运行 |
|---|---|
| Windows | 解压后双击 `FntForge.exe` |
| Mac（Apple 芯片） | 解压后打开 `FntForge.app`。若被拦截：系统设置 → 隐私与安全性 → 仍要打开 |
| Mac（Intel） | 同上，选 `fntforge-macos-x64` |
| Linux | 运行 `FntForge` |

## Cocos2d-x Lua

```lua
local label = cc.Label:createWithBMFont("fonts/gold.fnt", "金币 1111HP+", cc.TEXT_ALIGNMENT_CENTER)
self:addChild(label)
```

把 `gold.fnt` 和 `gold.png` 放在同一目录。`page file` 写的是文件名，Cocos 就是这样读的。

## 命令行

```bash
fntforge export \
  --font MyFont.ttf \
  --size 48 \
  --chars "0123456789金币HP+" \
  --gold \
  --scale 2 \
  --out ./out \
  --name gold
```

`--project font.fntproj` 可走工程文件。无参数启动即为图形界面。

## 源码结构

```
crates/fntforge-core     栅格化、度量、装箱、.fnt 写出
crates/fntforge-fx       图层特效合成（SDF 描边/发光/斜面）
crates/fntforge-import   ASL / PSD 只读导入
crates/fntforge-app      egui 图形界面 + clap 命令行
```

## 许可

MIT。测试字体为 DejaVu Sans。界面中文字体为文泉驿正黑子集。
