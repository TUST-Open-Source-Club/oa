import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import Input from './Input.vue'

describe('Input', () => {
  it('渲染标签并展示绑定值', () => {
    const wrapper = mount(Input, { props: { label: '邮箱', modelValue: 'a@b.cn' } })
    expect(wrapper.text()).toContain('邮箱')
    expect(wrapper.find('input').element.value).toBe('a@b.cn')
  })

  it('输入时触发 update:modelValue', async () => {
    const wrapper = mount(Input)
    await wrapper.find('input').setValue('hello')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['hello'])
  })

  it('错误提示与 aria-invalid', () => {
    const wrapper = mount(Input, { props: { error: '格式错误' } })
    expect(wrapper.text()).toContain('格式错误')
    expect(wrapper.find('input').attributes('aria-invalid')).toBe('true')
    expect(wrapper.find('input').classes().join(' ')).toContain('border-danger-500')
  })

  it('禁用状态', () => {
    const wrapper = mount(Input, { props: { disabled: true } })
    expect(wrapper.find('input').attributes('disabled')).toBeDefined()
  })
})
