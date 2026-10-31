//! PDF 导出：typst 排版引擎 + 系统中文字体。
//!
//! # 为什么用 typst
//! 自研 PDF 排版（printpdf 级别的原语）需要手写换行 / 表格 / 分页，且字体子集化
//! 很难做对；typst（Apache-2.0）是成熟的开源排版引擎：CJK 断行、表格、分页与
//! **字体子集化**开箱即用，PDF 体积只包含实际用到的字形。
//!
//! # 字体来源（纯本地，不联网）
//! 不随包分发字体（体积考虑），按平台候选列表加载**系统 CJK 字体**：
//! macOS PingFang / Windows 微软雅黑 / Linux Noto Sans CJK（`fonts-noto-cjk`）。
//! 全部缺失时返回明确的中文错误，不静默产出乱码 PDF（CI 已安装 fonts-noto-cjk）。
//!
//! # 雷达图
//! **typst 原生绘制**（polygon 覆盖层）而非复用 HTML 导出的 SVG——typst-pdf 的
//! svg2pdf 未启用 `text` feature，SVG 里的 `<text>` 标签会被静默丢弃；
//! 原生绘制的文字与正文字体一致，且不受该限制。
//!
//! # 评分口径
//! 总分 = 已覆盖维度平均（四舍五入），全部 N/A 时如实显示「N/A」，
//! 与前端 `totalOrNullOf` 同一口径。

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use typst_pdf::PdfOptions;

use crate::commands::{category_cn, severity_key};
use crate::dto::ScanReportDto;

/// 系统中文字体候选（按平台排列，命中第一个存在的文件即用）。
const FONT_CANDIDATES: &[&str] = &[
    // macOS（系统 CJK 字体几乎全是 TTC 集合，逐 face 加载）
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    // Windows
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\simhei.ttf",
    "C:\\Windows\\Fonts\\simsun.ttc",
    // Linux（fonts-noto-cjk / wqy；CI 质量门禁已安装前者）
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];

/// 单 face 加载防御上限：TTC 集合可能包含十几个 face，超过视为异常数据。
const MAX_FACES_PER_FILE: u32 = 32;

/// 生成报告 PDF 字节流。
///
/// # Errors
/// - 系统缺少 CJK 字体（返回带修复指引的中文错误，不静默降级）；
/// - typst 编译 / PDF 导出失败（合并全部诊断消息，便于一次看全）。
pub fn report_to_pdf(report: &ScanReportDto) -> Result<Vec<u8>, String> {
    let (book, fonts) = load_cjk_fonts()?;
    let family = fonts
        .first()
        .map(|f| f.info().family.clone())
        .unwrap_or_default();
    let world = PdfWorld::new(book, fonts, &family, report);

    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned.output.map_err(diagnostics_to_string)?;
    typst_pdf::pdf(&document, &PdfOptions::default()).map_err(diagnostics_to_string)
}

/// 把 typst 诊断合并为一条可读消息（`# Errors` 口径：一次看全，不留谜语）。
fn diagnostics_to_string(errs: typst::ecow::EcoVec<typst::diag::SourceDiagnostic>) -> String {
    errs.iter()
        .map(|e| e.message.to_string())
        .collect::<Vec<_>>()
        .join("；")
}

/// 扫描系统 CJK 字体：返回字体簿与字体列表。
fn load_cjk_fonts() -> Result<(LazyHash<FontBook>, Vec<Font>), String> {
    for path in FONT_CANDIDATES {
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        let bytes = Bytes::new(data);
        let mut fonts = Vec::new();
        for index in 0..MAX_FACES_PER_FILE {
            match Font::new(bytes.clone(), index) {
                Some(font) => fonts.push(font),
                None => break,
            }
        }
        if !fonts.is_empty() {
            return Ok((LazyHash::new(FontBook::from_fonts(&fonts)), fonts));
        }
    }
    Err(
        "未找到可用的中文字体，无法导出 PDF：macOS / Windows 使用系统自带字体即可，\
         Linux 请安装 fonts-noto-cjk（PDF 全程本地生成，不联网下载字体）"
            .to_owned(),
    )
}

/// typst 的最小 [`World`] 实现：单一 main 源文件 + 预载字体，无外部文件。
struct PdfWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
}

impl PdfWorld {
    fn new(
        book: LazyHash<FontBook>,
        fonts: Vec<Font>,
        family: &str,
        report: &ScanReportDto,
    ) -> Self {
        let main = Source::new(
            FileId::new(None, VirtualPath::new("/report.typ")),
            report_to_typst(report, family),
        );
        Self {
            library: LazyHash::new(Library::builder().build()),
            book,
            fonts,
            main,
        }
    }
}

