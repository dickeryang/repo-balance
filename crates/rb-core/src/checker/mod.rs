//! checker 模块：检查器插件体系（契约、注册表、切片计划、扫描上下文、占位实现）。
//!
//! 模块依赖方向：mod → plan/context → model/error，noop → mod，无循环依赖；
//! 本模块不 import git2、不引入 tauri，全部公共类型经 `crate::checker::`
//! 单一命名空间导出。

pub mod big_files;
pub mod context;
pub mod noop;
pub mod plan;

use crate::error::RbError;
use crate::model::finding::{Category, Finding};
use crate::Result;

pub use big_files::BigFilesChecker;
pub use context::{ScanConfig, ScanContext, SliceData};
pub use noop::NoopChecker;
pub use plan::{merge_plans, DataSlice, ScanPlan};

/// 单个检查器：声明所需数据切片（plan），基于就绪切片纯计算产出结论（check）。
///
/// 实现约定：
/// - 无状态（`Send + Sync`，可跨线程共享引用）；
/// - `check` 为纯计算，同输入同输出，不引入失败通道；
/// - `evidence` 必须为掩码后的证据字符串；
/// - `suggestion` 必须为可执行中文建议。
pub trait Checker: Send + Sync {
    /// 稳定唯一标识，如 `"big-files"` / `"zombie-branches"`。
    fn id(&self) -> &'static str;

    /// 所属检查维度。
    fn category(&self) -> Category;

    /// 声明本检查器需要的数据切片。
    ///
    /// 幂等：同一上下文多次调用结果等价；切片仅限 [`DataSlice`] 封闭集，
    /// 采集由编排层统一完成（本方法只声明、不采集）。
    fn plan(&self, ctx: &ScanContext) -> ScanPlan;

    /// 基于已就绪切片执行检查，返回结论列表（纯计算，不引入失败通道）。
    fn check(&self, ctx: &ScanContext) -> Vec<Finding>;
}

/// 检查器注册表：持有全部 Checker 实例，供扫描编排消费（spec 5.4）。
///
/// 线程安全约束已上移至 [`Checker`] trait 声明，容器侧无冗余约束；
/// 任一时刻注册表内不存在重复 id（spec 6.4.2）。
#[derive(Default)]
pub struct CheckerRegistry {
    /// 按注册序保存的检查器实例。
    checkers: Vec<Box<dyn Checker>>,
}

impl CheckerRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个检查器。
    ///
    /// id 冲突时返回 [`RbError::DuplicateChecker`]（文案携带冲突 id），
    /// 且既有注册项不覆盖、不破坏；成功时返回 `&mut Self` 保留链式用法。
    pub fn register(&mut self, checker: Box<dyn Checker>) -> Result<&mut Self> {
        let id = checker.id();
        if self.checkers.iter().any(|c| c.id() == id) {
            return Err(RbError::DuplicateChecker { id: id.to_owned() });
        }
        self.checkers.push(checker);
        Ok(self)
    }

    /// 全部已注册检查器（注册序）；`engine::scan` 的唯一查询入口。
    pub fn list(&self) -> &[Box<dyn Checker>] {
        &self.checkers
    }

    /// 已注册检查器数量。
    pub fn len(&self) -> usize {
        self.checkers.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.checkers.is_empty()
    }
}

