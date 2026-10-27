import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import Card from './Card.vue'

describe('Card', () => {
  it('默认带内边距并渲染插槽', () => {
    const wrapper = mount(Card, { slots: { default: '内容' } })
    expect(wrapper.text()).toContain('内容')
    expect(wrapper.classes()).toContain('p-6')
  })

  it('padded=false 去掉内边距', () => {
    const wrapper = mount(Card, { props: { padded: false } })
    expect(wrapper.classes()).not.toContain('p-6')
  })
})
