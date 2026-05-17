# oral-arithmetic-generator

在 A4 纸张上生成可对齐打印的六年级口算与计算练习页，通过 LaTeX (`xelatex`) 直接输出 PDF。

## 前置依赖

需要系统安装 `xelatex`，通常随完整的 TeX 发行版一起提供：

- macOS: `brew install --cask mactex-no-gui`（或安装 [MacTeX](https://www.tug.org/mactex/)）
- Linux: 安装 TeX Live（`sudo apt install texlive-xetex texlive-lang-chinese` 等）
- Windows: 安装 [TeX Live](https://www.tug.org/texlive/) 或 [MiKTeX](https://miktex.org/)

可用 `xelatex --version` 验证安装。

## 用法

```bash
cargo run -p oral-arithmetic-generator -- \
  --output worksheet.pdf \
  --target-pages 1 \
  --count-a 24 \
  --count-b 12 \
  --count-c 12 \
  --count-d 10 \
  --count-e 10 \
  --with-answers
```

程序会先生成 `.tex` 中间产物，再调用 `xelatex` 编译为 `worksheet.pdf`。CSS 已替换为 LaTeX 的 `ctexart` + `geometry` + `multicol` 排版，根据 `--target-pages` 在更宽松/标准/更紧凑布局之间自动切换。

加 `--keep-tex` 可同时保留与 PDF 同名的 `.tex` 源文件，便于排版微调或排错。

未显式传入 `--seed` 时，程序会默认使用当前系统 Unix 时间戳（秒）作为随机种子；如需复现同一份题目，请手动传入固定 `--seed`。

## 题型覆盖

- A. 直接写得数 / 口算
- B. 脱式计算
- C. 简便计算
- D. 分数、小数、百分数互化与混合计算
- E. 比、求比值、化简比
