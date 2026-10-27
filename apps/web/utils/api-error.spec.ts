import { describe, expect, it } from 'vitest'

import { apiErrorMessage, problemOf } from './api-error'

describe('problemOf', () => {
  it('直接返回 ProblemDetails 形态的 data', () => {
    const problem = { code: 'AUTH_INVALID', detail: '账号或密码错误' }

    expect(problemOf({ data: problem })).toEqual(problem)
  })

  it('兼容 H3 createError 的 data.data 嵌套', () => {
    const problem = { detail: '激活链接已过期' }

    expect(problemOf({ data: { data: problem } })).toEqual(problem)
  })

  it('无 data / 非对象 / 空对象返回 null', () => {
    expect(problemOf(null)).toBeNull()
    expect(problemOf(undefined)).toBeNull()
    expect(problemOf(new Error('boom'))).toBeNull()
    expect(problemOf({ data: 'oops' })).toBeNull()
    expect(problemOf({ data: { unrelated: true } })).toBeNull()
  })
})

describe('apiErrorMessage', () => {
  it('优先使用业务 detail', () => {
    expect(apiErrorMessage({ data: { detail: '用户名已存在', title: '冲突' } })).toBe('用户名已存在')
  })

  it('没有 detail 时退回 title', () => {
    expect(apiErrorMessage({ data: { title: '请求过于频繁' } })).toBe('请求过于频繁')
  })

  it('都没有时使用兜底文案（可自定义）', () => {
    expect(apiErrorMessage(new Error('network'))).toBe('请求失败，请稍后重试')
    expect(apiErrorMessage(undefined, '保存失败')).toBe('保存失败')
  })
})
