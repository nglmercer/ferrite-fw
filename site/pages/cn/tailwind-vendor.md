---
title: Tailwind Vendor
order: 7
---

# Tailwind Vendor

Ferrite 用从 [tailwind-rs](https://github.com/nglmercer/tailwind-rs)
vendored 来的工具类 CSS（`utilitycss`：Rust 写的运行时无关工具类
编译器）为本站定样式。

## 为什么 vendor

- **密封构建**：文档 CSS 永不依赖存活的上游。
- **固定语义**：`vendor/README.md` 中的 pin 指明每次构建用的
  确切上游 commit。
- **可审计 diff**：重新 vendor 就是普通的目录替换。

## 布局

```text
vendor/
  README.md                     # 来源、pin、更新流程
  tailwind-rs/                  # 纯净上游 + 已物化的 manifest
    crates/utilitycss-compiler  # ferrite-tailwind 用的 API
    crates/utilitycss-span      # SourceId
    ...                         # 上游 workspace 其余部分
```

`ferrite-tailwind` 链接 `utilitycss-compiler`、`utilitycss-span`、
`utilitycss-theme`；其余 crate 留作参考与未来的 extractor。

## 主题扩展

上游默认主题有意极简（pre-1.0）：少数色相只有 `500`，red 只有
`50`/`600`，gray 只有 `50`/`100`/`500`/`900`。Ferrite 通过公开的
`Theme::builder()` API 补上完整的 stone 与 orange 色阶以及 `6xl`
宽度（`crates/ferrite-tailwind` 的 `ferrite_theme()`）。不修改任何
vendored 源码。

少数工具类家族上游还没有，在 `site/src/docs.css` 的 compat 块中以
同名类补齐：margin `auto`、单边边框宽、`leading-6`、`shrink-0`、
`cursor-pointer`。每行都标明上游支持后即删除。

## 更新

```bash
rm -rf vendor/tailwind-rs
git clone https://github.com/nglmercer/tailwind-rs /tmp/tailwind-rs
cp -r /tmp/tailwind-rs vendor/tailwind-rs
rm -rf vendor/tailwind-rs/.git
cargo run -p xtask -- materialize-tailwind
# 在 vendor/README.md 记录新的 HEAD
cargo test -p ferrite-tailwind
```
