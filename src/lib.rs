use std::collections::HashSet;
use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionKind {
    Direct,
    Vertical,
    Simplified,
    Conversion,
    Ratio,
}

impl SectionKind {
    pub fn title(self) -> &'static str {
        match self {
            SectionKind::Direct => "A. 直接写得数 / 口算",
            SectionKind::Vertical => "B. 脱式计算",
            SectionKind::Simplified => "C. 简便计算",
            SectionKind::Conversion => "D. 分数、小数、百分数互化与混合计算",
            SectionKind::Ratio => "E. 比、求比值、化简比",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub prompt: String,
    pub answer: String,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub problems: Vec<Problem>,
}

#[derive(Clone, Debug)]
pub struct Worksheet {
    pub title: String,
    pub subtitle: String,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug)]
pub struct WorksheetConfig {
    pub title: String,
    pub subtitle: String,
    pub counts: [usize; 5],
    pub columns: usize,
    pub target_pages: usize,
    pub seed: u64,
    pub show_answers: bool,
}

impl Default for WorksheetConfig {
    fn default() -> Self {
        Self {
            title: "六年级口算与计算练习".to_string(),
            subtitle: "姓名：__________    班级：__________    日期：__________".to_string(),
            counts: [24, 11, 11, 8, 8],
            columns: 2,
            target_pages: 2,
            seed: default_seed(),
            show_answers: false,
        }
    }
}

fn default_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(1)
}

/// How many times to retry a duplicate prompt before accepting it.
///
/// Pools per template are bounded (see the `generate_*_problem` arms), so a
/// hard cap keeps generation O(count) even when a section's distinct-prompt
/// space is genuinely smaller than the requested count.
const DEDUP_MAX_ATTEMPTS: usize = 64;

pub fn generate_worksheet(config: &WorksheetConfig) -> Worksheet {
    let mut rng = SimpleRng::new(config.seed);
    let section_kinds = [
        SectionKind::Direct,
        SectionKind::Vertical,
        SectionKind::Simplified,
        SectionKind::Conversion,
        SectionKind::Ratio,
    ];
    let mut sections = Vec::new();

    for (index, kind) in section_kinds.into_iter().enumerate() {
        let count = config.counts[index];
        if count == 0 {
            continue;
        }
        let mut problems = Vec::with_capacity(count);
        let mut seen: HashSet<String> = HashSet::with_capacity(count);

        while problems.len() < count {
            let mut chosen: Option<Problem> = None;
            for _ in 0..DEDUP_MAX_ATTEMPTS {
                let candidate = match kind {
                    SectionKind::Direct => generate_direct_problem(&mut rng),
                    SectionKind::Vertical => generate_vertical_problem(&mut rng),
                    SectionKind::Simplified => generate_simplified_problem(&mut rng),
                    SectionKind::Conversion => generate_conversion_problem(&mut rng),
                    SectionKind::Ratio => generate_ratio_problem(&mut rng),
                };
                if !seen.contains(&candidate.prompt) {
                    chosen = Some(candidate);
                    break;
                }
            }
            let problem = chosen.unwrap_or_else(|| match kind {
                SectionKind::Direct => generate_direct_problem(&mut rng),
                SectionKind::Vertical => generate_vertical_problem(&mut rng),
                SectionKind::Simplified => generate_simplified_problem(&mut rng),
                SectionKind::Conversion => generate_conversion_problem(&mut rng),
                SectionKind::Ratio => generate_ratio_problem(&mut rng),
            });
            seen.insert(problem.prompt.clone());
            problems.push(problem);
        }
        sections.push(Section { kind, problems });
    }

    Worksheet {
        title: config.title.clone(),
        subtitle: config.subtitle.clone(),
        sections,
    }
}

pub fn render_latex(
    worksheet: &Worksheet,
    columns: usize,
    target_pages: usize,
    show_answers: bool,
) -> String {
    let safe_columns = columns.max(1);
    let safe_target_pages = target_pages.max(1);
    let density = choose_density(worksheet, safe_columns, safe_target_pages);

    let font_size_pt = match density {
        DensityMode::Compact => 10,
        _ => 11,
    };

    let mut tex = String::new();
    let _ = writeln!(
        tex,
        "\\documentclass[UTF8,a4paper,{font_size_pt}pt]{{ctexart}}"
    );
    tex.push_str(
        "\\usepackage[margin=12mm]{geometry}\n\
         \\usepackage{multicol}\n\
         \\usepackage{amsmath}\n\
         \\usepackage{enumitem}\n\
         \\pagestyle{empty}\n\
         \\setlength{\\parindent}{0pt}\n\
         \\setlength{\\columnsep}{6mm}\n",
    );
    if density == DensityMode::Compact {
        tex.push_str("\\linespread{0.95}\n");
    }
    tex.push_str("\\begin{document}\n\n");

    let _ = writeln!(
        tex,
        "{{\\Large\\bfseries {}\\par}}",
        escape_latex(&worksheet.title)
    );
    tex.push_str("\\vspace{3mm}\n");
    let subtitle_lines: Vec<String> = worksheet.subtitle.split('\n').map(escape_latex).collect();
    tex.push_str(&subtitle_lines.join("\\\\\n"));
    tex.push_str("\\par\n\\vspace{6mm}\n\n");

    for section in &worksheet.sections {
        let layout = section_layout(section, safe_columns, density);
        render_section_latex(&mut tex, section, &layout, false);
    }

    if show_answers {
        tex.push_str("\\newpage\n\\section*{参考答案}\n\n");
        for section in &worksheet.sections {
            let mut layout = section_layout(section, safe_columns, density);
            layout.blank_lines = 0;
            layout.itemsep_mm = 1;
            render_section_latex(&mut tex, section, &layout, true);
        }
    }

    tex.push_str("\\end{document}\n");
    tex
}

