---
title: 资源
order: 7
---

# 资源

静态资源用 query 后缀（`ferrite-assets`，spec §30）。
后缀决定表示形式；流水线据此哈希、内联或生成 shim。

## Query

```js
import url from "./logo.svg";        // 哈希 URL（dev 路径 / 构建哈希）
import text from "./doc.txt?raw";    // 文件文本作字符串
import url2 from "./a.bin?url";      // 永远 URL，永不内联
import data from "./icon.svg?inline";// 永远 data: URL
import Worker from "./w.js?worker";  // 作为 worker 入口构建
import wasm from "./add.wasm?wasm";  // 作为 WASM 入口构建
```

| 后缀 | Dev | 构建 |
|---|---|---|
| （无） | serve 路径 | 内容哈希文件（`logo.4ad83f.svg`） |
| `?raw` | 内联文本 | 内联文本 |
| `?url` | serve 路径 | 哈希文件 URL |
| `?inline` | data: URL | data: URL |
| `?worker` | worker shim | 独立 worker chunk |
| `?wasm` | WASM loader | 哈希 WASM + glue（见 [WASM](wasm)） |

## 导入如何解析

资源导入重写为 `…?asset-shim`：导出 URL（或文本/data）的极小
JS 模块。请求普通 URL 则以内容推导的 MIME serve 原始字节。
于是 `import url` 在 dev 原生 ESM 与构建哈希输出下都工作，无需
loader 配置。

## 哈希与缓存

构建输出文件名嵌入内容哈希，资源不可变、可永久缓存。
[清单](pipeline)把源码 id 映射到哈希输出；HTML
`<link>`/`<script>` 与 CSS `url()` 引用同步重写。

## Worker

`?worker` 入口作为独立 chunk 构建，自带导入闭包。
导入得到构造器/URL，传给
`new Worker(url, { type: "module" })`。共享依赖与主入口在同一
chunk 图中去重。
