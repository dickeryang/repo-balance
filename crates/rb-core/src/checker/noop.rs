//! 占位检查器：插件契约的最小实现示范，可作为新检查器的复制起点。

use crate::checker::{Checker, ScanContext, ScanPlan};
use crate::model::finding::{Category, Finding};

/// 占位检查器：无状态、零切片声明、零结论产出。
pub struct NoopChecker;

impl Checker for NoopChecker {
    /// 保留标识：供注册表唯一性校验拦截后续冲突注册（spec 5.5.3.1）。
    fn id(&self) -> &'static str {
        "noop"
    }

    /// 占位取值，无业务语义。
    fn category(&self) -> Category {
        Category::Structure
    }

    /// 恒返回零切片：占位实现不消费任何数据。
    fn plan(&self, _ctx: &ScanContext) -> ScanPlan {
        ScanPlan::new()
    }

    /// 恒返回零结论：占位实现不产出任何 finding。
    fn check(&self, _ctx: &ScanContext) -> Vec<Finding> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::{CheckerRegistry, ScanConfig};
    use crate::model::snapshot::RepoSnapshot;

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

    /// 注册 NoopChecker 成功后 plan 返回零切片、check 返回零结论，
    /// 注册→规划→检查链路全程无错误（spec 5.5.1.1）。
    #[test]
    fn 占位检查器_注册规划检查链路_全程无错误() {
        let snap = snapshot();
        let config = ScanConfig::default();
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(NoopChecker)).unwrap();
        let ctx = ScanContext::base(&snap, &config);
        let checker = &registry.list()[0];
        assert_eq!(checker.id(), "noop");
        assert!(checker.plan(&ctx).is_empty());
        assert!(checker.check(&ctx).is_empty());
    }

    /// 再次注册任意 id 为 noop 的实例被 DuplicateChecker 拦截（保留标识生效）。
    #[test]
    fn 保留标识noop_冲突注册被拦截() {
        let mut registry = CheckerRegistry::new();
        registry.register(Box::new(NoopChecker)).unwrap();
        let err = registry.register(Box::new(NoopChecker)).unwrap_err();
        assert!(err.to_string().contains("noop"));
        assert_eq!(registry.len(), 1);
    }
}