fn render_section_latex(
    tex: &mut String,
    section: &Section,
    layout: &SectionLayout,
    is_answer: bool,
) {
    let heading = if is_answer {
        format!("\\subsection*{{{}}}\n", escape_latex(section.kind.title()))
    } else {
        format!("\\section*{{{}}}\n", escape_latex(section.kind.title()))
    };
    tex.push_str(&heading);

    let use_multicols = layout.columns > 1;
    if use_multicols {
        let _ = writeln!(tex, "\\begin{{multicols}}{{{}}}", layout.columns);
    }
    let _ = writeln!(
        tex,
        "\\begin{{enumerate}}[leftmargin=*, itemsep={}mm, parsep=0pt, topsep=0pt]",
        layout.itemsep_mm
    );

    for problem in &section.problems {
        if is_answer {
            let prompt = render_math_latex(answer_prompt(&problem.prompt));
            let answer = render_math_latex(&problem.answer);
            let _ = writeln!(tex, "\\item {prompt} = {answer}");
        } else {
            let prompt = render_math_latex(&problem.prompt);
            let _ = write!(tex, "\\item {prompt}");
            if layout.blank_lines > 0 {
                tex.push_str("\\\\[2mm]\n");
                for k in 0..layout.blank_lines {
                    tex.push_str("\\rule{\\linewidth}{0.4pt}");
                    if k + 1 < layout.blank_lines {
                        tex.push_str("\\\\[2mm]\n");
                    } else {
                        tex.push('\n');
                    }
                }
            } else {
                tex.push('\n');
            }
        }
    }

    tex.push_str("\\end{enumerate}\n");
    if use_multicols {
        tex.push_str("\\end{multicols}\n");
    }
    tex.push('\n');
}

fn generate_direct_problem(rng: &mut SimpleRng) -> Problem {
    match rng.gen_range(0, 6) {
        0 => {
            let denominator = [5, 8, 10][rng.gen_range(0, 3)];
            let left = rng.gen_range(1, denominator);
            let right = rng.gen_range(1, denominator - left + 1);
            let result = Fraction::new(left as i64 + right as i64, denominator as i64);
            Problem {
                prompt: format!("{}/{} + {}/{} = ", left, denominator, right, denominator),
                answer: result.to_string(),
            }
        }
        1 => {
            let decimal = [0.2, 0.25, 0.4, 0.5, 1.25][rng.gen_range(0, 5)];
            let integer = [4, 8, 20, 40, 80][rng.gen_range(0, 5)];
            Problem {
                prompt: format!("{} × {} = ", format_decimal(decimal), integer),
                answer: format_decimal(decimal * integer as f64),
            }
        }
        2 => {
            let percent = [10, 20, 25, 35, 40, 50][rng.gen_range(0, 6)];
            let integer = [40, 80, 120, 160, 200][rng.gen_range(0, 5)];
            Problem {
                prompt: format!("{}% × {} = ", percent, integer),
                answer: ((percent * integer) / 100).to_string(),
            }
        }
        3 => {
            let a = [1.2, 2.4, 3.6, 4.8][rng.gen_range(0, 4)];
            let b = [0.3, 0.4, 0.6, 0.8][rng.gen_range(0, 4)];
            Problem {
                prompt: format!("{} ÷ {} = ", format_decimal(a), format_decimal(b)),
                answer: format_decimal(a / b),
            }
        }
        4 => {
            let total = [90, 120, 150, 180][rng.gen_range(0, 4)];
            let part = [10, 15, 20, 25, 30][rng.gen_range(0, 5)];
            Problem {
                prompt: format!("{} - {}% × {} = ", total, part, total),
                answer: (total - (part * total) / 100).to_string(),
            }
        }
        _ => {
            // Pick proper-fraction operands with a common denominator and a strictly
            // larger left numerator. Format the prompt directly with the chosen
            // numerator/denominator so Fraction::new's reduction (which would
            // collapse e.g. 5/5 to "1") never bleeds into the prompt text.
            let denominator = [4, 5, 6, 8, 10, 12][rng.gen_range(0, 6)] as i64;
            let left_num = rng.gen_range(2, denominator as usize) as i64;
            let right_num = rng.gen_range(1, left_num as usize) as i64;
            let result = Fraction::new(left_num - right_num, denominator);
            Problem {
                prompt: format!(
                    "{}/{} - {}/{} = ",
                    left_num, denominator, right_num, denominator
                ),
                answer: result.to_string(),
            }
        }
    }
}

