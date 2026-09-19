# StyleStack JSON schema（v1）

工程文件 `.fntproj` 与样式导出共用这一套字段。颜色均为 `{r,g,b,a}` 0–255。

```json
{
  "version": 1,
  "font_path": "fonts/Title.ttf",
  "font_hash": "…",
  "font_name": "Title",
  "font_size": 48,
  "chars": "0123456789HP+金币",
  "preview_text": "金币 1111HP+",
  "tabular_nums": true,
  "ascii_fallback": true,
  "extra_scales": [2.0],
  "align_h": "Center",
  "pack": { "max_size": 2048, "spacing": 1, "power_of_two": false, "square": false },
  "style": {
    "fill": { "kind": "linear", "angle_deg": 90, "stops": [[0, {"r":255,"g":243,"b":160,"a":255}], [1, {"r":184,"g":132,"b":28,"a":255}]] },
    "stroke": { "enabled": true, "size": 1.5, "position": "outer", "color": {"r":42,"g":24,"b":8,"a":255}, "opacity": 1.0, "blend": "normal" },
    "drop_shadow": { "enabled": false, "use_global_light": true },
    "outer_glow": { "enabled": true, "size": 6, "contour": "linear" },
    "gradient_overlay": { "enabled": true },
    "bevel": { "enabled": false },
    "satin": { "enabled": false },
    "global_light": { "angle_deg": 120, "altitude_deg": 30 }
  }
}
```

ASL / PSD 导入会尽力映射到同一 schema；解析失败时保留已填参数，不启动 Photoshop。