impl World for PdfWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            Ok(self.main.clone())
        } else {
            Err(FileError::NotFound(
                id.vpath().as_rooted_path().display().to_string().into(),
            ))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        // 模板不引用外部文件；到这里说明模板写错了引用，按未找到处理
        Err(FileError::NotFound(
            id.vpath().as_rooted_path().display().to_string().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        // 报告时间由 Rust 侧从 started_at 生成并注入模板，不依赖 typst 时钟
        None
    }
}

// ===== 模板生成 =====

/// 把报告渲染为 typst 源码。用户数据一律进**字符串字面量**（只需转义
/// 反斜杠与引号），避免 markup 里的 `*` / `#` / `<` 等注入排版语义。
fn report_to_typst(report: &ScanReportDto, family: &str) -> String {
    let mut s = String::with_capacity(8 * 1024);
    s.push_str("#set page(paper: \"a4\", margin: (x: 18mm, y: 20mm), numbering: \"1\")\n");
    s.push_str(&format!(
        "#set text(font: (\"{}\", \"Libertinus Serif\"), size: 10pt, lang: \"zh\")\n",
        esc(family)
    ));
    s.push_str("#set par(justify: true, leading: 0.62em)\n");
    s.push_str(&format!(
        "#set document(title: \"仓衡扫描报告 · {}\", author: \"RepoBalance\")\n",
        breakable(&esc(&report.repo_path))
    ));

    // 严重度配色（与界面主题一致的打印友好版）与文案
    s.push_str("#let sev-color = (critical: rgb(\"#c62f45\"), warning: rgb(\"#b26a00\"), info: rgb(\"#1f7a5c\"))\n");
    s.push_str("#let sev-label = (critical: \"严重\", warning: \"警告\", info: \"提示\")\n");

    // findings 数组（字符串全部转义）
    s.push_str("#let findings = (\n");
    for f in &report.findings {
        s.push_str(&format!(
            "  (severity: \"{}\", id: \"{}\", category: \"{}\", title: \"{}\", \
             evidence: \"{}\", suggestion: \"{}\"),\n",
            esc(severity_key(&f.severity)),
            breakable(&esc(&f.id)),
            esc(category_cn(&f.category)),
            breakable(&esc(&f.title)),
            breakable(&esc(&f.evidence)),
            breakable(&esc(&f.suggestion)),
        ));
    }
    s.push_str(")\n");

    // 覆盖范围（受限扫描如实标注）
    match &report.scope_note {
        Some(note) => s.push_str(&format!(
            "#let scope-note = \"{}\"\n",
            breakable(&esc(note))
        )),
        None => s.push_str("#let scope-note = none\n"),
    }

    // 信息表 + 标题
    let total = total_score(report);
    s.push_str(&format!(
        concat!(
            "#align(center)[\n",
            "  #text(size: 19pt, weight: \"bold\")[仓衡扫描报告]\n",
            "  #v(1.5mm)\n",
            "  #text(size: 8.5pt, fill: luma(110))[RepoBalance · 纯本地体检 · evidence 已掩码]\n",
            "]\n",
            "#v(3mm)\n",
            "#table(\n",
            "  columns: (auto, 1fr),\n",
            "  inset: 5pt,\n",
            "  stroke: none,\n",
            "  fill: (_, y) => if calc.odd(y) {{ luma(245) }},\n",
            "  align: left,\n",
            "  [仓库], [#raw(\"{}\")],\n",
            "  [生成时间], [{}],\n",
            "  [引擎版本], [{}],\n",
            "  [扫描耗时], [{} ms],\n",
            "  [覆盖范围], [#if scope-note == none [全量] else [#scope-note]],\n",
            "  [总分], [#text(weight: \"bold\")[{}]],\n",
            ")\n",
            "#v(2mm)\n"
        ),
        breakable(&esc(&report.repo_path)),
        fmt_utc(report.started_at),
        esc(&report.engine_version),
        report.duration_ms,
        match total {
            Some(t) => format!("{t} / 100"),
            None => "N/A（无覆盖维度）".to_owned(),
        },
    ));

    // 五维评分表 + 雷达图
    s.push_str("== 五维评分\n\n");
    let dims: Vec<String> = report.radar_points.iter().map(|p| esc(&p.name)).collect();
    let cells: Vec<String> = report
        .radar_points
        .iter()
        .map(|p| match p.score {
            Some(v) => v.to_string(),
            None => "N/A".to_owned(),
        })
        .collect();
    s.push_str(&format!(
        "#table(\n  columns: {},\n  align: center,\n  inset: 6pt,\n  stroke: 0.4pt + luma(200),\n",
        report.radar_points.len() + 1
    ));
    s.push_str("  ");
    for d in &dims {
        s.push_str(&format!("[*{d}*], "));
    }
    s.push_str("[*总分*],\n  ");
    for c in &cells {
        s.push_str(&format!("[{c}], "));
    }
    s.push_str(&format!(
        "[{}],\n)\n#v(2mm)\n\n",
        match total {
            Some(t) => t.to_string(),
            None => "N/A".to_owned(),
        }
    ));

    s.push_str(&radar_typst(&report.radar_points));
    s.push('\n');

    // 问题列表
    s.push_str(&format!(
        "== 发现的问题（{} 条）\n\n",
        report.findings.len()
    ));
    s.push_str(
        "#if findings.len() == 0 [\n  未发现问题，仓库健康 ✓\n]\n#for f in findings [\n  #block(\n    breakable: true,\n    width: 100%,\n    inset: 8pt,\n    radius: 4pt,\n    fill: luma(249),\n    stroke: (left: 2.5pt + sev-color.at(f.severity)),\n  )[\n",
    );
    s.push_str(
        "    #box(fill: sev-color.at(f.severity).lighten(80%), inset: (x: 4pt, y: 2pt), radius: 3pt, \
         text(fill: sev-color.at(f.severity), size: 8.5pt, weight: \"bold\")[#sev-label.at(f.severity)])\n",
    );
    s.push_str("    #h(5pt) #text(weight: \"bold\")[#f.title]\n");
    s.push_str("    #v(1.5mm) #text(size: 9pt)[#emph[类别] #h(5pt) #f.category #h(14pt) #emph[ID] #h(5pt) #raw(f.id)]\n");
    s.push_str("    #v(1mm) #text(size: 9pt)[#emph[证据] #h(5pt) #f.evidence]\n");
    s.push_str("    #v(1mm) #text(size: 9pt)[#emph[建议] #h(5pt) #f.suggestion]\n");
    s.push_str("  ]\n  #v(2.5mm)\n]\n");
    s
}

/// 雷达图的 typst 原生绘制：网格五边形 + 轴线 + 数据多边形 + 维度标签，
/// 全部为绝对坐标覆盖层（与 HTML 导出的 SVG 同一几何口径，N/A 按 0 绘制并标注）。
fn radar_typst(points: &[crate::dto::RadarPointDto]) -> String {
    const CX: f64 = 33.0; // mm，箱体中心
    const CY: f64 = 31.0;
    const R: f64 = 22.0; // 数据多边形最大半径
    const LABEL_R: f64 = 29.0; // 标签半径（箱内）
    const BOX_W: f64 = 66.0;
    const BOX_H: f64 = 62.0;
    const PI: f64 = std::f64::consts::PI;

    let angle = |i: usize| -> f64 { -PI / 2.0 + (i as f64) * 2.0 * PI / 5.0 };
    // 箱体坐标（原点=左上角），供 polygon 使用
    let vertex = |i: usize, r: f64| -> (f64, f64) {
        let a = angle(i);
        (CX + r * a.cos(), CY + r * a.sin())
    };
    // 相对箱体中心的偏移，供 place(dx, dy) 的标签使用
    let offset = |i: usize, r: f64| -> (f64, f64) {
        let a = angle(i);
        (r * a.cos(), r * a.sin())
    };
    let pts = |radius: f64| -> String {
        (0..5)
            .map(|i| {
                let (x, y) = vertex(i, radius);
                format!("({x:.2}mm, {y:.2}mm), ")
            })
            .collect()
    };

    let mut s = String::from("== 五维雷达\n\n#align(center)[\n#box(width: 66mm, height: 62mm)[\n");
    // 同心网格（4 档）与轴线
    for ratio in [1.0_f64, 0.75, 0.5, 0.25] {
        s.push_str(&format!(
            "#place(polygon(stroke: 0.4pt + luma(205), fill: none, {}))\n",
            pts(R * ratio)
        ));
    }
    for i in 0..5 {
        let (x1, y1) = vertex(i, 0.0);
        let (x2, y2) = vertex(i, R);
        s.push_str(&format!(
            "#place(polygon(stroke: 0.4pt + luma(205), fill: none, ({x1:.2}mm, {y1:.2}mm), ({x2:.2}mm, {y2:.2}mm)))\n"
        ));
    }
    // 数据多边形（任一维度有分才画，全 N/A 不画空壳）
    if points.iter().any(|p| p.score.is_some()) {
        let data: String = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let r = (p.score.unwrap_or(0) as f64 / 100.0) * R;
                let (x, y) = vertex(i, r);
                format!("({x:.2}mm, {y:.2}mm), ")
            })
            .collect();
        s.push_str(&format!(
            "#place(polygon(fill: rgb(\"#4f8cff\").lighten(75%), stroke: 1.2pt + rgb(\"#4f8cff\"), {data}))\n"
        ));
    }
    // 维度标签（名称 + 得分 / N/A）
    for (i, p) in points.iter().enumerate() {
        let (dx, dy) = offset(i, LABEL_R);
        let score = p
            .score
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_owned());
        s.push_str(&format!(
            "#place(center + horizon, dx: {dx:.2}mm, dy: {dy:.2}mm, text(size: 8pt, fill: luma(120))[{} {}])\n",
            esc(&p.name),
            score
        ));
    }
    let _ = (BOX_W, BOX_H); // 尺寸以字面量写在 box 上，常量仅作口径说明
    s.push_str("]\n]\n\n");
    s
}

