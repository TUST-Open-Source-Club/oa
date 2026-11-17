import { describe, expect, it } from 'vitest'
import { buildChecklist, EVENT_CHECKLIST_TEMPLATES, findChecklistTemplate } from './event-templates'

const start = '2026-11-20T10:00:00+08:00'

describe('event checklist templates', () => {
  it('ships three built-in templates', () => {
    expect(EVENT_CHECKLIST_TEMPLATES.map((item) => item.key)).toEqual(['recruit', 'lecture', 'contest'])
  })

  it('has items in every template', () => {
    for (const template of EVENT_CHECKLIST_TEMPLATES) {
      expect(template.items.length).toBeGreaterThan(0)
    }
  })

  it('falls back to recruit for unknown keys', () => {
    expect(findChecklistTemplate('nope').key).toBe('recruit')
    expect(findChecklistTemplate(null).key).toBe('recruit')
  })

  it('counts due dates backwards from the event start', () => {
    const items = buildChecklist('recruit', start, new Date('2026-10-01T00:00:00Z'))
    const first = items[0]!
    expect(first.title).toBe('确认宣讲场地')
    expect(new Date(first.dueAt).toISOString()).toBe(
      new Date(new Date(start).getTime() - 7 * 24 * 3600 * 1000).toISOString(),
    )
    const last = items[items.length - 1]!
    expect(new Date(last.dueAt).toISOString()).toBe(new Date(start).toISOString())
  })

  it('uses now when the event has no start time', () => {
    const now = new Date('2026-10-01T00:00:00Z')
    const items = buildChecklist('lecture', null, now)
    expect(new Date(items[0]!.dueAt).toISOString()).toBe(
      new Date(now.getTime() - 10 * 24 * 3600 * 1000).toISOString(),
    )
  })
})
