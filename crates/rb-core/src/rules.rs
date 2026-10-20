//! 自定义规则：用户以 JSON 定义的（正则 + 路径 glob + 严重度 + 话术）检查规则。
//!
//! 定位：检查器插件机制的轻量替代（见 `docs/roadmap.md` 中期任务）。规则在
//! 进入检查器前统一校验并编译为 [`regex::Regex`]；`regex` crate 为线性时间
//! 匹配（无回溯），因此用户正则不会把扫描拖成指数耗时。
//!
//! 纪律：
//! - 校验失败一律经 [`RbError::InvalidRule`] 返回可读中文原因，不 panic；
//! - 编译产物 [CompiledRule] 自带路径限定与正则，检查器只做纯匹配。

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::error::RbError;
use crate::model::finding::{Category, Severity};
use crate::Result;

/// 规则 id 长度上限（id 会进入 finding id，需稳定且可读）。
const RULE_ID_MAX: usize = 64;

/// 单个文件内最多保留的匹配偏移数（超出只计数、不存储，防止偏移数组膨胀）。
const MAX_OFFSETS_PER_FILE: usize = 4096;

/// 正则长度上限：防御超长规则带来的编译开销。
const PATTERN_MAX: usize = 1024;

/// 标题 / 建议的展示长度上限。
const TEXT_MAX: usize = 200;

/// 一条用户自定义规则（JSON 定义，见 `docs/custom-rules.md`）。
///
/// 除 `id` / `title` / `pattern` / `suggestion` 外的字段均可省略：
/// `severity` 默认 warning、`category` 默认结构维度、`path_glob` 默认不限路径、
/// `ignore_case` 默认区分大小写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomRule {
    /// 稳定唯一标识（`[A-Za-z0-9._-]`，≤ 64 字符），用于 finding id 与去重。
    pub id: String,
    /// 结论标题（命中的问题叫什么）。
    pub title: String,
    /// 严重程度，默认 `warning`。
    #[serde(default = "default_severity")]
    pub severity: Severity,
    /// 所属维度，默认 `structure`。
    #[serde(default = "default_category")]
    pub category: Category,
    /// 匹配用正则（线性时间匹配，可用 `(?s)` 等内联开关）。
    pub pattern: String,
    /// 路径 glob 限定（如 `src/*.rs`）；`None` / 空表示不限路径。
    #[serde(default)]
    pub path_glob: Option<String>,
    /// 是否忽略大小写（默认否）。
    #[serde(default)]
    pub ignore_case: bool,
    /// 可执行的中文改进建议。
    pub suggestion: String,
}

/// [`CustomRule::severity`] 的 serde 缺省值（warning）。
fn default_severity() -> Severity {
    Severity::Warning
}

/// [`CustomRule::category`] 的 serde 缺省值（结构维度）。
fn default_category() -> Category {
    Category::Structure
}

impl CustomRule {
    /// 校验规则合法性：必填项非空、id 字符合法且不过长、正则可编译。
    ///
    /// 错误信息携带规则 id 或序号，便于用户在 GUI / CLI 中直接定位。
    pub fn validate(&self) -> Result<()> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(RbError::InvalidRule("规则 id 不能为空".into()));
        }
        if id.len() > RULE_ID_MAX {
            return Err(RbError::InvalidRule(format!(
                "规则 id 过长（{0} > {RULE_ID_MAX} 字符）：{id}",
                id.len()
            )));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(RbError::InvalidRule(format!(
                "规则 id 只允许字母、数字与 . _ -：{id}"
            )));
        }
        if self.title.trim().is_empty() {
            return Err(RbError::InvalidRule(format!("规则 {id} 的标题不能为空")));
        }
        if self.title.chars().count() > TEXT_MAX {
            return Err(RbError::InvalidRule(format!(
                "规则 {id} 的标题过长（>{TEXT_MAX} 字符）"
            )));
        }
        if self.suggestion.trim().is_empty() {
            return Err(RbError::InvalidRule(format!("规则 {id} 的建议不能为空")));
        }
        if self.suggestion.chars().count() > TEXT_MAX {
            return Err(RbError::InvalidRule(format!(
                "规则 {id} 的建议过长（>{TEXT_MAX} 字符）"
            )));
        }
        if self.pattern.trim().is_empty() {
            return Err(RbError::InvalidRule(format!("规则 {id} 的正则不能为空")));
        }
        if self.pattern.len() > PATTERN_MAX {
            return Err(RbError::InvalidRule(format!(
                "规则 {id} 的正则过长（> {PATTERN_MAX} 字节）"
            )));
        }
        if let Some(glob) = &self.path_glob {
            if glob.trim().is_empty() {
                return Err(RbError::InvalidRule(format!(
                    "规则 {id} 的路径限定不能为空白（留空请直接省略该字段）"
                )));
            }
        }
        self.build_regex()
            .map_err(|e| RbError::InvalidRule(format!("规则 {id} 的正则无法编译: {e}")))?;
        Ok(())
    }

    /// 内部：按 `ignore_case` 构编译正则。
    fn build_regex(&self) -> std::result::Result<Regex, regex::Error> {
        RegexBuilder::new(&self.pattern)
            .case_insensitive(self.ignore_case)
            .build()
    }

    /// 校验并编译为检查器可直接使用的 [`CompiledRule`]。
    pub fn compile(&self) -> Result<CompiledRule> {
        self.validate()?;
        let regex = self
            .build_regex()
            .map_err(|e| RbError::InvalidRule(format!("规则 {} 的正则无法编译: {e}", self.id)))?;
        Ok(CompiledRule {
            rule: self.clone(),
            regex,
        })
    }
}

