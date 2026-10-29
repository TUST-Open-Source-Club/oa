import { describe, expect, it } from 'vitest'
import { attachmentSrc, messagePreview, quoteText, sortConversations } from './im'

const base = {
  id: 'c1',
  type: 'group',
  name: '群聊',
  memberIds: [],
  unread: 0,
  updatedAt: '2026-10-01T10:00:00Z',
}

describe('sortConversations', () => {
  it('puts pinned first, then mentions, then recency', () => {
    const rows = [
      { ...base, id: 'a', updatedAt: '2026-10-03T00:00:00Z' },
      { ...base, id: 'b', updatedAt: '2026-10-02T00:00:00Z', hasMention: true },
      { ...base, id: 'c', updatedAt: '2026-10-01T00:00:00Z', pinned: true },
    ]
    expect(sortConversations(rows).map((row) => row.id)).toEqual(['c', 'b', 'a'])
  })

  it('does not mutate the input', () => {
    const rows = [{ ...base, id: 'a' }]
    sortConversations(rows)
    expect(rows[0]?.id).toBe('a')
  })
})

describe('messagePreview', () => {
  it('renders text, media and system previews', () => {
    expect(messagePreview({ type: 'text', content: { text: '你好' } })).toBe('你好')
    expect(messagePreview({ type: 'image', content: {} })).toBe('[图片]')
    expect(
      messagePreview({
        type: 'file',
        content: { attachments: [{ id: 'f1', name: 'a.pdf', mime: 'application/pdf', url: '' }] },
      }),
    ).toBe('[文件] a.pdf')
    expect(messagePreview({ type: 'file', content: {} })).toBe('[文件]')
    expect(messagePreview({ type: 'video', content: {} })).toBe('[video]')
  })

  it('handles an empty message', () => {
    expect(messagePreview(null)).toBe('暂无消息')
  })
})

describe('quoteText', () => {
  it('summarises the quoted message', () => {
    expect(quoteText({ type: 'text', content: { text: '原文' } })).toBe('原文')
    expect(
      quoteText({ type: 'text', content: { text: '原文' }, status: 'recalled' }),
    ).toBe('该消息已撤回')
    expect(quoteText({ type: 'image', content: {} })).toBe('[image]')
    expect(quoteText(null)).toBe('原消息不可用')
  })
})

describe('attachmentSrc', () => {
  it('rewrites im api prefix to the BFF path', () => {
    expect(attachmentSrc('/api/v1/im/files/x?token=t')).toBe('/api/im/files/x?token=t')
    expect(attachmentSrc('/other/path')).toBe('/other/path')
  })
})