fn generate_vertical_problem(rng: &mut SimpleRng) -> Problem {
    match rng.gen_range(0, 5) {
        0 => {
            let a = [2.5, 3.6, 4.8, 7.2][rng.gen_range(0, 4)];
            let b = [4.0, 2.5, 1.25, 0.75][rng.gen_range(0, 4)];
            let c = [3.6, 2.4, 1.8, 0.9][rng.gen_range(0, 4)];
            let d = [0.9, 0.6, 0.3][rng.gen_range(0, 3)];
            Problem {
                prompt: format!(
                    "{} × {} - {} ÷ {} = ",
                    format_decimal(a),
                    format_decimal(b),
                    format_decimal(c),
                    format_decimal(d)
                ),
                answer: format_decimal(a * b - c / d),
            }
        }
        1 => {
            let left = Fraction::new([1, 3, 5][rng.gen_range(0, 3)] as i64, 4);
            // Odd numerators so Fraction::new(_, 2) can't reduce to "1" in prompt.
            let mid = Fraction::new([1, 3, 5][rng.gen_range(0, 3)] as i64, 2);
            let right = Fraction::new([1, 2][rng.gen_range(0, 2)] as i64, 3);
            let answer = left + mid * right;
            Problem {
                prompt: format!("{} + {} × {} = ", left, mid, right),
                answer: answer.to_string(),
            }
        }
        2 => {
            let percent = [40, 60, 80][rng.gen_range(0, 3)] as f64 / 100.0;
            let fraction = Fraction::new([3, 5, 7][rng.gen_range(0, 3)] as i64, 8);
            let decimal = [0.8, 1.2, 1.5][rng.gen_range(0, 3)];
            let answer = percent * fraction.to_f64() + decimal;
            Problem {
                prompt: format!(
                    "{}% × {} + {} = ",
                    format_decimal(percent * 100.0),
                    fraction,
                    format_decimal(decimal)
                ),
                answer: format_decimal(answer),
            }
        }
        3 => {
            // Build (a, b, c) so that c divides (a + b) exactly. The legacy form
            // sampled a, b, c independently and used integer division, which
            // silently truncated remainders (e.g. (24+3)÷8×3 → 9 instead of
            // 10.125). Sampling parametrically keeps integer arithmetic exact.
            let c = [2_i32, 3, 4, 5, 6][rng.gen_range(0, 5)];
            let q = rng.gen_range(3, 9) as i32;
            let total = c * q;
            let max_b = ((total - 1) / 2).max(1);
            let b = rng.gen_range(1, max_b as usize + 1) as i32;
            let a = total - b;
            let d = [2_i32, 3, 4, 5][rng.gen_range(0, 4)];
            Problem {
                prompt: format!("({} + {}) ÷ {} × {} = ", a, b, c, d),
                answer: (q * d).to_string(),
            }
        }
        _ => {
            let left = Fraction::new([3, 5, 7][rng.gen_range(0, 3)] as i64, 6);
            let right = Fraction::new([1, 2, 3][rng.gen_range(0, 3)] as i64, 3);
            let sub = Fraction::new(1, 2);
            let answer = (left + right) / sub;
            Problem {
                prompt: format!("({} + {}) ÷ {} = ", left, right, sub),
                answer: answer.to_string(),
            }
        }
    }
}

fn generate_simplified_problem(rng: &mut SimpleRng) -> Problem {
    match rng.gen_range(0, 5) {
        0 => {
            let mid = [3, 4, 6, 7, 8, 9, 11, 13, 16, 19, 24, 32][rng.gen_range(0, 12)];
            let answer = 25 * mid * 4;
            Problem {
                prompt: format!("25 × {} × 4 = ", mid),
                answer: answer.to_string(),
            }
        }
        1 => {
            let value = [1.7, 2.5, 3.7, 4.8][rng.gen_range(0, 4)];
            Problem {
                prompt: format!("{} × 9.9 = ", format_decimal(value)),
                answer: format_decimal(value * 9.9),
            }
        }
        2 => {
            let a = Fraction::new([3, 5, 7][rng.gen_range(0, 3)] as i64, 8);
            let b = Fraction::new([1, 2, 3][rng.gen_range(0, 3)] as i64, 8);
            let c = Fraction::new(1, 8);
            Problem {
                prompt: format!("{} + {} - {} = ", a, b, c),
                answer: (a + b - c).to_string(),
            }
        }
        3 => {
            let value = [2.4, 3.6, 4.8, 6.5][rng.gen_range(0, 4)];
            Problem {
                prompt: format!("{} × 101 = ", format_decimal(value)),
                answer: format_decimal(value * 101.0),
            }
        }
        _ => {
            let value = [
                12.5, 14.5, 17.5, 22.5, 24.5, 27.5, 32.5, 36.5, 42.5, 52.5, 62.5, 72.5,
            ][rng.gen_range(0, 12)];
            Problem {
                prompt: format!("{} + 7.5 = ", format_decimal(value)),
                answer: format_decimal(value + 7.5),
            }
        }
    }
}

