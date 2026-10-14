//! 依赖清单 / 锁定文件的轻量解析：抽出（包名, 版本）对供漏洞匹配。
//!
//! 只做**逐行文本扫描**，不引入 TOML / YAML / XML 解析依赖（保持 rb-core 依赖最小集）：
//! 漏洞匹配关心的是「装了哪个包的哪个版本」，格式边角（注释、属性展开、继承）
//! 解析不出来时跳过该条依赖即可——漏一条远好于错一条。
//!
//! 版本口径：锁定文件取精确版本；清单文件可能写范围（`^1.2.3`），此时取其中的
//! 基准版本号参与匹配（范围下界近似，见 [`extract_version`]）。

use crate::model::slice_meta::Ecosystem;

/// 从依赖文件解析出的单个依赖。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDep {
    /// 包名（npm 为不含 scope 前缀的完整名，Maven 为 `groupId:artifactId`）。
    pub name: String,
    /// 版本（已剥离范围运算符与前缀）。
    pub version: String,
}

/// 解析一个依赖文件；`content` 为 `None`（超预算 / 二进制）时返回空。
///
/// 按**文件名**分派解析器，文件名不在已知集合内返回空（不猜测格式）。
pub fn parse_dep_file(path: &str, content: Option<&str>, eco: Ecosystem) -> Vec<ParsedDep> {
    let Some(content) = content else {
        return Vec::new();
    };
    let name = file_name(path);
    let mut out = match name {
        "Cargo.lock" | "poetry.lock" => parse_toml_lock(content),
        "Cargo.toml" => parse_cargo_toml(content),
        "package-lock.json" | "composer.lock" | "Pipfile.lock" => parse_json_lock(content),
        "package.json" | "composer.json" => parse_json_manifest(content),
        "yarn.lock" => parse_yarn_lock(content),
        "go.mod" => parse_go_mod(content),
        "requirements.txt" => parse_requirements(content),
        "pom.xml" => parse_pom_xml(content),
        _ => Vec::new(),
    };
    // 同一文件内重复声明（如 package-lock 同时有 packages 与 dependencies）
    // 去重后排序，保证输出稳定、不重复扣分。
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup();
    let _ = eco;
    out
}

/// 取路径末段文件名（统一正斜杠）。
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// 从版本声明中提取可比较的版本号；无法提取时返回 `None`。
///
/// 处理：范围/比较运算符前缀（`^ ~ >= <= > < = v`）、空格、通配 `*`、
/// 非版本占位（`latest` / `workspace:` / `file:` / `git` / URL）。
pub fn extract_version(raw: &str) -> Option<String> {
    let s = raw.trim().trim_matches(['"', '\'', ' ', ',']);
    if s.is_empty() {
        return None;
    }
    for bad in [
        "latest",
        "workspace:",
        "file:",
        "link:",
        "portal:",
        "git+",
        "http://",
        "https://",
        "git://",
        "*",
    ] {
        if s.starts_with(bad) {
            return None;
        }
    }
    let s = s.trim_start_matches(['^', '~', '=', '>', '<', 'v', 'V', ' ']);
    // 取首个「数字开头」的连续片段，截断其后的范围符（如 `1.2.3 || 2.x` 取 `1.2.3`）。
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| {
            !(c.is_ascii_digit()
                || c == '.'
                || c == '-'
                || c == '_'
                || c == '+'
                || c.is_ascii_alphanumeric())
        })
        .unwrap_or(rest.len());
    let v = rest[..end].trim_end_matches(['.', '-', '_']).to_owned();
    if v.is_empty() || !v.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(v)
}

/// 归一化包名：剥离 npm lock 的路径前缀（`node_modules/@scope/pkg` → `@scope/pkg`）。
///
/// package-lock v3 的 `packages` 与 v1 的 `dependencies` 可能同时存在，
/// 前缀不剥离会产生同一包的两条记录（重复扣分）。
fn npm_name(key: &str) -> &str {
    key.rsplit_once("node_modules/")
        .map(|(_, n)| n)
        .unwrap_or(key)
}

