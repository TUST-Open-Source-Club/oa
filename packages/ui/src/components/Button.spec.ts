import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import Button from './Button.vue'

describe('Button', () => {
  it('渲染插槽内容与默认样式', () => {
    const wrapper = mount(Button, { slots: { default: '保存' } })
    expect(wrapper.text()).toContain('保存')
    expect(wrapper.classes().join(' ')).toContain('bg-primary-600')
    expect(wrapper.attributes('type')).toBe('button')
  })

  it('variant 与 size 影响样式', () => {
    const wrapper = mount(Button, {
      props: { variant: 'danger', size: 'lg' },
      slots: { default: '删除' },
    })
    const classes = wrapper.classes().join(' ')
    expect(classes).toContain('bg-danger-500')
    expect(classes).toContain('h-12')
  })

  it('点击时触发 click 事件', async () => {
    const wrapper = mount(Button, { slots: { default: '提交' } })
    await wrapper.trigger('click')
    expect(wrapper.emitted('click')).toHaveLength(1)
  })

  it('disabled 或 loading 时不触发 click 且按钮禁用', async () => {
    const disabled = mount(Button, { props: { disabled: true }, slots: { default: '禁用' } })
    await disabled.trigger('click')
    expect(disabled.emitted('click')).toBeUndefined()
    expect(disabled.attributes('disabled')).toBeDefined()

    const loading = mount(Button, { props: { loading: true }, slots: { default: '加载' } })
    await loading.trigger('click')
    expect(loading.emitted('click')).toBeUndefined()
    expect(loading.find('[aria-hidden="true"]').exists()).toBe(true)
  })

  it('block 时占满宽度', () => {
    const wrapper = mount(Button, { props: { block: true }, slots: { default: '全宽' } })
    expect(wrapper.classes()).toContain('w-full')
  })
})
