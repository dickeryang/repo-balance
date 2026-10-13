//! 敏感信息检查器（Security 维度）：私钥头、云厂商 Access Key、代码平台
//! token、`.env` 赋值、高熵字符串五类规则的双通道扫描（正则 + 熵）。
//!
//! 纯计算：消费 `FileContents("all")` 切片（编排层已按预算读取文本内容、
//! 过滤忽略规则），不触碰文件系统、不 import git2。
//!
//! # 掩码与误报控制
//! - evidence 中的密钥样本一律经 [`mask_secret`] 掩码（一处掩码，处处掩码）；
//! - 停用词白名单（EXAMPLE / SAMPLE / placeholder 等）跳过文档示例值；
//! - `.env.example` / `.env.sample` / `.env.template` 等模板文件不触发 env 规则；
//! - 二进制与超预算文件无内容，天然不参与扫描。

use std::sync::OnceLock;

use regex::Regex;

use crate::checker::context::SliceData;
use crate::checker::plan::DataSlice;
use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding, Severity};

/// 敏感信息检查器：无状态、纯计算。
pub struct SecretsChecker;

/// 文件内容切片的固定模式（与 plan 声明一致）。
const ALL_PATTERN: &str = "all";

/// 高熵通道的最小值长度（字符数）。
const ENTROPY_MIN_LEN: usize = 20;

/// 高熵通道的香农熵阈值（bits/char）：hex 随机串约 4.0，base64 约 5.2，
/// 自然语言通常 < 4.0；取 4.2 在漏报与误报间折中。
const ENTROPY_THRESHOLD: f64 = 4.2;

/// env 赋值规则的键名展示上限。
const ENV_KEY_PREVIEW: usize = 5;

/// 命中行的证据展示上限。
const LINE_PREVIEW: usize = 8;

/// 规则标识（finding id 中段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    PrivateKey,
    AwsAccessKey,
    GithubToken,
    GitlabToken,
    EnvFile,
    HighEntropy,
}

impl Rule {
    /// 全部规则（稳定顺序）。
    const ALL: [Rule; 6] = [
        Rule::PrivateKey,
        Rule::AwsAccessKey,
        Rule::GithubToken,
        Rule::GitlabToken,
        Rule::EnvFile,
        Rule::HighEntropy,
    ];

    /// finding id 中段（如 `secrets.private-key:{path}`）。
    fn slug(self) -> &'static str {
        match self {
            Rule::PrivateKey => "private-key",
            Rule::AwsAccessKey => "aws-access-key",
            Rule::GithubToken => "github-token",
            Rule::GitlabToken => "gitlab-token",
            Rule::EnvFile => "env-file",
            Rule::HighEntropy => "high-entropy",
        }
    }

    /// 规则中文描述（evidence 展示）。
    fn desc(self) -> &'static str {
        match self {
            Rule::PrivateKey => "私钥头（-----BEGIN … PRIVATE KEY-----）",
            Rule::AwsAccessKey => "AWS Access Key ID（AKIA/ASIA 开头）",
            Rule::GithubToken => "GitHub token（ghp_/gho_/github_pat_ 等）",
            Rule::GitlabToken => "GitLab Personal Access Token（glpat-）",
            Rule::EnvFile => ".env 文件包含赋值配置",
            Rule::HighEntropy => "高熵密钥赋值（key/token/secret 等键名）",
        }
    }

    /// 默认严重度。
    fn severity(self) -> Severity {
        match self {
            Rule::PrivateKey | Rule::AwsAccessKey | Rule::GithubToken | Rule::GitlabToken => {
                Severity::Critical
            }
            Rule::EnvFile | Rule::HighEntropy => Severity::Warning,
        }
    }

    /// 可执行修复建议（中文）。
    fn suggestion(self) -> &'static str {
        match self {
            Rule::PrivateKey => {
                "立即轮换该私钥；使用 git filter-repo --path <文件> --invert-paths 从历史中清除，并将私钥改存密钥管理系统（如 Vault / KMS），仓库内只放占位符"
            }
            Rule::AwsAccessKey => {
                "立即在 AWS IAM 控制台停用并轮换该 Access Key；排查 CloudTrail 确认是否被滥用，后续改用环境变量或 Secrets Manager 注入"
            }
            Rule::GithubToken => {
                "立即在 GitHub Settings → Developer settings 撤销并重建该 token（细粒度权限）；已推送的 token 必须按已泄露处理"
            }
            Rule::GitlabToken => {
                "立即在 GitLab 用户设置 → Access Tokens 撤销该 token；检查审计事件确认使用范围后重建"
            }
            Rule::EnvFile => {
                "将 .env 加入 .gitignore 并执行 git rm --cached .env；提供 .env.example 作为模板，真实配置经部署环境注入；已泄露的值一律轮换"
            }
            Rule::HighEntropy => {
                "确认该高熵值是否为真实密钥：是则轮换并改为从环境变量/密钥管理服务读取；否可在 .repobalance-ignore 中忽略该路径"
            }
        }
    }
}

