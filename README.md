# oral-arithmetic-generator

在 A4 纸张上生成可对齐打印的六年级口算与计算练习页，通过 LaTeX (`xelatex`) 直接输出 PDF。默认参数会生成 62 道题（A:24 / B:11 / C:11 / D:8 / E:8），刚好铺满 A4 正反两面。

## 前置依赖

需要系统安装 `xelatex`，通常随完整的 TeX 发行版一起提供：

- macOS: `brew install --cask mactex-no-gui`（或安装 [MacTeX](https://www.tug.org/mactex/)）
- Linux: 安装 TeX Live（`sudo apt install texlive-xetex texlive-lang-chinese` 等）
- Windows: 安装 [TeX Live](https://www.tug.org/texlive/) 或 [MiKTeX](https://miktex.org/)

可用 `xelatex --version` 验证安装。

## 用法

直接运行即可得到铺满 2 页 A4 的 `worksheet.pdf`：

```bash
cargo run -p oral-arithmetic-generator
```

需要附带参考答案页（会多输出 1 页答案）：

```bash
cargo run -p oral-arithmetic-generator -- --with-answers
```

自定义题量、页数、随机种子等：

```bash
cargo run -p oral-arithmetic-generator -- \
  --output worksheet.pdf \
  --target-pages 2 \
  --count-a 24 --count-b 11 --count-c 11 --count-d 8 --count-e 8 \
  --seed 20260518
```

程序会先生成 `.tex` 中间产物，再调用 `xelatex` 编译为 `worksheet.pdf`。排版用 `ctexart` + `geometry` + `multicol`，根据 `--target-pages` 在更宽松/标准/更紧凑布局之间自动切换。

加 `--keep-tex` 可同时保留与 PDF 同名的 `.tex` 源文件，便于排版微调或排错。

未显式传入 `--seed` 时，程序会默认使用当前系统 Unix 时间戳（秒）作为随机种子；如需复现同一份题目，请手动传入固定 `--seed`。

`--target-pages` 仅约束**题目区**的页数，不计入答案页；加 `--with-answers` 会在题目页之后追加 1 页参考答案。

## 题型覆盖

- A. 直接写得数 / 口算
- B. 脱式计算
- C. 简便计算
- D. 分数、小数、百分数互化与混合计算
- E. 比、求比值、化简比