impl std::fmt::Debug for CheckerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CheckerRegistry")
            .field("len", &self.checkers.len())
            .finish()
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::model::finding::Severity;
    use crate::model::snapshot::RepoSnapshot;

    /// 可编程 mock 检查器：id / category / plan 声明切片 / check 结论均可配置。
    struct MockChecker {
        id: &'static str,
        category: Category,
        slices: Vec<DataSlice>,
        findings: Vec<Finding>,
    }

    impl MockChecker {
        fn new(id: &'static str, category: Category) -> Self {
            Self {
                id,
                category,
                slices: Vec::new(),
                findings: Vec::new(),
            }
        }

        fn with_slices(mut self, slices: Vec<DataSlice>) -> Self {
            self.slices = slices;
            self
        }

        fn with_findings(mut self, findings: Vec<Finding>) -> Self {
            self.findings = findings;
            self
        }
    }

    impl Checker for MockChecker {
        fn id(&self) -> &'static str {
            self.id
        }

        fn category(&self) -> Category {
            self.category
        }

        fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
            let mut plan = ScanPlan::new();
            for s in &self.slices {
                plan.push(s.clone());
            }
            plan
        }

        fn check(&self, _ctx: &ScanContext) -> Vec<Finding> {
            self.findings.clone()
        }
    }

    /// 构造一条最小 finding。
    fn finding(id: &str) -> Finding {
        Finding {
            id: id.into(),
            severity: Severity::Warning,
            category: Category::Structure,
            title: "标题".into(),
            evidence: "证据".into(),
            suggestion: "建议".into(),
        }
    }

    /// 内存构造零值快照（不触碰 git）。
    fn snapshot() -> RepoSnapshot {
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

    /// 注册成功后 list 可查询到该检查器且 len 正确。
    #[test]
    fn 注册成功_可查询_len正确() {
        let mut registry = CheckerRegistry::new();
        registry
            .register(Box::new(MockChecker::new("a", Category::Structure)))
            .unwrap();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list()[0].id(), "a");
        assert!(!registry.is_empty());
    }

    /// 重复 id 注册返回 Err 且文案含冲突 id，既有注册项零变化。
    #[test]
    fn 重复id注册_返回Err含冲突id_既有项不变() {
        let mut registry = CheckerRegistry::new();
        registry
            .register(Box::new(MockChecker::new("dup", Category::History)))
            .unwrap();
        let err = registry
            .register(Box::new(MockChecker::new(
                "dup",
                Category::Structure,
            )))
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("dup"), "错误文案缺少冲突 id: {msg}");
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list()[0].id(), "dup");
        assert_eq!(registry.list()[0].category(), Category::History);
    }

    /// 依次注册 A、B、C 后 list 顺序为 A、B、C（注册序稳定，spec 6.4.3）。
    #[test]
    fn 注册顺序保持_按注册序返回() {
        let mut registry = CheckerRegistry::new();
        for id in ["a", "b", "c"] {
            registry
                .register(Box::new(MockChecker::new(id, Category::Structure)))
                .unwrap();
        }
        let ids: Vec<&str> = registry.list().iter().map(|c| c.id()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    /// 经 mock 的 plan(ctx) 收集多份计划后 merge_plans 归并——
    /// 同请求多 Checker 仅采集一次、异请求不丢失（spec 5.2.2 交互流程）。
    #[test]
    fn mock计划合并_同请求仅采集一次_异请求不丢失() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let a = MockChecker::new("a", Category::Structure)
            .with_slices(vec![DataSlice::FullHistory]);
        let b = MockChecker::new("b", Category::History)
            .with_slices(vec![DataSlice::FullHistory, DataSlice::WorkingTree]);
        let c = MockChecker::new("c", Category::Deps)
            .with_slices(vec![DataSlice::DependencyFiles]);

        let checkers: [&dyn Checker; 3] = [&a, &b, &c];
        let plans: Vec<ScanPlan> = checkers.iter().map(|c| c.plan(&ctx)).collect();
        let merged = merge_plans(&plans);
        assert_eq!(
            merged.slices(),
            &[
                DataSlice::FullHistory,
                DataSlice::WorkingTree,
                DataSlice::DependencyFiles
            ]
        );
    }

    /// 同一 mock 对同一上下文连续两次 plan 结果等价（幂等，spec 5.2.1.4）。
    #[test]
    fn plan声明幂等_两次调用等价() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let mock = MockChecker::new("a", Category::Structure).with_slices(vec![
            DataSlice::FullHistory,
            DataSlice::file_contents("*.rs").unwrap(),
        ]);
        assert_eq!(mock.plan(&ctx), mock.plan(&ctx));
    }

    /// 同一实例对相同上下文多次 check 返回等价结论列表（纯函数，spec 5.1.1.3）。
    #[test]
    fn check纯计算_多次调用结果等价() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let ctx = ScanContext::base(&snap, &config);
        let mock = MockChecker::new("a", Category::Structure)
            .with_findings(vec![finding("a.x"), finding("a.y")]);
        assert_eq!(mock.check(&ctx), mock.check(&ctx));
    }
}