/// 编译后的内置规则集（正则常量合法；编译失败时该规则禁用而非 panic）。
struct CompiledRules {
    private_key: Option<Regex>,
    aws_access_key: Option<Regex>,
    github_token: Option<Regex>,
    gitlab_token: Option<Regex>,
    /// 键值赋值捕获：第 1 组为候选密钥值。
    key_value: Option<Regex>,
    /// env 赋值捕获：第 1 组为键名，第 2 组为值。
    env_assign: Option<Regex>,
}

/// 全局规则集（惰性编译一次）。
fn rules() -> &'static CompiledRules {
    static RULES: OnceLock<CompiledRules> = OnceLock::new();
    RULES.get_or_init(|| CompiledRules {
        private_key: Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----").ok(),
        aws_access_key: Regex::new(r"(?:AKIA|ASIA|ABIA|ACCA)[0-9A-Z]{16}").ok(),
        github_token: Regex::new(r"gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,}").ok(),
        gitlab_token: Regex::new(r"glpat-[A-Za-z0-9_\-]{20,}").ok(),
        key_value: Regex::new(
            r#"(?i)[A-Za-z0-9_.\-]*(?:key|token|secret|password|passwd|pwd|credential|auth|apikey)[A-Za-z0-9_.\-]*\s*[:=]\s*["']?([A-Za-z0-9+/=_\-.]{20,})["']?"#,
        )
        .ok(),
        env_assign: Regex::new(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_.]*)\s*=\s*(.+?)\s*$").ok(),
    })
}

/// 单条命中（规则 + 行号 + 掩码样本）。
struct Hit {
    rule: Rule,
    line_no: usize,
    sample: String,
}

impl Checker for SecretsChecker {
    fn id(&self) -> &'static str {
        "secrets"
    }

    fn category(&self) -> Category {
        Category::Security
    }

    /// 幂等声明文件内容切片（`all`）：复用大文件检查器已请求的同一切片，
    /// 合并后仅采集一次。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        let mut plan = ScanPlan::new();
        plan.push(DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        });
        plan
    }

    /// 双通道扫描：正则规则逐行匹配；高熵通道仅扫描非 env 文件的键值赋值。
    /// 切片未就绪（或全部无内容）时防御性返回空。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding> {
        let all_slice = DataSlice::FileContents {
            pattern: ALL_PATTERN.to_owned(),
        };
        let Some(SliceData::FileContents(files)) = ctx.slice(&all_slice) else {
            return Vec::new();
        };

        let mut findings = Vec::new();
        for file in files {
            let Some(content) = file.content.as_deref() else {
                continue; // 二进制 / 超预算：无内容可扫
            };
            findings.extend(scan_file(&file.path, content));
        }
        findings.sort_by(|a, b| a.id.cmp(&b.id));
        findings
    }
}

