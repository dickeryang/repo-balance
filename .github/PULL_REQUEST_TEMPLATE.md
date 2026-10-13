## 改动动机

<!-- 为什么改：关联的 issue、发现的缺陷或需求背景 -->

## 方案

<!-- 怎么改：关键实现与取舍；涉及新增检查器请说明切片声明与评分维度 -->

## 验证

- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets --workspace` 零 warning
- [ ] `cargo test --workspace` 全绿（新增检查器需覆盖：命中 / 不命中 / 边界阈值 / 忽略规则）
- [ ] `cd ui && npm run lint && npm run build` 通过（前端文案已同步 zh/en 字典）
- [ ] `cargo build --release -p rb-core && target/release/rb scan . --ci` 无 Critical 结论

## 影响面

<!-- 是否影响分层边界（rb-core 不得引入 tauri / std::fs）、公开 API、报告格式或持久化数据 -->

## 备注

<!-- 合并后由维护者同一提交双端推送（AtomGit + GitHub）保持同步 -->
