use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use oral_arithmetic_generator::{WorksheetConfig, generate_worksheet, render_latex};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut config = WorksheetConfig::default();
    let mut output = PathBuf::from("worksheet.pdf");
    let mut keep_tex = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--title" => config.title = next_value(&mut args, "--title")?,
            "--subtitle" => config.subtitle = next_value(&mut args, "--subtitle")?,
            "--output" => output = PathBuf::from(next_value(&mut args, "--output")?),
            "--seed" => config.seed = parse_value(&mut args, "--seed")?,
            "--columns" => config.columns = parse_value(&mut args, "--columns")?,
            "--target-pages" => config.target_pages = parse_value(&mut args, "--target-pages")?,
            "--count-a" => config.counts[0] = parse_value(&mut args, "--count-a")?,
            "--count-b" => config.counts[1] = parse_value(&mut args, "--count-b")?,
            "--count-c" => config.counts[2] = parse_value(&mut args, "--count-c")?,
            "--count-d" => config.counts[3] = parse_value(&mut args, "--count-d")?,
            "--count-e" => config.counts[4] = parse_value(&mut args, "--count-e")?,
            "--with-answers" => config.show_answers = true,
            "--keep-tex" => keep_tex = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => return Err(format!("未知参数：{other}\n使用 --help 查看可用选项。")),
        }
    }

    let worksheet = generate_worksheet(&config);
    let tex = render_latex(
        &worksheet,
        config.columns,
        config.target_pages,
        config.show_answers,
    );

    compile_pdf(&tex, &output, keep_tex)?;
    println!("已生成 {}", output.display());
    Ok(())
}

fn compile_pdf(tex: &str, out_pdf: &Path, keep_tex: bool) -> Result<(), String> {
    let work_dir = unique_work_dir()?;
    fs::create_dir_all(&work_dir)
        .map_err(|err| format!("创建临时目录失败 ({}): {err}", work_dir.display()))?;

    let tex_path = work_dir.join("worksheet.tex");
    fs::write(&tex_path, tex)
        .map_err(|err| format!("写入 .tex 文件失败 ({}): {err}", tex_path.display()))?;

    let status = Command::new("xelatex")
        .arg("-interaction=nonstopmode")
        .arg("-halt-on-error")
        .arg("-output-directory")
        .arg(&work_dir)
        .arg(&tex_path)
        .output();

    let output = match status {
        Ok(out) => out,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(
                "未找到 xelatex，请安装 MacTeX 或 TeX Live 后重试（macOS: brew install --cask mactex-no-gui）。"
                    .to_string(),
            );
        }
        Err(err) => return Err(format!("调用 xelatex 失败：{err}")),
    };

    if !output.status.success() {
        let log_path = work_dir.join("worksheet.log");
        let log_tail = fs::read_to_string(&log_path)
            .ok()
            .map(|text| extract_error_lines(&text))
            .unwrap_or_else(|| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .rev()
                    .take(40)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        return Err(format!(
            "xelatex 编译失败（保留 .tex 于 {}）：\n{log_tail}",
            tex_path.display()
        ));
    }

    let produced = work_dir.join("worksheet.pdf");
    if let Some(parent) = out_pdf.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建输出目录失败 ({}): {err}", parent.display()))?;
    }
    fs::copy(&produced, out_pdf).map_err(|err| {
        format!(
            "复制 PDF 到 {} 失败：{err}（产物位于 {}）",
            out_pdf.display(),
            produced.display()
        )
    })?;

    if keep_tex {
        let kept = out_pdf.with_extension("tex");
        fs::copy(&tex_path, &kept)
            .map_err(|err| format!("保留 .tex 到 {} 失败：{err}", kept.display()))?;
        println!("保留 .tex 于 {}", kept.display());
    }

    let _ = fs::remove_dir_all(&work_dir);
    Ok(())
}

fn unique_work_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    Ok(env::temp_dir().join(format!("oral-arithmetic-{pid}-{nanos}")))
}

fn extract_error_lines(log: &str) -> String {
    let lines: Vec<&str> = log.lines().collect();
    let mut excerpt: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with('!') || line.starts_with("l."))
        .collect();
    if excerpt.is_empty() {
        let start = lines.len().saturating_sub(40);
        excerpt = lines[start..].to_vec();
    }
    excerpt.join("\n")
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} 缺少对应的值"))
}

fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    let value = next_value(args, flag)?;
    value
        .parse::<T>()
        .map_err(|_| format!("{flag} 的值无效: {value}"))
}

fn print_help() {
    println!(
        "口算题生成器（LaTeX → PDF）

用法:
  cargo run -- --output worksheet.pdf [选项]

选项:
  --title <文本>         设置标题
  --subtitle <文本>      设置副标题
  --output <路径>        输出 PDF 文件路径（默认 worksheet.pdf）
  --seed <数字>          设置随机种子；未提供时默认使用当前系统时间戳
  --columns <数字>       题目列数，默认 2
  --target-pages <数字>  目标练习页数，默认 1
  --count-a <数字>       A 类题数量
  --count-b <数字>       B 类题数量
  --count-c <数字>       C 类题数量
  --count-d <数字>       D 类题数量
  --count-e <数字>       E 类题数量
  --with-answers         追加答案页
  --keep-tex             同时保留中间产物 .tex 文件
  --help, -h             显示帮助

依赖:
  需要系统安装 xelatex（MacTeX / TeX Live）。
  macOS:  brew install --cask mactex-no-gui
"
    );
}