/// 扫描单个文件内容，按 (规则, 文件) 聚合为 finding 列表。
fn scan_file(path: &str, content: &str) -> Vec<Finding> {
    let rules = rules();
    let is_env = is_env_file(path);
    let mut hits: Vec<Hit> = Vec::new();
    let mut env_keys: Vec<String> = Vec::new();

    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let mut strong_hit = false;
        if let Some(re) = &rules.private_key {
            if let Some(m) = re.find(line) {
                if !is_stopword(m.as_str()) {
                    hits.push(Hit {
                        rule: Rule::PrivateKey,
                        line_no,
                        sample: mask_header(m.as_str()),
                    });
                    strong_hit = true;
                }
            }
        }
        if let Some(re) = &rules.aws_access_key {
            if let Some(m) = re.find(line) {
                if !is_stopword(m.as_str()) {
                    hits.push(Hit {
                        rule: Rule::AwsAccessKey,
                        line_no,
                        sample: mask_secret(m.as_str()),
                    });
                    strong_hit = true;
                }
            }
        }
        if let Some(re) = &rules.github_token {
            if let Some(m) = re.find(line) {
                if !is_stopword(m.as_str()) {
                    hits.push(Hit {
                        rule: Rule::GithubToken,
                        line_no,
                        sample: mask_secret(m.as_str()),
                    });
                    strong_hit = true;
                }
            }
        }
        if let Some(re) = &rules.gitlab_token {
            if let Some(m) = re.find(line) {
                if !is_stopword(m.as_str()) {
                    hits.push(Hit {
                        rule: Rule::GitlabToken,
                        line_no,
                        sample: mask_secret(m.as_str()),
                    });
                    strong_hit = true;
                }
            }
        }
        // 高熵通道：跳过 env 文件（由 env 规则统一覆盖）与已被强规则命中的行
        // （避免同一密钥双渠道重复结论）。
        if !is_env && !strong_hit {
            if let Some(re) = &rules.key_value {
                if let Some(caps) = re.captures(line) {
                    let value = &caps[1];
                    if !is_stopword(value)
                        && value.chars().count() >= ENTROPY_MIN_LEN
                        && shannon_entropy(value) >= ENTROPY_THRESHOLD
                    {
                        hits.push(Hit {
                            rule: Rule::HighEntropy,
                            line_no,
                            sample: mask_secret(value),
                        });
                    }
                }
            }
        }
        if is_env {
            if let Some(re) = &rules.env_assign {
                if let Some(caps) = re.captures(line) {
                    let value = caps[2].trim();
                    if !value.is_empty() && value != "\"\"" && value != "''" {
                        env_keys.push(caps[1].to_owned());
                    }
                }
            }
        }
    }

    let mut findings = Vec::new();
    for rule in Rule::ALL {
        let rule_hits: Vec<&Hit> = hits.iter().filter(|h| h.rule == rule).collect();
        if rule_hits.is_empty() {
            continue;
        }
        let lines: Vec<String> = rule_hits
            .iter()
            .take(LINE_PREVIEW)
            .map(|h| h.line_no.to_string())
            .collect();
        let more = if rule_hits.len() > LINE_PREVIEW {
            format!(" 等 {}", rule_hits.len())
        } else {
            String::new()
        };
        let sample = rule_hits[0].sample.clone();
        findings.push(Finding {
            id: format!("secrets.{}:{}", rule.slug(), path),
            severity: rule.severity(),
            category: Category::Security,
            title: format!("检测到{}: {}", rule.desc(), path),
            evidence: format!(
                "文件: {path}\n命中行: {}{more}\n规则: {}\n样本（已掩码）: {sample}",
                lines.join(", "),
                rule.desc(),
            ),
            suggestion: rule.suggestion().to_owned(),
        });
    }

    if !env_keys.is_empty() {
        let preview: Vec<String> = env_keys.iter().take(ENV_KEY_PREVIEW).cloned().collect();
        let more = if env_keys.len() > ENV_KEY_PREVIEW {
            format!(" 等 {} 个", env_keys.len())
        } else {
            String::new()
        };
        findings.push(Finding {
            id: format!("secrets.{}:{}", Rule::EnvFile.slug(), path),
            severity: Rule::EnvFile.severity(),
            category: Category::Security,
            title: format!(".env 配置文件被提交且包含赋值: {path}"),
            evidence: format!(
                "文件: {path}\n赋值键: {}{more}\n说明: .env 常含环境相关机密配置，不应进入版本控制",
                preview.join(", ")
            ),
            suggestion: Rule::EnvFile.suggestion().to_owned(),
        });
    }

    findings
}

