use crate::error::Result;
use crate::model::finding::Finding;
use crate::model::snapshot::RepoSnapshot;

/// 单个检查器：输入仓库快照，输出 findings。
pub trait Checker {
    /// 稳定标识，如 `history.merge-commit-ratio`。
    fn id(&self) -> &'static str;

    /// 所属维度。
    fn category(&self) -> crate::model::finding::Category;

    /// 执行检查。
    fn check(&self, snapshot: &RepoSnapshot) -> Result<Vec<Finding>>;
}

/// 检查器注册表：持有全部 Checker 实例，供扫描编排调用。
#[derive(Default)]
pub struct CheckerRegistry {
    checkers: Vec<Box<dyn Checker + Send + Sync>>,
}

impl CheckerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个检查器。
    pub fn register(&mut self, checker: Box<dyn Checker + Send + Sync>) -> &mut Self {
        self.checkers.push(checker);
        self
    }

    /// 已注册检查器数量。
    pub fn len(&self) -> usize {
        self.checkers.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.checkers.is_empty()
    }

    /// 遍历全部检查器。
    pub fn iter(&self) -> impl Iterator<Item = &(dyn Checker + Send + Sync)> {
        self.checkers.iter().map(|b| b.as_ref())
    }
}
