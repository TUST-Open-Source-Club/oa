import { describe, expect, it } from 'vitest'

import { toFormSchema } from './form-schema'

describe('toFormSchema', () => {
  it('规整完整字段（含选项与可选属性）', () => {
    const schema = toFormSchema({
      version: 3,
      fields: [
        {
          key: 'name',
          type: 'text',
          label: '姓名',
          required: true,
          maxLength: 20,
        },
        {
          key: 'team',
          type: 'select',
          label: '队伍',
          options: [
            { label: 'A', value: 'a' },
            { label: '', value: '' },
          ],
        },
      ],
    })

    expect(schema.version).toBe(3)
    expect(schema.fields).toHaveLength(2)
    expect(schema.fields[0]).toEqual({
      key: 'name',
      type: 'text',
      label: '姓名',
      required: true,
      maxLength: 20,
    })
    // 空 label/value 的选项被丢弃
    expect(schema.fields[1]?.options).toEqual([{ label: 'A', value: 'a' }])
  })

  it('兼容 { schema: {...} } 包装形态', () => {
    const schema = toFormSchema({ schema: { fields: [{ key: 'k', label: '标签' }] } })

    expect(schema.version).toBeUndefined()
    expect(schema.fields).toEqual([{ key: 'k', type: 'text', label: '标签' }])
  })

  it('缺失字段补默认值，脏数据被丢弃', () => {
    const schema = toFormSchema({
      fields: [{ type: 'email' }, null, 'oops', { key: 'x', type: '', label: '' }, { key: 'y' }],
    })

    expect(schema.fields).toEqual([
      { key: 'field_1', type: 'email', label: 'field_1' },
      { key: 'x', type: 'text', label: 'x' },
      { key: 'y', type: 'text', label: 'y' },
    ])
  })

  it('非对象输入与缺失 fields 返回空列表', () => {
    expect(toFormSchema(undefined)).toEqual({ fields: [] })
    expect(toFormSchema('nope')).toEqual({ fields: [] })
    expect(toFormSchema({ version: 1 })).toEqual({ version: 1, fields: [] })
  })

  it('空选项数组按无选项处理', () => {
    const schema = toFormSchema({ fields: [{ key: 'k', label: 'l', options: [] }] })

    expect(schema.fields[0]?.options).toBeUndefined()
  })
})