/// 是否为触发 env 规则的文件：`.env` 或 `.env.*`，排除示例/模板文件。
fn is_env_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == ".env" {
        return true;
    }
    let Some(rest) = name.strip_prefix(".env.") else {
        return false;
    };
    let lower = rest.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "example" | "sample" | "template" | "dist" | "defaults" | "default"
    )
}

/// 停用词白名单：命中则视为文档示例/占位值，不产出结论。
fn is_stopword(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    const STOPWORDS: [&str; 12] = [
        "example",
        "sample",
        "placeholder",
        "not-a-real",
        "notarealkey",
        "changeme",
        "xxxxxx",
        "dummy",
        "yourkey",
        "your_key",
        "redacted",
        "todo",
    ];
    STOPWORDS.iter().any(|s| lower.contains(s))
}

/// 香农熵（bits/char）。
fn shannon_entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut total = 0usize;
    for ch in s.chars() {
        *counts.entry(ch).or_insert(0usize) += 1;
        total += 1;
    }
    if total == 0 {
        return 0.0;
    }
    let total_f = total as f64;
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / total_f;
            -p * p.log2()
        })
        .sum()
}

/// 密钥样本掩码：保留前 4 / 后 2 字符并标注总长（≤8 字符全掩码）。
fn mask_secret(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    match chars.len() {
        0 => "（空值）".to_owned(),
        n if n <= 8 => "*".repeat(n),
        n => format!(
            "{}…{}（共 {n} 字符，已掩码）",
            chars[..4].iter().collect::<String>(),
            chars[n - 2..].iter().collect::<String>()
        ),
    }
}

