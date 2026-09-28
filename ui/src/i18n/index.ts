import { ref, computed } from 'vue'

export type Locale = 'zh' | 'en'

const STORAGE_KEY = 'rb-locale'

const zh: Record<string, string> = {
  'app.name': '仓衡 RepoBalance',
  'app.sub': 'Git 仓库健康度体检台 · 纯本地 · 只诊断不改仓库',
  'app.localBadge': '🔒 纯本地零网络',
  'app.switchRepo': '🔄 更换仓库',
  'app.history': '📜 历史',
  'app.step1': '① 接入仓库',
  'app.step2': '② 体检扫描',
  'app.step3': '③ 报告仪表盘',
  'app.step4': '④ 历史对比',
  'app.langToggle': 'EN',

  'connect.title': '选择要体检的 Git 仓库',
  'connect.desc': '路径仅来自系统目录选择对话框，应用不会直接触达文件系统之外的数据',
  'connect.selectDir': '选择目录…',
  'connect.demoHint': '没有仓库？试试演示仓库',
  'connect.demoName': 'demo-repo',
  'connect.demoSuffix': '（内置三类合成问题数据）',
  'connect.reselDir': '重新选择目录',
  'connect.commitCount': '提交总数',
  'connect.branchCount': '分支数',
  'connect.workdirFiles': '工作区文件',
  'connect.depManifests': '依赖清单',
  'connect.resel': '重新选择',
  'connect.startScan': '开始体检 →',

  'config.title': '检查器配置',
  'config.bigFiles': '大文件检查器',
  'config.threshold': '大文件阈值',
  'config.hint': '配置保存在本地（localStorage），下次启动自动恢复',
  'config.ignoreHint': '在仓库根目录创建',
  'config.ignoreHint2': '可忽略特定路径（语法同 .gitignore）',

  'scan.title': '正在体检',
  'scan.back': '← 返回重新选择',
  'scan.cancel': '✕ 取消扫描',
  'scan.cancelling': '取消请求已发送…',
  'scan.preparing': '正在准备扫描…',
  'scan.stage': '阶段',
  'scan.stage.snapshot': '初始化',
  'scan.stage.workdir': '工作区采集',
  'scan.stage.history': '历史采集',
  'scan.stage.check': '检查',
  'scan.stage.done': '完成',
  'scan.checkerRegistered': '已注册检查器：big-files（stub 阶段无实时进度）',
  'scan.feedDone': '完成 · finding',
  'scan.feedRunning': '运行中…',
  'scan.feedPending': '待运行',

  'report.health': '🟢 健康',
  'report.needsImprove': '🟡 需改善',
  'report.needsAttention': '🔴 需要关注',
  'report.summary': '仓库健康总评',
  'report.scoreLabel': '五维平均分（0~100）',
  'report.cancelled': ' · 扫描已取消 · 显示已完成部分',
  'report.findings': '发现的问题',
  'report.findingCount': '条',
  'report.partial': '（部分结果）',
  'report.noFinding': '该筛选条件下无发现 · 维度健康',
  'report.evidence': '证据',
  'report.mask': '🔒 掩码展示 · 导出同掩码',
  'report.suggestion': '修复建议',
  'report.export': '导出报告（evidence 保持掩码）',
  'report.exported': '已导出',
  'report.back': '← 重新选择仓库',
  'report.dimCount': '条',

  'filter.all': '全部',
  'filter.critical': '严重',
  'filter.warning': '警告',
  'filter.info': '提示',
  'filter.structure': '结构',
  'filter.history': '历史',
  'filter.branches': '分支',
  'filter.deps': '依赖',
  'filter.security': '安全',

  'history.title': '扫描历史与对比',
  'history.count': '共',
  'history.countUnit': '条（最多保留 20 条）',
  'history.clearAll': '清空全部',
  'history.empty': '暂无扫描历史。',
  'history.emptyHint': '完成一次扫描后，报告会自动保存到此处，可用于跨次对比。',
  'history.findingUnit': '项发现',
  'history.baseline': '基准',
  'history.target': '对照',
  'history.delete': '删除',
  'history.compareResult': '对比结果',
  'history.clearSel': '清除选择',
  'history.score': '得分',
  'history.scoreDelta': '总分变化',
  'history.findingDelta': '发现项变化',
  'history.added': '新增',
  'history.resolved': '消除',
  'history.common': '仍存在',
  'history.addedFindings': '新增发现',
  'history.resolvedFindings': '已消除',
  'history.commonFindings': '仍存在',
  'history.noDiff': '两次扫描的发现项完全一致，无差异。',
  'history.hintSelect': '请再选择一条作为',
  'history.hintGen': '，即可生成对比结果。',
  'history.sameRecord': '基准与对照不能为同一条记录。',
  'history.back': '← 返回',
}