fn generate_conversion_problem(rng: &mut SimpleRng) -> Problem {
    match rng.gen_range(0, 5) {
        0 => {
            let decimal = [0.125, 0.25, 0.375, 0.625][rng.gen_range(0, 4)];
            let fraction = Fraction::from_decimal(decimal);
            Problem {
                prompt: format!("{} = (   ) / (   )", format_decimal(decimal)),
                answer: fraction.to_string(),
            }
        }
        1 => {
            let fraction = Fraction::new([1, 3, 7, 9][rng.gen_range(0, 4)] as i64, 20);
            Problem {
                prompt: format!("{} = (   )%", fraction),
                answer: format_decimal(fraction.to_f64() * 100.0),
            }
        }
        2 => {
            let percent = [12.5, 25.0, 45.0, 60.0, 75.0][rng.gen_range(0, 5)];
            Problem {
                prompt: format!("{}% = (     )", format_decimal(percent)),
                answer: format_decimal(percent / 100.0),
            }
        }
        3 => {
            let items = [
                ("3/4, 0.76, 78%", "3/4 < 0.76 < 78%"),
                ("2/5, 0.38, 41%", "0.38 < 2/5 < 41%"),
                ("5/8, 0.6, 63%", "0.6 < 5/8 < 63%"),
                ("1/2, 0.45, 55%", "0.45 < 1/2 < 55%"),
                ("7/10, 0.65, 72%", "0.65 < 7/10 < 72%"),
                ("4/5, 0.78, 82%", "0.78 < 4/5 < 82%"),
                ("3/8, 0.4, 36%", "36% < 3/8 < 0.4"),
                ("9/20, 0.5, 48%", "9/20 < 48% < 0.5"),
            ];
            let (prompt, answer) = items[rng.gen_range(0, items.len())];
            Problem {
                prompt: format!("比较大小：{}", prompt),
                answer: answer.to_string(),
            }
        }
        _ => {
            let percent = [20, 25, 40, 75][rng.gen_range(0, 4)];
            let whole = [120, 160, 200][rng.gen_range(0, 3)];
            Problem {
                prompt: format!("{}% × {} = ", percent, whole),
                answer: ((percent * whole) / 100).to_string(),
            }
        }
    }
}

fn generate_ratio_problem(rng: &mut SimpleRng) -> Problem {
    match rng.gen_range(0, 5) {
        0 => {
            let left = [12, 15, 18, 24][rng.gen_range(0, 4)] as i64;
            let right = [18, 20, 27, 36][rng.gen_range(0, 4)] as i64;
            let reduced = Fraction::new(left, right);
            Problem {
                prompt: format!("{} : {} 化简比", left, right),
                answer: format!("{}:{}", reduced.numerator, reduced.denominator),
            }
        }
        1 => {
            let pairs: [(f64, Fraction); 6] = [
                (0.6, Fraction::new(3, 5)),
                (0.5, Fraction::new(1, 4)),
                (0.75, Fraction::new(3, 8)),
                (0.4, Fraction::new(1, 5)),
                (0.3, Fraction::new(3, 5)),
                (0.8, Fraction::new(2, 5)),
            ];
            let (left, right) = pairs[rng.gen_range(0, pairs.len())];
            let ratio = left / right.to_f64();
            Problem {
                prompt: format!("{} : {} 求比值", format_decimal(left), right),
                answer: format_decimal(ratio),
            }
        }
        2 => {
            let percent = [20, 25, 40, 75][rng.gen_range(0, 4)] as i64;
            let reduced = Fraction::new(percent, 100);
            Problem {
                prompt: format!("把 {}% 写成最简整数比", percent),
                answer: format!("{}:{}", reduced.numerator, reduced.denominator),
            }
        }
        3 => {
            let left = [1.5, 2.4, 3.6][rng.gen_range(0, 3)];
            let right = [0.5, 0.6, 1.2][rng.gen_range(0, 3)];
            Problem {
                prompt: format!(
                    "{} : {} 求比值",
                    format_decimal(left),
                    format_decimal(right)
                ),
                answer: format_decimal(left / right),
            }
        }
        _ => {
            let left = Fraction::new([1, 2, 3][rng.gen_range(0, 3)] as i64, 4);
            let right = [25, 50, 75][rng.gen_range(0, 3)] as i64;
            let ratio = Fraction::new(left.numerator * 100, left.denominator * right);
            Problem {
                prompt: format!("{} : {}% 化成最简整数比", left, right),
                answer: format!("{}:{}", ratio.numerator, ratio.denominator),
            }
        }
    }
}