/// 私钥头掩码：头本身非机密，仅折叠中间空白防超长行。
fn mask_header(s: &str) -> String {
    if s.chars().count() <= 48 {
        s.to_owned()
    } else {
        let head: String = s.chars().take(45).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::{mask_secret, scan_file, shannon_entropy, SecretsChecker, ALL_PATTERN};
    use crate::checker::{Checker, CheckerRegistry, DataSlice, ScanConfig, ScanContext, SliceData};
    use crate::engine::scan::scan;
    use crate::model::finding::{Finding, Severity};
    use crate::model::snapshot::RepoSnapshot;
    use crate::testutil::TestRepo;

    fn zero_snapshot() -> RepoSnapshot {
        RepoSnapshot {
            path: "/tmp/demo".into(),
            branch_count: 0,
            branches: vec![],
            commit_count: 0,
            recent_commits: vec![],
            file_count: 0,
            total_bytes: 0,
            dep_manifests: vec![],
        }
    }

    /// 构造 AWS 形态密钥（字符串拼接，避免字面量被自身规则命中）。
    fn aws_key() -> String {
        format!("AKIA{}", "QW3RTY7Z9XCV2BNM")
    }

    /// 构造 GitHub token 形态（拼接同上；token 本体 40 字符满足 {36,} 口径）。
    fn gh_token() -> String {
        format!("ghp_{}", "aB3dE6gH9jK2mN5pQ8rS1tU4vW7xY0zAqWeRtYuIo")
    }

    /// 构造高熵赋值形态（拼接同上）。
    fn high_entropy_value() -> String {
        format!("aG7xK2pQ9wL{}", "mN4vBcD3f")
    }

    fn ctx_with_content<'a>(
        snapshot: &'a RepoSnapshot,
        config: &'a ScanConfig,
        files: Vec<(&str, Option<&str>)>,
    ) -> ScanContext<'a> {
        let mut ctx = ScanContext::base(snapshot, config);
        let contents: Vec<crate::model::slice_meta::FileContent> = files
            .into_iter()
            .map(|(path, content)| crate::model::slice_meta::FileContent {
                path: path.into(),
                size: content.map(|c| c.len() as u64).unwrap_or(0),
                content: content.map(str::to_owned),
            })
            .collect();
        ctx.insert_slice(
            DataSlice::FileContents {
                pattern: ALL_PATTERN.into(),
            },
            SliceData::FileContents(contents),
        );
        ctx
    }

    fn ids(findings: &[Finding]) -> Vec<String> {
        findings.iter().map(|f| f.id.clone()).collect()
    }

    // ===== 命中 =====

    /// 私钥头 → Critical，evidence 含掩码规则说明。
    #[test]
    fn 命中_私钥头_产出Critical() {
        let content = format!("-----BEGIN {}PRIVATE KEY-----\nMIIEvQIBADANBg\n", "RSA ");
        let findings = scan_file("deploy/id_rsa", &content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
        assert!(findings[0].id.starts_with("secrets.private-key:"));
        assert!(findings[0].suggestion.contains("git filter-repo"));
    }

    /// AWS Access Key → Critical 且样本已掩码（不出现完整密钥）。
    #[test]
    fn 命中_AWS密钥_产出Critical且掩码() {
        let key = aws_key();
        let content = format!("aws_access_key_id = {key}\n");
        let findings = scan_file("config.yml", &content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
        assert!(findings[0].id.starts_with("secrets.aws-access-key:"));
        assert!(
            !findings[0].evidence.contains(&key),
            "evidence 不得包含未掩码密钥"
        );
        assert!(findings[0].evidence.contains("已掩码"));
    }

    /// GitHub token → Critical。
    #[test]
    fn 命中_GitHubToken_产出Critical() {
        let token = gh_token();
        let content = format!("// auth\nconst t = \"{token}\";\n");
        let findings = scan_file("src/auth.ts", &content);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("secrets.github-token:"));
        assert!(!findings[0].evidence.contains(&token));
    }

    /// GitLab token → Critical。
    #[test]
    fn 命中_GitLabToken_产出Critical() {
        let token = format!("glpat-{}", "Ab3dE6gH9jK2mN5pQ8rS");
        let content = format!("GITLAB_TOKEN={token}\n");
        let findings = scan_file("ci.yml", &content);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("secrets.gitlab-token:"));
    }

    /// .env 赋值 → Warning，证据仅展示键名不展示值。
    #[test]
    fn 命中_env文件赋值_产出Warning且仅展示键名() {
        let content = "API_TOKEN=abc123\nDB_PASSWORD=\"s3cret\"\n# comment\nEMPTY=\n";
        let findings = scan_file(".env", content);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].id.starts_with("secrets.env-file:"));
        assert!(findings[0].evidence.contains("API_TOKEN"));
        assert!(!findings[0].evidence.contains("s3cret"), "不得泄露赋值内容");
    }

    /// 高熵密钥赋值（第二通道）→ Warning。
    #[test]
    fn 命中_高熵密钥赋值_产出Warning() {
        let value = high_entropy_value();
        let content = format!("secret_key = \"{value}\"\n");
        let findings = scan_file("settings.py", &content);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].id.starts_with("secrets.high-entropy:"));
        assert!(!findings[0].evidence.contains(&value));
    }

    // ===== 不命中 =====

    /// 普通文本（无密钥形态、低熵）→ 空结论。
    #[test]
    fn 不命中_普通文本_返回空() {
        let findings = scan_file(
            "README.md",
            "# 项目\n这是一个示例仓库，用于演示体检功能。\nversion = 1.0.0\n",
        );
        assert!(findings.is_empty());
    }

    /// 二进制/超预算文件（content 为 None）→ 跳过不扫。
    #[test]
    fn 不命中_无内容文件_跳过() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ctx_with_content(
            &snap,
            &config,
            vec![
                ("blob.bin", None),
                ("leak.txt", Some(&format!("key = {}\n", aws_key()))),
            ],
        );
        let findings = SecretsChecker.check(&ctx);
        assert_eq!(findings.len(), 1, "仅对有内容文件产出结论");
        assert!(findings[0].id.contains("leak.txt"));
    }

    /// 停用词白名单：文档示例值不产出结论。
    #[test]
    fn 不命中_停用词示例值_返回空() {
        let content = format!("aws_key = AKIA{} \n", "IOSFODNN7EXAMPLE");
        let findings = scan_file("docs/guide.md", &content);
        assert!(findings.is_empty(), "EXAMPLE 形态应被停用词跳过");
    }

    /// .env.example 模板文件不触发 env 规则。
    #[test]
    fn 不命中_env模板文件_不触发env规则() {
        let findings = scan_file(".env.example", "API_TOKEN=your-token-here\n");
        assert!(findings.is_empty());
    }

    /// 低熵长值（自然语言）不触发高熵通道。
    #[test]
    fn 不命中_低熵长值_不触发高熵通道() {
        let content = "api_key = this_is_a_very_long_but_low_entropy_value\n";
        let findings = scan_file("app.conf", content);
        assert!(findings.is_empty());
    }

    // ===== 边界 =====

    /// 边界：熵函数对空串/单字符串稳定；阈值附近区分明显。
    #[test]
    fn 边界_熵函数口径() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        let high = shannon_entropy(&high_entropy_value());
        assert!(high >= 4.2, "构造值应超过阈值: {high}");
        let low = shannon_entropy("aaaaaaaaaaaaaaaaaaaa");
        assert!(low < 4.2, "全同字符熵应低于阈值: {low}");
    }

    /// 边界：掩码函数对短值/空值不 panic。
    #[test]
    fn 边界_掩码函数安全() {
        assert_eq!(mask_secret(""), "（空值）");
        assert_eq!(mask_secret("short"), "*****");
        let masked = mask_secret("QW3RTY7Z9XCV2BNMABCD");
        assert!(masked.contains('…'));
        assert!(!masked.contains("QW3RTY7Z9XCV2BNMABCD"));
    }

    /// 边界：.env 空值与引号空值不计入。
    #[test]
    fn 边界_env空值不计入() {
        let findings = scan_file(".env", "A=\nB=\"\"\nC=''\n");
        assert!(findings.is_empty());
    }

    /// 忽略规则：.repobalance-ignore 忽略 .env 后不产出结论（端到端）。
    #[test]
    fn 忽略规则_被忽略的env文件不产出finding() {
        let mut t = TestRepo::init();
        t.commit(&[(".env", b"API_TOKEN=abc123\n")], "add env");
        std::fs::write(t.path().join(".repobalance-ignore"), ".env\n").unwrap();
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(SecretsChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        assert!(report.findings.is_empty());
    }

    /// 端到端：仓库内 .env + AWS 密钥文件 → 两类结论齐全。
    #[test]
    fn 端到端_多文件命中() {
        let mut t = TestRepo::init();
        let key = aws_key();
        t.commit(&[(".env", b"API_TOKEN=abc123\n")], "add env");
        t.commit(
            &[("config/creds.yml", format!("aws_key: {key}\n").as_bytes())],
            "add creds",
        );
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(SecretsChecker)).unwrap();
        let report = scan(t.path(), &registry).unwrap();
        let got = ids(&report.findings);
        assert!(got.iter().any(|id| id.starts_with("secrets.env-file:")));
        assert!(got
            .iter()
            .any(|id| id.starts_with("secrets.aws-access-key:")));
        // env-file Warning(-8) + aws Critical(-15) = 77；高熵通道被强规则行抑制。
        assert_eq!(report.scores.security, Some(77));
    }

    /// plan 声明幂等：两次调用等价，切片恰为 FileContents("all")。
    #[test]
    fn plan声明幂等_含FileContents切片() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let p1 = SecretsChecker.plan(&ctx);
        let p2 = SecretsChecker.plan(&ctx);
        assert_eq!(p1, p2);
        assert_eq!(p1.slices().len(), 1);
        assert!(p1
            .slices()
            .iter()
            .any(|s| matches!(s, DataSlice::FileContents { pattern } if pattern == "all")));
    }

    /// 同一实例重复 check 结果等价（纯函数）；切片缺失时防御性返回空。
    #[test]
    fn check纯函数_重复等价_切片缺失返回空() {
        let snap = zero_snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        assert!(SecretsChecker.check(&ctx).is_empty(), "切片未就绪返回空");

        let content = format!("key = {}\n", aws_key());
        let ctx = ctx_with_content(&snap, &config, vec![("a.txt", Some(&content))]);
        assert_eq!(SecretsChecker.check(&ctx), SecretsChecker.check(&ctx));
    }
}