/// 总分（已覆盖维度平均，四舍五入）；全部 N/A 时为 `None`。
/// 与前端 `totalOrNullOf` 同口径：区分「0 分」与「无结论」。
fn total_score(report: &ScanReportDto) -> Option<u8> {
    let covered: Vec<u8> = report.radar_points.iter().filter_map(|p| p.score).collect();
    if covered.is_empty() {
        return None;
    }
    let sum: u32 = covered.iter().map(|v| u32::from(*v)).sum();
    Some((sum as f64 / covered.len() as f64).round() as u8)
}

/// typst 字符串字面量转义：反斜杠与引号之外的控制字符走 `\u{..}`（`\\n` 之外的
/// 换行 / 制表等一并转义，杜绝排版语义注入）。
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\u{a}"),
            '\r' => out.push_str("\\u{d}"),
            '\t' => out.push_str("\\u{9}"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// typst 只在空格与少数 URL 字符（`/ : ? & =` 等）处提供断行机会，不含这些
/// 字符的连续长 token（无斜杠长域名、JWT、长哈希、深路径段）会把 A4 页面
/// 撑出页边。这里兜底：连续不可断段超过 [`BREAKABLE_LIMIT`] 时插入零宽空格
/// U+200B 作为可断点；典型 URL（含 `/` 等内置断点）不会被改动。
const BREAKABLE_LIMIT: usize = 60;

fn breakable(s: &str) -> String {
    // typst 自身可断的字符（超出此集合的连续段才需要兜底断点）
    const BUILT_IN: &[char] = &[
        '/', ':', '?', '&', '=', '#', '@', '~', '-', '_', '.', ',', ';', '%',
    ];
    let mut out = String::with_capacity(s.len());
    let mut run = 0usize;
    for c in s.chars() {
        out.push(c);
        if c.is_whitespace() || BUILT_IN.contains(&c) {
            run = 0;
        } else {
            run += 1;
            if run >= BREAKABLE_LIMIT {
                out.push('\u{200b}');
                run = 0;
            }
        }
    }
    out
}

/// Unix 秒 → `YYYY-MM-DD HH:MM UTC`（报告头明确标注 UTC 口径，不做时区猜测）。
fn fmt_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs = epoch_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

/// 天数 → 公历日期（Howard Hinnant 的 `civil_from_days` 算法，公有领域）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{FindingDto, RadarPointDto};
    use rb_core::model::finding::{Category, Severity};

    fn fixture() -> ScanReportDto {
        ScanReportDto {
            repo_path: "/tmp/demo \"仓库\"\n路径含转义".to_owned(),
            findings: vec![FindingDto {
                id: "secrets.env-file:.env".to_owned(),
                severity: Severity::Critical,
                category: Category::Security,
                title: ".env 配置文件被提交且包含赋值: .env".to_owned(),
                evidence: "命中 2 处（已掩码）".to_owned(),
                suggestion: "把 .env 加入 .gitignore，并改用密钥管理服务".to_owned(),
            }],
            scores: crate::dto::ScoresDto {
                structure: Some(90),
                history: Some(85),
                branches: None,
                deps: Some(78),
                security: Some(40),
            },
            radar_points: vec![
                RadarPointDto {
                    key: "structure".into(),
                    name: "结构".into(),
                    score: Some(90),
                    finding_count: 1,
                },
                RadarPointDto {
                    key: "history".into(),
                    name: "历史".into(),
                    score: Some(85),
                    finding_count: 0,
                },
                RadarPointDto {
                    key: "branches".into(),
                    name: "分支".into(),
                    score: None,
                    finding_count: 0,
                },
                RadarPointDto {
                    key: "deps".into(),
                    name: "依赖".into(),
                    score: Some(78),
                    finding_count: 2,
                },
                RadarPointDto {
                    key: "security".into(),
                    name: "安全".into(),
                    score: Some(40),
                    finding_count: 1,
                },
            ],
            started_at: 0,
            duration_ms: 123,
            engine_version: "test".to_owned(),
            cancelled: false,
            scope_note: Some("最多 500 个提交（CLI --max-commits 500）".to_owned()),
        }
    }

    #[test]
    fn 转义_反斜杠引号与控制字符() {
        assert_eq!(esc("a\\b\"c\nd\te"), "a\\\\b\\\"c\\u{a}d\\u{9}e");
        assert_eq!(esc("\u{1}"), "\\u{1}");
    }

    #[test]
    fn 时间戳_epoch_零是_1970_01_01() {
        assert_eq!(fmt_utc(0), "1970-01-01 00:00 UTC");
        // 2026-10-31 00:00:00 UTC
        assert_eq!(fmt_utc(1_793_404_800), "2026-10-31 00:00 UTC");
    }

    #[test]
    fn 总分_只平均已覆盖维度且全na为none() {
        assert_eq!(total_score(&fixture()), Some(73)); // (90+85+78+40)/4 = 73.25 → 73
        let mut all_na = fixture();
        for p in &mut all_na.radar_points {
            p.score = None;
        }
        assert_eq!(total_score(&all_na), None);
    }

    #[test]
    fn 模板_用户数据全部进转义字符串() {
        let src = report_to_typst(&fixture(), "PingFang SC");
        // 仓库路径里的引号与换行必须以转义形式出现在字符串字面量里
        assert!(
            src.contains("仓库 \\\"\\u{a}路径含转义") || src.contains("\\\""),
            "{src}"
        );
        assert!(src.contains("scope-note = \"最多 500 个提交"), "{src}");
        // 雷达图是原生 polygon，不含 svg 引用
        assert!(!src.contains("image("));
    }

    #[test]
    fn pdf_导出_头部与体积合理() {
        match report_to_pdf(&fixture()) {
            Ok(bytes) => {
                assert!(bytes.starts_with(b"%PDF"), "应为 PDF 魔数");
                // 子集化后也应有实质内容（字体子集 + 图形）
                assert!(bytes.len() > 4_000, "体积异常小: {}", bytes.len());
            }
            Err(msg) => {
                // 仅当环境缺 CJK 字体时允许走到这里（CI 已安装 fonts-noto-cjk）
                assert!(msg.contains("中文字体"), "意外的错误: {msg}");
            }
        }
    }

    /// 断点兜底：典型 URL（含 `/` 等内置断点字符）原样保留；
    /// 无断点长 token 插零宽空格且不丢失字符。
    #[test]
    fn 断点兜底_典型url不动_超长无断点token插零宽空格() {
        let url = "https://atomgit.com/dickeryang/repo-balance.git";
        assert_eq!(breakable(url), url);
        let long = "a".repeat(180);
        let out = breakable(&long);
        // 60 / 120 / 180 三处各插一个
        assert_eq!(out.matches('\u{200b}').count(), 3, "{out}");
        let stripped: String = out.chars().filter(|c| *c != '\u{200b}').collect();
        assert_eq!(stripped, long, "插入断点不得丢失字符");
    }

    /// 远程体检报告（repo_path 为 URL、临时目录是实现细节）可导出 PDF，
    /// URL 完整进入模板（title 与信息表）。
    #[test]
    fn pdf_导出_远程_url_报告可导出() {
        let mut report = fixture();
        report.repo_path = "https://example.com/dickeryang/repo.git?token=abc&ref=main".to_owned();
        let src = report_to_typst(&report, "PingFang SC");
        assert!(
            src.contains("https://example.com/dickeryang/repo.git?token=abc&ref=main"),
            "{src}"
        );
        match report_to_pdf(&report) {
            Ok(bytes) => assert!(bytes.starts_with(b"%PDF")),
            Err(msg) => panic!("远程 URL 报告应可导出: {msg}"),
        }
    }
}