/// 已编译规则：规则原文 + 编译后的正则，供检查器纯匹配使用。
#[derive(Debug, Clone)]
pub struct CompiledRule {
    /// 规则原文（结论的标题/建议/严重度/维度均取自这里）。
    rule: CustomRule,
    /// 编译后的正则（已应用大小写开关）。
    regex: Regex,
}

impl CompiledRule {
    /// 规则原文。
    pub fn rule(&self) -> &CustomRule {
        &self.rule
    }

    /// 编译后的正则。
    pub fn regex(&self) -> &Regex {
        &self.regex
    }

    /// 该规则是否适用于给定仓库相对路径（无 path_glob 时恒真）。
    pub fn path_allowed(&self, path: &str) -> bool {
        match self.rule.path_glob.as_deref() {
            Some(glob) => crate::glob::matches_pattern(glob, path),
            None => true,
        }
    }

    /// 在文本中查找匹配偏移（按出现顺序，线性时间）。
    ///
    /// 返回 `(偏移量, 命中总数)`：偏移量最多保留 [`MAX_OFFSETS_PER_FILE`] 个，
    /// 超出部分**只计数不存储**——像 `x*` 这类几乎逐字节命中的正则会让偏移
    /// 数组随文件体积线性膨胀（1 MiB 文件可达百万级 usize），而结论证据只需要
    /// 前若干处行号；命中总数始终真实，不做截断。
    pub fn find_offsets(&self, text: &str) -> (Vec<usize>, usize) {
        let mut offsets = Vec::new();
        let mut total = 0usize;
        for m in self.regex.find_iter(text) {
            total += 1;
            if offsets.len() < MAX_OFFSETS_PER_FILE {
                offsets.push(m.start());
            }
        }
        (offsets, total)
    }
}

/// 批量校验并编译规则集；id 重复视为配置错误（finding id 依赖 id 唯一）。
pub fn compile_rules(rules: &[CustomRule]) -> Result<Vec<CompiledRule>> {
    let mut seen: Vec<&str> = Vec::with_capacity(rules.len());
    let mut compiled = Vec::with_capacity(rules.len());
    for (i, rule) in rules.iter().enumerate() {
        if let Some(prev) = seen.iter().find(|id| **id == rule.id) {
            return Err(RbError::InvalidRule(format!(
                "第 {} 条规则的 id 与之前的规则重复: {prev}",
                i + 1
            )));
        }
        seen.push(&rule.id);
        compiled.push(rule.compile()?);
    }
    Ok(compiled)
}

