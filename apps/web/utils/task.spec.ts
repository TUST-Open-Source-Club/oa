import { describe, expect, it } from 'vitest'
import { dueBucket, groupTasksByDue } from './task'

const now = new Date('2026-10-30T10:00:00+08:00')

describe('dueBucket', () => {
  it('classifies missing and overdue', () => {
    expect(dueBucket(null, now)).toBe('none')
    expect(dueBucket('2026-10-29T10:00:00+08:00', now)).toBe('overdue')
  })

  it('classifies today, this week and later', () => {
    expect(dueBucket('2026-10-30T18:00:00+08:00', now)).toBe('today')
    expect(dueBucket('2026-11-02T10:00:00+08:00', now)).toBe('week')
    expect(dueBucket('2026-11-20T10:00:00+08:00', now)).toBe('later')
  })
})

describe('groupTasksByDue', () => {
  it('groups tasks in a stable order', () => {
    const groups = groupTasksByDue(
      [
        { id: 'a', dueAt: '2026-10-29T10:00:00+08:00' },
        { id: 'b', dueAt: null },
        { id: 'c', dueAt: '2026-10-30T18:00:00+08:00' },
      ],
      now,
    )
    expect(groups.overdue.map((task) => task.id)).toEqual(['a'])
    expect(groups.today.map((task) => task.id)).toEqual(['c'])
    expect(groups.none.map((task) => task.id)).toEqual(['b'])
    expect(groups.week).toEqual([])
  })
})
