import { describe, expect, it } from 'vitest'
import { diffLines } from './text-diff'

describe('diffLines', () => {
  it('marks unchanged lines as same', () => {
    expect(diffLines('a\nb', 'a\nb')).toEqual([
      { kind: 'same', text: 'a' },
      { kind: 'same', text: 'b' },
    ])
  })

  it('detects added lines', () => {
    expect(diffLines('a', 'a\nb')).toEqual([
      { kind: 'same', text: 'a' },
      { kind: 'add', text: 'b' },
    ])
  })

  it('detects removed lines', () => {
    expect(diffLines('a\nb', 'a')).toEqual([
      { kind: 'same', text: 'a' },
      { kind: 'remove', text: 'b' },
    ])
  })

  it('renders a modified line as remove + add', () => {
    expect(diffLines('旧内容', '新内容')).toEqual([
      { kind: 'remove', text: '旧内容' },
      { kind: 'add', text: '新内容' },
    ])
  })

  it('treats empty before as all additions', () => {
    expect(diffLines('', 'a\nb')).toEqual([
      { kind: 'add', text: 'a' },
      { kind: 'add', text: 'b' },
    ])
  })

  it('keeps context around changes', () => {
    expect(diffLines('a\nb\nc', 'a\nx\nc')).toEqual([
      { kind: 'same', text: 'a' },
      { kind: 'remove', text: 'b' },
      { kind: 'add', text: 'x' },
      { kind: 'same', text: 'c' },
    ])
  })
})