/// `key = "value"` 形式的取值（TOML 简易解析）。
fn toml_value(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once('=')?;
    let k = k.trim().to_owned();
    if k.is_empty() {
        return None;
    }
    let v = v.trim().trim_matches('"').to_owned();
    if v.is_empty() {
        return None;
    }
    Some((k, v))
}

/// Cargo.lock / poetry.lock：`[[package]]` 块内的 `name` / `version`。
fn parse_toml_lock(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // 新块开始：先结算上一块
            if let (Some(n), Some(v)) = (name.take(), version.take()) {
                out.push(ParsedDep {
                    name: n,
                    version: v,
                });
            }
            continue;
        }
        if let Some((k, v)) = toml_value(line) {
            match k.as_str() {
                "name" => name = Some(v),
                "version" => version = Some(v),
                _ => {}
            }
        }
    }
    if let (Some(n), Some(v)) = (name, version) {
        out.push(ParsedDep {
            name: n,
            version: v,
        });
    }
    out
}

/// Cargo.toml：仅解析各 `dependencies` 段落内的 `name = "..."` / `name = { version = "..." }`。
fn parse_cargo_toml(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    let mut in_deps = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // 段名形如 [dependencies] / [dev-dependencies] / [target.'cfg(...)'.dependencies]
            in_deps = line.trim_matches(['[', ']']).contains("dependencies");
            continue;
        }
        if !in_deps || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = toml_value(line) else {
            continue;
        };
        // `name = { version = "1.2", features = [...] }`
        let version = if v.starts_with('{') {
            v.trim_matches(['{', '}'])
                .split(',')
                .find_map(|part| {
                    let (pk, pv) = part.split_once('=')?;
                    (pk.trim() == "version").then(|| pv.trim().trim_matches('"').to_owned())
                })
                .unwrap_or_default()
        } else {
            v
        };
        if let Some(v) = extract_version(&version) {
            out.push(ParsedDep {
                name: k,
                version: v,
            });
        }
    }
    out
}

/// package-lock.json / composer.lock / Pipfile.lock：优先 `packages`，回退 `dependencies`。
fn parse_json_lock(content: &str) -> Vec<ParsedDep> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    // npm v2/v3：packages 的键形如 "node_modules/foo" / "node_modules/@scope/bar"
    if let Some(packages) = value.get("packages").and_then(|v| v.as_object()) {
        for (key, entry) in packages {
            if key.is_empty() {
                continue; // 根包自身
            }
            if let Some(version) = entry
                .get("version")
                .and_then(|v| v.as_str())
                .and_then(extract_version)
            {
                out.push(ParsedDep {
                    name: npm_name(key).to_owned(),
                    version,
                });
            }
        }
    }

    // npm v1 / composer.lock / Pipfile.lock：dependencies | packages | default | develop
    for bucket in ["dependencies", "packages", "default", "develop"] {
        if let Some(map) = value.get(bucket).and_then(|v| v.as_object()) {
            for (name, entry) in map {
                if let Some(version) = entry
                    .get("version")
                    .and_then(|v| v.as_str())
                    .and_then(extract_version)
                {
                    out.push(ParsedDep {
                        name: npm_name(name).to_owned(),
                        version,
                    });
                }
            }
        }
    }
    // composer.lock 是数组
    if let Some(arr) = value.as_array() {
        for entry in arr {
            if let (Some(name), Some(version)) = (
                entry.get("name").and_then(|v| v.as_str()),
                entry
                    .get("version")
                    .and_then(|v| v.as_str())
                    .and_then(extract_version),
            ) {
                out.push(ParsedDep {
                    name: name.to_owned(),
                    version,
                });
            }
        }
    }
    out
}

/// package.json / composer.json：dependencies 等映射的**范围声明**。
fn parse_json_manifest(content: &str) -> Vec<ParsedDep> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for bucket in [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
        "require",
        "require-dev",
    ] {
        if let Some(map) = value.get(bucket).and_then(|v| v.as_object()) {
            for (name, version) in map {
                let Some(version) = version.as_str() else {
                    continue;
                };
                if let Some(v) = extract_version(version) {
                    out.push(ParsedDep {
                        name: name.clone(),
                        version: v,
                    });
                }
            }
        }
    }
    out
}

