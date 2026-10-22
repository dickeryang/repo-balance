import { describe, expect, it } from 'vitest'
import { fmtIgnoreBytes, ignoredPercent, isIgnoreDirty, userRuleCount } from './ignore'

describe('忽略规则编辑器纯函数', () => {
  it('ignoredPercent_空仓库返回0而不是NaN', () => {
    expect(ignoredPercent(0, 0)).toBe(0)
    expect(ignoredPercent(5, 0)).toBe(0)
  })

  it('ignoredPercent_保留一位小数且整除时不带小数尾巴', () => {
    expect(ignoredPercent(1, 2)).toBe(50)
    expect(ignoredPercent(1, 3)).toBe(33.3)
    expect(ignoredPercent(2, 3)).toBe(66.7)
    expect(ignoredPercent(0, 10)).toBe(0)
  })

  it('fmtIgnoreBytes_按量级切换单位', () => {
    expect(fmtIgnoreBytes(512)).toBe('512 B')
    expect(fmtIgnoreBytes(2048)).toBe('2 KiB')
    expect(fmtIgnoreBytes(1_048_576)).toBe('1.0 MiB')
    expect(fmtIgnoreBytes(2 * 1024 * 1_048_576)).toBe('2.0 GiB')
  })

  it('isIgnoreDirty_逐字比较_尾部换行也算改动', () => {
    expect(isIgnoreDirty('a\n', 'a\n')).toBe(false)
    expect(isIgnoreDirty('a\n', 'a')).toBe(true)
    expect(isIgnoreDirty('', '')).toBe(false)
  })

  it('userRuleCount_跳过空行与注释且兼容CRLF', () => {
    expect(userRuleCount('# 注释\n\n  \n*.bin\n/dist/\n')).toBe(2)
    expect(userRuleCount('# 注释\r\n*.bin\r\n')).toBe(1)
    expect(userRuleCount('')).toBe(0)
  })
})