/// 解析规则 JSON（顶层为规则数组）并校验编译；返回规则原文。
///
/// CLI `--rules FILE` 与 GUI 导入共用本入口：**先整体校验再返回**，
/// 不存在「一半规则生效」的中间态。
pub fn load_rules_json(text: &str) -> Result<Vec<CustomRule>> {
    let rules: Vec<CustomRule> = serde_json::from_str(text)
        .map_err(|e| RbError::InvalidRule(format!("规则 JSON 解析失败: {e}")))?;
    compile_rules(&rules)?;
    Ok(rules)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 构造一条最小可用规则。
    fn rule(id: &str) -> CustomRule {
        CustomRule {
            id: id.into(),
            title: "禁用 console.log".into(),
            severity: Severity::Warning,
            category: Category::Structure,
            pattern: r"console\.log".into(),
            path_glob: None,
            ignore_case: false,
            suggestion: "改用统一日志封装，避免调试语句进入主干".into(),
        }
    }

    /// 命中用例：合法规则编译后可按正则找到匹配偏移。
    #[test]
    fn 合法规则_编译后可匹配() {
        let compiled = rule("no-console").compile().unwrap();
        assert!(compiled.path_allowed("src/main.rs"), "未限定路径时恒适用");
        assert_eq!(compiled.find_offsets("a\nconsole.log(1)").0.len(), 1);
        assert_eq!(
            compiled
                .find_offsets("console.log(1); console.log(2)")
                .0
                .len(),
            2
        );
        assert!(compiled.find_offsets("println!(\"hi\")").0.is_empty());
    }

    /// 边界：海量命中时偏移数组受上限约束，但命中总数保持真实。
    #[test]
    fn 海量命中_偏移受上限约束且总数真实() {
        let mut r = rule("many");
        r.pattern = "x*".into();
        let compiled = r.compile().unwrap();
        let text = "abc".repeat(3000);
        let (offsets, total) = compiled.find_offsets(&text);
        assert_eq!(total, text.len() + 1, "命中总数不截断");
        assert!(
            offsets.len() <= MAX_OFFSETS_PER_FILE,
            "偏移不得超过上限: {}",
            offsets.len()
        );
    }

    /// 不命中用例：路径 glob 限定生效，目录外文件跳过。
    #[test]
    fn 路径限定_仅匹配命中glob的文件() {
        let mut r = rule("no-console");
        r.path_glob = Some("src/*.rs".into());
        let compiled = r.compile().unwrap();
        assert!(compiled.path_allowed("src/main.rs"));
        assert!(
            !compiled.path_allowed("deep/src/a.rs"),
            "带分隔符的模式按完整路径匹配，不做 basename 兜底"
        );
        assert!(!compiled.path_allowed("tests/main.rs"));
    }

    /// 边界：无分隔符模式按 basename 兜底，任意目录深度生效。
    #[test]
    fn 路径限定_无分隔符模式按basename兜底() {
        let mut r = rule("no-console");
        r.path_glob = Some("*.rs".into());
        let compiled = r.compile().unwrap();
        assert!(compiled.path_allowed("src/deep/mod.rs"));
        assert!(!compiled.path_allowed("src/deep/mod.ts"));
    }

    /// 边界：ignore_case 开关改变匹配结果。
    #[test]
    fn 忽略大小写开关_生效() {
        let mut r = rule("todo-upper");
        r.pattern = "TODO".into();
        r.ignore_case = true;
        let compiled = r.compile().unwrap();
        assert_eq!(compiled.find_offsets("todo").0.len(), 1);

        let mut strict = rule("todo-upper");
        strict.pattern = "TODO".into();
        let compiled = strict.compile().unwrap();
        assert!(compiled.find_offsets("todo").0.is_empty());
    }

    /// 边界：id 校验（空白 / 非法字符 / 超长）全部拒绝。
    #[test]
    fn 非法id_校验拒绝() {
        for bad in ["", "   ", "has space", "has:colon", "路径"] {
            let mut r = rule("ok");
            r.id = bad.into();
            let err = r.validate().unwrap_err().to_string();
            assert!(
                err.contains("规则 id") || err.contains("正则"),
                "{bad}: {err}"
            );
        }
        let mut r = rule("ok");
        r.id = "a".repeat(RULE_ID_MAX + 1);
        assert!(r.validate().is_err(), "超长 id 应被拒绝");
    }

    /// 边界：正则无法编译与必填文本为空均在编译期被拒。
    #[test]
    fn 非法正则与空文本_校验拒绝() {
        let mut bad_regex = rule("bad-regex");
        bad_regex.pattern = "([".into();
        let err = bad_regex.compile().unwrap_err().to_string();
        assert!(err.contains("正则无法编译"), "{err}");

        let mut empty_title = rule("empty-title");
        empty_title.title = "  ".into();
        assert!(empty_title.validate().is_err());

        let mut empty_suggestion = rule("empty-suggestion");
        empty_suggestion.suggestion = String::new();
        assert!(empty_suggestion.validate().is_err());

        let mut empty_pattern = rule("empty-pattern");
        empty_pattern.pattern = " ".into();
        assert!(empty_pattern.validate().is_err());

        let mut blank_glob = rule("blank-glob");
        blank_glob.path_glob = Some("  ".into());
        assert!(blank_glob.validate().is_err());
    }

    /// id 重复视为配置错误，错误信息携带重复的 id。
    #[test]
    fn id重复_编译报错含id() {
        let rules = vec![rule("dup"), rule("dup")];
        let err = compile_rules(&rules).unwrap_err().to_string();
        assert!(err.contains("dup"), "{err}");
    }

    /// JSON 载入：缺省字段回退默认值（severity→warning / category→structure / path_glob→无 / 区分大小写）。
    #[test]
    fn 载入json_缺省字段回退默认值() {
        let text = r#"[{"id":"a","title":"t","pattern":"x","suggestion":"s"}]"#;
        let rules = load_rules_json(text).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].severity, Severity::Warning);
        assert_eq!(rules[0].category, Category::Structure);
        assert_eq!(rules[0].path_glob, None);
        assert!(!rules[0].ignore_case);
    }

    /// JSON 载入：非法 JSON / 非数组 / 非法规则均给出可读错误。
    #[test]
    fn 载入json_非法输入给出可读错误() {
        for bad in ["", "{}", "not json", r#"[{"id":"a"}]"#] {
            let err = load_rules_json(bad).unwrap_err().to_string();
            assert!(err.contains("自定义规则错误"), "{bad}: {err}");
        }
    }

    /// JSON 往返：紧凑 JSON 序列化字段名为 snake_case，反序列化等价。
    #[test]
    fn 规则json往返_snake_case稳定() {
        let r = rule("roundtrip");
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"path_glob\""), "{json}");
        assert!(json.contains("\"ignore_case\""), "{json}");
        let back: CustomRule = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