/// yarn.lock（v1 经典格式）：块头 `name@range:` + 块内 `version "1.2.3"`。
fn parse_yarn_lock(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        // 块头：以冒号结尾且不在缩进内
        if !line.starts_with(' ') && trimmed.ends_with(':') {
            let header = trimmed.trim_end_matches(':').trim_matches('"');
            // 形如 "@babel/core@^7.0.0" / "lodash@4.17.21" / "@a/b@^1, @a/b@^2"
            let first = header.split(", ").next().unwrap_or(header);
            let name = first
                .rsplit_once('@')
                .map(|(n, _)| n)
                .unwrap_or(first)
                .trim_matches('"')
                .to_owned();
            current = (!name.is_empty()).then_some(name);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version ") {
            let version = rest.trim().trim_matches('"').to_owned();
            if let (Some(name), Some(version)) = (current.take(), extract_version(&version)) {
                out.push(ParsedDep { name, version });
            }
        }
    }
    out
}

/// go.mod：`require x v1.2.3` 单行或 `require ( ... )` 块。
fn parse_go_mod(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(')') {
            in_block = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("require ") {
            if rest.trim_end().ends_with('(') {
                in_block = true;
                continue;
            }
            if let Some(dep) = go_dep(rest) {
                out.push(dep);
            }
            continue;
        }
        if in_block {
            // 块内行：`module v1.2.3 // indirect`
            let body = line.split("//").next().unwrap_or(line).trim();
            if let Some(dep) = go_dep(body) {
                out.push(dep);
            }
        }
    }
    out
}

/// go.mod 单行 require 体解析（`github.com/gin-gonic/gin v1.9.1`）。
fn go_dep(body: &str) -> Option<ParsedDep> {
    let mut parts = body.split_whitespace();
    let name = parts.next()?.to_owned();
    let version = parts.next()?;
    let version = extract_version(version)?;
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None; // 伪版本（v0.0.0-2023...）已由 extract_version 剥离 v 前缀
    }
    Some(ParsedDep { name, version })
}

/// requirements.txt：`name==1.2.3` / `name[extra]>=1.2` 等。
fn parse_requirements(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or(raw).trim();
        if line.is_empty() || line.starts_with('-') || line.starts_with("http") {
            continue; // -r / -e / 直接 URL 安装
        }
        let name_part = line
            .split(['=', '>', '<', '!', '~', ' '])
            .next()
            .unwrap_or(line)
            .trim();
        // 去掉 extras（`celery[redis]`）
        let name = name_part.split('[').next().unwrap_or(name_part).trim();
        if name.is_empty() {
            continue;
        }
        // 无版本声明（`celery` 裸包名）无法参与漏洞匹配，跳过。
        if let Some(version) = extract_version(line) {
            out.push(ParsedDep {
                name: name.to_owned(),
                version,
            });
        }
    }
    out
}

/// pom.xml：逐个 `<dependency>` 块取 groupId / artifactId / version。
///
/// 不解析 `${property}` 与父 POM 继承：这类版本拿不到就跳过该条（不猜版本）。
fn parse_pom_xml(content: &str) -> Vec<ParsedDep> {
    let mut out = Vec::new();
    for block in content.split("<dependency>").skip(1) {
        let block = block.split("</dependency>").next().unwrap_or(block);
        let group = tag_value(block, "groupId").unwrap_or_default();
        let artifact = match tag_value(block, "artifactId") {
            Some(a) => a,
            None => continue,
        };
        let Some(version) = tag_value(block, "version").and_then(|v| extract_version(&v)) else {
            continue; // 版本由 dependencyManagement / 父 POM 决定，无法静态确定
        };
        let name = if group.is_empty() {
            artifact
        } else {
            format!("{group}:{artifact}")
        };
        out.push(ParsedDep { name, version });
    }
    out
}