fn push_latex_char(out: &mut String, ch: char) {
    match ch {
        '\\' => out.push_str("\\textbackslash{}"),
        '{' => out.push_str("\\{"),
        '}' => out.push_str("\\}"),
        '$' => out.push_str("\\$"),
        '&' => out.push_str("\\&"),
        '#' => out.push_str("\\#"),
        '_' => out.push_str("\\_"),
        '%' => out.push_str("\\%"),
        '^' => out.push_str("\\textasciicircum{}"),
        '~' => out.push_str("\\textasciitilde{}"),
        '<' => out.push_str("\\textless{}"),
        '>' => out.push_str("\\textgreater{}"),
        _ => out.push(ch),
    }
}

fn escape_latex(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        push_latex_char(&mut out, ch);
    }
    out
}

fn render_math_latex(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut index = 0;

    while index < chars.len() {
        if let Some((next_index, numerator, denominator)) = parse_fraction_token(&chars, index) {
            let _ = write!(out, "$\\frac{{{numerator}}}{{{denominator}}}$");
            index = next_index;
            continue;
        }

        match chars[index] {
            '×' => out.push_str("$\\times$"),
            '÷' => out.push_str("$\\div$"),
            other => push_latex_char(&mut out, other),
        }
        index += 1;
    }

    out
}