const en: Record<string, string> = {
  'app.name': 'RepoBalance',
  'app.sub': 'Git Repo Health Checker · Local Only · Diagnose Without Modifying',
  'app.localBadge': '🔒 Local Zero-Network',
  'app.switchRepo': '🔄 Switch Repo',
  'app.history': '📜 History',
  'app.step1': '① Connect',
  'app.step2': '② Scan',
  'app.step3': '③ Dashboard',
  'app.step4': '④ Compare',
  'app.langToggle': '中文',

  'connect.title': 'Select a Git Repository to Scan',
  'connect.desc': 'Path comes only from the system directory picker; the app never reaches beyond the file system',
  'connect.selectDir': 'Select Directory…',
  'connect.demoHint': 'No repo? Try the demo repo',
  'connect.demoName': 'demo-repo',
  'connect.demoSuffix': ' (built-in synthetic issue data)',
  'connect.reselDir': 'Reselect Directory',
  'connect.commitCount': 'Commits',
  'connect.branchCount': 'Branches',
  'connect.workdirFiles': 'Workdir Files',
  'connect.depManifests': 'Dep Manifests',
  'connect.resel': 'Reselect',
  'connect.startScan': 'Start Scan →',

  'config.title': 'Checker Configuration',
  'config.bigFiles': 'Big Files Checker',
  'config.threshold': 'Big File Threshold',
  'config.hint': 'Saved locally (localStorage); restored on next launch',
  'config.ignoreHint': 'Create',
  'config.ignoreHint2': 'in the repo root to ignore paths (gitignore syntax)',

  'scan.title': 'Scanning',
  'scan.back': '← Back to Selection',
  'scan.cancel': '✕ Cancel Scan',
  'scan.cancelling': 'Cancel request sent…',
  'scan.preparing': 'Preparing scan…',
  'scan.stage': 'Stage',
  'scan.stage.snapshot': 'Snapshot',
  'scan.stage.workdir': 'Workdir',
  'scan.stage.history': 'History',
  'scan.stage.check': 'Check',
  'scan.stage.done': 'Done',
  'scan.checkerRegistered': 'Registered checker: big-files (stub phase, no real-time progress)',
  'scan.feedDone': 'Done · finding',
  'scan.feedRunning': 'Running…',
  'scan.feedPending': 'Pending',

  'report.health': '🟢 Healthy',
  'report.needsImprove': '🟡 Needs Improvement',
  'report.needsAttention': '🔴 Needs Attention',
  'report.summary': 'Repository Health Summary',
  'report.scoreLabel': 'Five-dimension Average (0~100)',
  'report.cancelled': ' · Scan cancelled · Showing partial results',
  'report.findings': 'Findings',
  'report.findingCount': '',
  'report.partial': ' (partial)',
  'report.noFinding': 'No findings under this filter · Dimension healthy',
  'report.evidence': 'Evidence',
  'report.mask': '🔒 Masked · Export preserves masking',
  'report.suggestion': 'Suggestion',
  'report.export': 'Export Report (evidence stays masked)',
  'report.exported': 'Exported',
  'report.back': '← Reselect Repository',
  'report.dimCount': '',

  'filter.all': 'All',
  'filter.critical': 'Critical',
  'filter.warning': 'Warning',
  'filter.info': 'Info',
  'filter.structure': 'Structure',
  'filter.history': 'History',
  'filter.branches': 'Branches',
  'filter.deps': 'Deps',
  'filter.security': 'Security',

  'history.title': 'Scan History & Compare',
  'history.count': '',
  'history.countUnit': ' entries (max 20 retained)',
  'history.clearAll': 'Clear All',
  'history.empty': 'No scan history yet.',
  'history.emptyHint': 'After completing a scan, the report is saved here for cross-scan comparison.',
  'history.findingUnit': ' findings',
  'history.baseline': 'Baseline',
  'history.target': 'Target',
  'history.delete': 'Delete',
  'history.compareResult': 'Comparison Result',
  'history.clearSel': 'Clear Selection',
  'history.score': 'Score',
  'history.scoreDelta': 'Score Delta',
  'history.findingDelta': 'Finding Delta',
  'history.added': 'Added',
  'history.resolved': 'Resolved',
  'history.common': 'Common',
  'history.addedFindings': 'Added Findings',
  'history.resolvedFindings': 'Resolved',
  'history.commonFindings': 'Still Present',
  'history.noDiff': 'Both scans produced identical findings; no differences.',
  'history.hintSelect': 'Select another entry as ',
  'history.hintGen': ' to generate the comparison.',
  'history.sameRecord': 'Baseline and target cannot be the same record.',
  'history.back': '← Back',
}

const dicts: Record<Locale, Record<string, string>> = { zh, en }

function loadLocale(): Locale {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw === 'zh' || raw === 'en') return raw
  } catch {
    /* localStorage 不可用 */
  }
  return 'zh'
}

const locale = ref<Locale>(loadLocale())

export function setLocale(l: Locale) {
  locale.value = l
  try {
    localStorage.setItem(STORAGE_KEY, l)
  } catch {
    /* 忽略持久化失败 */
  }
}

export function toggleLocale() {
  setLocale(locale.value === 'zh' ? 'en' : 'zh')
}

export function t(key: string): string {
  return dicts[locale.value][key] ?? key
}

export function useI18n() {
  return {
    locale: computed(() => locale.value),
    t,
    setLocale,
    toggleLocale,
  }
}