/// 取 XML 块中指定标签的文本内容。
fn tag_value(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find(&close)?;
    let v = rest[..end].trim();
    if v.is_empty() || v.starts_with("${") {
        return None;
    }
    Some(v.to_owned())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// 版本提取：范围符、前缀、通配、非版本占位。
    #[test]
    fn 版本提取_范围与占位() {
        assert_eq!(extract_version("^1.2.3").unwrap(), "1.2.3");
        assert_eq!(extract_version("~> 2.0").unwrap(), "2.0");
        assert_eq!(extract_version(">=1.2.3 <2.0.0").unwrap(), "1.2.3");
        assert_eq!(extract_version("v1.2.3").unwrap(), "1.2.3");
        assert_eq!(extract_version("1.2.3 || 2.x").unwrap(), "1.2.3");
        assert_eq!(extract_version("\"1.0.0\",").unwrap(), "1.0.0");
        for bad in [
            "latest",
            "*",
            "workspace:*",
            "file:../x",
            "git+https://a",
            "",
            "  ",
        ] {
            assert_eq!(extract_version(bad), None, "{bad} 应无法提取版本");
        }
    }

    /// Cargo.lock：`[[package]]` 块解析。
    #[test]
    fn 解析_Cargo_lock() {
        let content = r#"
[[package]]
name = "serde"
version = "1.0.100"

[[package]]
name = "rb-core"
version = "0.1.0"
"#;
        let deps = parse_dep_file("Cargo.lock", Some(content), Ecosystem::Rust);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "rb-core");
        assert_eq!(
            deps[1],
            ParsedDep {
                name: "serde".into(),
                version: "1.0.100".into()
            }
        );
    }

    /// Cargo.toml：仅解析 dependencies 段，忽略 [package] 段的 version。
    #[test]
    fn 解析_Cargo_toml() {
        let content = r#"
[package]
name = "demo"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1.35", features = ["full"] }

[dev-dependencies]
tempfile = "3"
"#;
        let deps = parse_dep_file("Cargo.toml", Some(content), Ecosystem::Rust);
        let names: Vec<_> = deps.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["serde", "tempfile", "tokio"]);
        assert_eq!(
            deps.iter().find(|d| d.name == "tokio").unwrap().version,
            "1.35"
        );
    }

    /// package-lock.json v3：packages 映射，跳过根包。
    #[test]
    fn 解析_package_lock_v3() {
        let content = r#"{
          "packages": {
            "": { "name": "root", "version": "1.0.0" },
            "node_modules/lodash": { "version": "4.17.20" },
            "node_modules/@babel/core": { "version": "7.23.0" }
          }
        }"#;
        let deps = parse_dep_file("package-lock.json", Some(content), Ecosystem::JavaScript);
        assert!(deps
            .iter()
            .any(|d| d.name == "lodash" && d.version == "4.17.20"));
        assert!(deps
            .iter()
            .any(|d| d.name == "@babel/core" && d.version == "7.23.0"));
        assert!(!deps.iter().any(|d| d.name == "root"), "根包不应计入");
    }

    /// package-lock.json v1：dependencies 映射回退。
    #[test]
    fn 解析_package_lock_v1回退() {
        let content = r#"{"dependencies": {"minimist": {"version": "1.2.5"}}}"#;
        let deps = parse_dep_file("package-lock.json", Some(content), Ecosystem::JavaScript);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "minimist");
    }

    /// package.json：范围声明取基准版本。
    #[test]
    fn 解析_package_json范围() {
        let content = r#"{
          "dependencies": { "lodash": "^4.17.20", "axios": "latest" },
          "devDependencies": { "vitest": "~1.2.0" }
        }"#;
        let deps = parse_dep_file("package.json", Some(content), Ecosystem::JavaScript);
        assert_eq!(deps.len(), 2, "latest 这类占位不计入");
        assert!(deps
            .iter()
            .any(|d| d.name == "lodash" && d.version == "4.17.20"));
        assert!(deps
            .iter()
            .any(|d| d.name == "vitest" && d.version == "1.2.0"));
    }

    /// yarn.lock v1：块头带 scope 与多范围。
    #[test]
    fn 解析_yarn_lock() {
        let content = r#"
# yarn lockfile v1
lodash@^4.17.20:
  version "4.17.20"

"@babel/core@^7.0.0", "@babel/core@^7.1":
  version "7.23.0"
"#;
        let deps = parse_dep_file("yarn.lock", Some(content), Ecosystem::JavaScript);
        assert!(deps
            .iter()
            .any(|d| d.name == "lodash" && d.version == "4.17.20"));
        assert!(deps
            .iter()
            .any(|d| d.name == "@babel/core" && d.version == "7.23.0"));
    }

    /// go.mod：单行与块两种写法，忽略 indirect 注释。
    #[test]
    fn 解析_go_mod() {
        let content = r#"
module example.com/demo

go 1.22

require github.com/gin-gonic/gin v1.9.1

require (
	github.com/stretchr/testify v1.8.4 // indirect
	golang.org/x/text v0.14.0
)
"#;
        let deps = parse_dep_file("go.mod", Some(content), Ecosystem::Go);
        assert!(deps
            .iter()
            .any(|d| d.name == "github.com/gin-gonic/gin" && d.version == "1.9.1"));
        assert!(deps
            .iter()
            .any(|d| d.name == "github.com/stretchr/testify" && d.version == "1.8.4"));
        assert!(deps
            .iter()
            .any(|d| d.name == "golang.org/x/text" && d.version == "0.14.0"));
    }

    /// requirements.txt：extras、注释与 -r 引用。
    #[test]
    fn 解析_requirements() {
        let content = r#"
# comment
-r other.txt
celery[redis]==5.3.6
requests>=2.31.0  # keep
django==4.2.9
"#;
        let deps = parse_dep_file("requirements.txt", Some(content), Ecosystem::Python);
        assert!(deps
            .iter()
            .any(|d| d.name == "celery" && d.version == "5.3.6"));
        assert!(deps
            .iter()
            .any(|d| d.name == "requests" && d.version == "2.31.0"));
        assert!(deps
            .iter()
            .any(|d| d.name == "django" && d.version == "4.2.9"));
        assert_eq!(deps.len(), 3);
    }

    /// pom.xml：groupId:artifactId，跳过占位版本。
    #[test]
    fn 解析_pom_xml() {
        let content = r#"
<project>
  <dependencies>
    <dependency>
      <groupId>org.apache.commons</groupId>
      <artifactId>commons-text</artifactId>
      <version>1.10.0</version>
    </dependency>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>managed</artifactId>
      <version>${managed.version}</version>
    </dependency>
  </dependencies>
</project>
"#;
        let deps = parse_dep_file("pom.xml", Some(content), Ecosystem::Java);
        assert_eq!(deps.len(), 1, "占位版本无法静态确定，跳过");
        assert_eq!(deps[0].name, "org.apache.commons:commons-text");
        assert_eq!(deps[0].version, "1.10.0");
    }

    /// 内容缺失（超预算 / 二进制）与未知文件名均返回空，不 panic。
    #[test]
    fn 解析_内容缺失与未知文件返回空() {
        assert!(parse_dep_file("Cargo.lock", None, Ecosystem::Rust).is_empty());
        assert!(parse_dep_file("unknown.txt", Some("a==1"), Ecosystem::Python).is_empty());
        assert!(parse_dep_file("package.json", Some("not json"), Ecosystem::JavaScript).is_empty());
    }

    /// 重复声明去重且输出有序（同一文件 packages + dependencies 双写场景）。
    #[test]
    fn 解析_重复依赖去重() {
        let content = r#"{
          "packages": { "node_modules/lodash": { "version": "4.17.20" } },
          "dependencies": { "lodash": { "version": "4.17.20" } }
        }"#;
        let deps = parse_dep_file("package-lock.json", Some(content), Ecosystem::JavaScript);
        assert_eq!(deps.len(), 1, "同一包同一版本只计一次");
    }
}