fn parse_fraction_token(chars: &[char], start: usize) -> Option<(usize, String, String)> {
    if start >= chars.len() {
        return None;
    }

    let mut cursor = start;
    if chars[cursor] == '-' {
        cursor += 1;
    }

    let digits_start = cursor;
    while cursor < chars.len() && chars[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == digits_start || cursor >= chars.len() || chars[cursor] != '/' {
        return None;
    }

    let slash_index = cursor;
    cursor += 1;
    let denominator_start = cursor;
    while cursor < chars.len() && chars[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == denominator_start {
        return None;
    }

    if start > 0 {
        let prev = chars[start - 1];
        if prev.is_ascii_digit() || prev.is_ascii_alphabetic() {
            return None;
        }
    }

    if cursor < chars.len() {
        let next = chars[cursor];
        if next.is_ascii_digit() || next.is_ascii_alphabetic() {
            return None;
        }
    }

    let numerator: String = chars[start..slash_index].iter().collect();
    let denominator: String = chars[denominator_start..cursor].iter().collect();
    Some((cursor, numerator, denominator))
}

fn answer_prompt(prompt: &str) -> &str {
    prompt
        .trim()
        .trim_end_matches('=')
        .trim()
        .trim_end_matches('?')
        .trim()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SectionLayout {
    columns: usize,
    blank_lines: usize,
    itemsep_mm: usize,
}

const HEADER_OVERHEAD_MM: usize = 25;
const SECTION_OVERHEAD_MM: usize = 30;
const PROMPT_LINE_MM: usize = 7;
const RULE_STEP_MM: usize = 4;
const PAGE_BUDGET_MM: usize = 240;

fn item_height_mm(blank_lines: usize) -> usize {
    let lead_to_first_rule = if blank_lines > 0 { 2 } else { 0 };
    PROMPT_LINE_MM + lead_to_first_rule + blank_lines * RULE_STEP_MM
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DensityMode {
    Relaxed,
    Standard,
    Compact,
}

fn density_itemsep_mm(density: DensityMode) -> usize {
    match density {
        DensityMode::Relaxed => 3,
        DensityMode::Standard => 2,
        DensityMode::Compact => 1,
    }
}

fn section_layout(section: &Section, max_columns: usize, density: DensityMode) -> SectionLayout {
    let max_prompt_len = section
        .problems
        .iter()
        .map(|problem| answer_prompt(&problem.prompt).chars().count())
        .max()
        .unwrap_or(0);

    let itemsep_mm = density_itemsep_mm(density);

    match section.kind {
        SectionKind::Direct => SectionLayout {
            columns: match density {
                DensityMode::Relaxed => max_columns.clamp(1, 3),
                DensityMode::Standard | DensityMode::Compact => max_columns.clamp(1, 4),
            },
            blank_lines: 0,
            itemsep_mm,
        },
        SectionKind::Vertical => {
            let columns = match density {
                DensityMode::Relaxed => 1,
                DensityMode::Standard => {
                    if max_columns >= 2 && max_prompt_len <= 18 {
                        2
                    } else {
                        1
                    }
                }
                DensityMode::Compact => {
                    if max_columns >= 2 && max_prompt_len <= 24 {
                        2
                    } else {
                        1
                    }
                }
            };
            SectionLayout {
                columns,
                blank_lines: match density {
                    DensityMode::Relaxed => 4,
                    DensityMode::Standard => {
                        if columns == 2 {
                            3
                        } else {
                            4
                        }
                    }
                    DensityMode::Compact => 2,
                },
                itemsep_mm,
            }
        }
        SectionKind::Simplified | SectionKind::Ratio => {
            let columns = match density {
                DensityMode::Relaxed => 1,
                DensityMode::Standard | DensityMode::Compact => max_columns.clamp(1, 2),
            };
            SectionLayout {
                columns,
                blank_lines: match density {
                    DensityMode::Relaxed => 4,
                    DensityMode::Standard => 3,
                    DensityMode::Compact => 2,
                },
                itemsep_mm,
            }
        }
        SectionKind::Conversion => {
            let has_compare = section
                .problems
                .iter()
                .any(|problem| problem.prompt.contains("比较大小"));
            let columns = match density {
                DensityMode::Relaxed => 1,
                DensityMode::Standard => {
                    if max_columns >= 2 && max_prompt_len <= 16 && !has_compare {
                        2
                    } else {
                        1
                    }
                }
                DensityMode::Compact => {
                    if max_columns >= 2 && max_prompt_len <= 20 && !has_compare {
                        2
                    } else {
                        1
                    }
                }
            };
            SectionLayout {
                columns,
                blank_lines: match density {
                    DensityMode::Relaxed => 4,
                    DensityMode::Standard => 3,
                    DensityMode::Compact => 2,
                },
                itemsep_mm,
            }
        }
    }
}

fn choose_density(worksheet: &Worksheet, max_columns: usize, target_pages: usize) -> DensityMode {
    let budget_mm = target_pages * PAGE_BUDGET_MM;
    let relaxed = estimate_height_mm(worksheet, max_columns, DensityMode::Relaxed);
    if relaxed <= budget_mm {
        return DensityMode::Relaxed;
    }

    let standard = estimate_height_mm(worksheet, max_columns, DensityMode::Standard);
    if standard <= budget_mm {
        return DensityMode::Standard;
    }

    DensityMode::Compact
}

fn estimate_height_mm(worksheet: &Worksheet, max_columns: usize, density: DensityMode) -> usize {
    let mut total = HEADER_OVERHEAD_MM;
    for section in &worksheet.sections {
        let layout = section_layout(section, max_columns, density);
        let rows = section.problems.len().div_ceil(layout.columns.max(1));
        if rows == 0 {
            continue;
        }
        total += SECTION_OVERHEAD_MM;
        total += rows * item_height_mm(layout.blank_lines);
        total += rows.saturating_sub(1) * layout.itemsep_mm;
    }
    total
}

fn format_decimal(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let mut text = format!("{rounded:.3}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fraction {
    numerator: i64,
    denominator: i64,
}

impl Fraction {
    fn new(numerator: i64, denominator: i64) -> Self {
        assert!(denominator != 0);
        let mut numerator = numerator;
        let mut denominator = denominator;
        if denominator < 0 {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = gcd(numerator.unsigned_abs(), denominator as u64) as i64;
        Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        }
    }

    fn from_decimal(value: f64) -> Self {
        let text = format_decimal(value);
        if let Some((int_part, frac_part)) = text.split_once('.') {
            let scale = 10_i64.pow(frac_part.len() as u32);
            let sign = if int_part.starts_with('-') { -1 } else { 1 };
            let int_abs = int_part.trim_start_matches('-').parse::<i64>().unwrap_or(0);
            let frac_abs = frac_part.parse::<i64>().unwrap_or(0);
            let numerator = sign * (int_abs * scale + frac_abs);
            Self::new(numerator, scale)
        } else {
            Self::new(text.parse::<i64>().unwrap_or(0), 1)
        }
    }

    fn to_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl std::fmt::Display for Fraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.denominator == 1 {
            write!(f, "{}", self.numerator)
        } else {
            write!(f, "{}/{}", self.numerator, self.denominator)
        }
    }
}

impl std::ops::Add for Fraction {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numerator * rhs.denominator + rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }
}

impl std::ops::Sub for Fraction {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numerator * rhs.denominator - rhs.numerator * self.denominator,
            self.denominator * rhs.denominator,
        )
    }
}

impl std::ops::Mul for Fraction {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numerator * rhs.numerator,
            self.denominator * rhs.denominator,
        )
    }
}

impl std::ops::Div for Fraction {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self::new(
            self.numerator * rhs.denominator,
            self.denominator * rhs.numerator,
        )
    }
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let rem = left % right;
        left = right;
        right = rem;
    }
    left.max(1)
}

#[derive(Clone, Debug)]
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.state >> 32) as u32
    }

    fn gen_range(&mut self, start: usize, end: usize) -> usize {
        assert!(start < end);
        let width = end - start;
        start + (self.next_u32() as usize % width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn direct_subtractions_avoid_degenerate_answers() {
        // Catches two related bugs in the Direct "_" arm:
        //   - Fraction::new(N, N) collapsing to "1" caused negative results
        //     (e.g. "1 - 2 = -1" came from 5/5 - 2/5).
        //   - Same numerator on both sides yielded "X - X = 0".
        // Skip percentage-style subtractions ("180 - 30% × 180") which legitimately
        // use the minus sign without belonging to that arm.
        for seed in [1_u64, 42, 20260330, 0xCAFE_BABE_u64, 0xDEAD_BEEF_u64] {
            let config = WorksheetConfig {
                seed,
                ..WorksheetConfig::default()
            };
            let worksheet = generate_worksheet(&config);
            for section in &worksheet.sections {
                if section.kind != SectionKind::Direct {
                    continue;
                }
                for problem in &section.problems {
                    if problem.prompt.contains('%') || !problem.prompt.contains(" - ") {
                        continue;
                    }
                    assert!(
                        !problem.answer.starts_with('-'),
                        "seed {seed}: negative answer in Direct subtraction: {} = {}",
                        problem.prompt,
                        problem.answer
                    );
                    assert_ne!(
                        problem.answer, "0",
                        "seed {seed}: zero answer in Direct subtraction: {} = {}",
                        problem.prompt, problem.answer
                    );
                }
            }
        }
    }

    #[test]
    fn vertical_paren_div_mul_division_is_exact() {
        // Catches the (a+b) ÷ c × d arm using integer truncation. We force a
        // vertical-only worksheet with enough problems to hit the arm repeatedly
        // and parse the prompt back to verify exactness.
        for seed in [1_u64, 42, 20260330, 0xCAFE_BABE_u64] {
            let config = WorksheetConfig {
                seed,
                counts: [0, 30, 0, 0, 0],
                ..WorksheetConfig::default()
            };
            let worksheet = generate_worksheet(&config);
            for section in &worksheet.sections {
                for problem in &section.problems {
                    let trimmed = problem.prompt.trim_end_matches('=').trim();
                    let Some(rest) = trimmed.strip_prefix('(') else {
                        continue;
                    };
                    let Some((a_str, rest)) = rest.split_once(" + ") else {
                        continue;
                    };
                    let Some((b_str, rest)) = rest.split_once(") ÷ ") else {
                        continue;
                    };
                    let Some((c_str, d_str)) = rest.split_once(" × ") else {
                        continue;
                    };
                    let (Ok(a), Ok(b), Ok(c), Ok(d)) = (
                        a_str.parse::<i64>(),
                        b_str.parse::<i64>(),
                        c_str.parse::<i64>(),
                        d_str.parse::<i64>(),
                    ) else {
                        continue;
                    };
                    assert_eq!(
                        (a + b) % c,
                        0,
                        "seed {seed}: non-exact division in {} = {}",
                        problem.prompt,
                        problem.answer
                    );
                    let expected = (a + b) / c * d;
                    let actual: i64 = problem.answer.parse().unwrap_or(i64::MIN);
                    assert_eq!(
                        expected, actual,
                        "seed {seed}: wrong answer for {} (expected {expected})",
                        problem.prompt
                    );
                }
            }
        }
    }

    #[test]
    fn worksheet_problems_are_unique_within_each_section() {
        // Default counts (24/11/11/8/8) exercise every section's pool size.
        // Several deterministic seeds catch RNG-specific collision patterns.
        for seed in [1_u64, 20260330, 99_999, 0xDEAD_BEEF] {
            let config = WorksheetConfig {
                seed,
                ..WorksheetConfig::default()
            };
            let worksheet = generate_worksheet(&config);
            for section in &worksheet.sections {
                let mut seen = HashSet::new();
                for problem in &section.problems {
                    assert!(
                        seen.insert(problem.prompt.clone()),
                        "duplicate prompt in section {:?} (seed {}): {}",
                        section.kind,
                        seed,
                        problem.prompt,
                    );
                }
            }
        }
    }

    #[test]
    fn worksheet_generation_respects_zero_counts() {
        let config = WorksheetConfig {
            counts: [2, 0, 1, 0, 1],
            seed: 20260330,
            ..WorksheetConfig::default()
        };
        let worksheet = generate_worksheet(&config);
        assert_eq!(worksheet.sections.len(), 3);
        assert_eq!(worksheet.sections[0].problems.len(), 2);
        assert_eq!(worksheet.sections[1].kind, SectionKind::Simplified);
        assert_eq!(worksheet.sections[2].kind, SectionKind::Ratio);
    }

    #[test]
    fn render_latex_contains_ctex_and_answers() {
        let worksheet = Worksheet {
            title: "test".to_string(),
            subtitle: "sub".to_string(),
            sections: vec![Section {
                kind: SectionKind::Direct,
                problems: vec![Problem {
                    prompt: "3/5 + 1/5 = ".to_string(),
                    answer: "4/5".to_string(),
                }],
            }],
        };
        let tex = render_latex(&worksheet, 2, 1, true);
        assert!(tex.contains("\\documentclass[UTF8,a4paper"));
        assert!(tex.contains("ctexart"));
        assert!(tex.contains("\\begin{document}"));
        assert!(tex.contains("\\end{document}"));
        assert!(tex.contains("参考答案"));
        assert!(tex.contains("\\frac{3}{5}"));
        assert!(tex.contains("\\frac{1}{5}"));
        assert!(tex.contains("\\frac{4}{5}"));
    }

    #[test]
    fn answer_prompt_removes_trailing_operators() {
        assert_eq!(answer_prompt("3/5 + 1/5 = "), "3/5 + 1/5");
        assert_eq!(answer_prompt("0.375 = 几分之几"), "0.375 = 几分之几");
    }

    #[test]
    fn render_math_latex_stacks_fraction_tokens() {
        let tex = render_math_latex("3/5 + 1/5 = 4/5");
        assert!(tex.contains("$\\frac{3}{5}$"));
        assert!(tex.contains("$\\frac{1}{5}$"));
        assert!(tex.contains("$\\frac{4}{5}$"));
    }

    #[test]
    fn render_math_latex_leaves_non_fraction_slashes_alone() {
        assert_eq!(render_math_latex("A/B"), "A/B");
    }

    #[test]
    fn render_math_latex_handles_operators_and_percent() {
        let tex = render_math_latex("50% × 80 ÷ 4 = ");
        assert!(tex.contains("50\\%"));
        assert!(tex.contains("$\\times$"));
        assert!(tex.contains("$\\div$"));
    }

    #[test]
    fn escape_latex_escapes_underscore_and_percent() {
        let escaped = escape_latex("姓名：__________ 50%");
        assert!(escaped.contains("\\_"));
        assert!(escaped.contains("50\\%"));
    }

    #[test]
    fn worked_sections_render_blank_lines() {
        let worksheet = Worksheet {
            title: "test".to_string(),
            subtitle: "sub".to_string(),
            sections: vec![
                Section {
                    kind: SectionKind::Direct,
                    problems: vec![Problem {
                        prompt: "1 + 1 = ".to_string(),
                        answer: "2".to_string(),
                    }],
                },
                Section {
                    kind: SectionKind::Vertical,
                    problems: vec![Problem {
                        prompt: "12 ÷ 3 + 18 ÷ 6 = ".to_string(),
                        answer: "6".to_string(),
                    }],
                },
            ],
        };
        let tex = render_latex(&worksheet, 2, 1, false);
        assert!(tex.contains("\\rule{\\linewidth}{0.4pt}"));
        assert!(tex.contains("\\begin{enumerate}"));
        assert!(tex.contains("\\section*"));
    }

    #[test]
    fn long_conversion_questions_fall_back_to_single_column() {
        let section = Section {
            kind: SectionKind::Conversion,
            problems: vec![Problem {
                prompt: "比较大小：3/4, 0.76, 78%".to_string(),
                answer: "3/4 < 0.76 < 78%".to_string(),
            }],
        };
        let layout = section_layout(&section, 2, DensityMode::Standard);
        assert_eq!(layout.columns, 1);
        assert_eq!(layout.blank_lines, 3);
    }

    #[test]
    fn density_becomes_compact_for_large_single_page_workload() {
        let config = WorksheetConfig {
            counts: [24, 12, 12, 10, 10],
            seed: 20260330,
            ..WorksheetConfig::default()
        };
        let worksheet = generate_worksheet(&config);
        assert_eq!(choose_density(&worksheet, 2, 1), DensityMode::Compact);
    }

    #[test]
    fn density_relaxes_when_target_pages_increase() {
        let config = WorksheetConfig {
            counts: [8, 4, 4, 4, 4],
            seed: 20260330,
            ..WorksheetConfig::default()
        };
        let worksheet = generate_worksheet(&config);
        let one_page = choose_density(&worksheet, 2, 1);
        let two_pages = choose_density(&worksheet, 2, 2);
        assert_eq!(one_page, DensityMode::Compact);
        assert_eq!(two_pages, DensityMode::Standard);
    }

    #[test]
    fn decimal_conversion_is_reduced() {
        let fraction = Fraction::from_decimal(0.375);
        assert_eq!(fraction.to_string(), "3/8");
    }

    #[test]
    fn default_seed_uses_system_time() {
        let lower_bound = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let config = WorksheetConfig::default();
        let upper_bound = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert!(config.seed >= lower_bound);
        assert!(config.seed <= upper_bound);
    }
}
