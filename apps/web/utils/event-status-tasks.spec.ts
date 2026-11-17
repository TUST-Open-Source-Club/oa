import { describe, expect, it } from 'vitest'
import { eventStatusTasks } from './event-status-tasks'

describe('eventStatusTasks', () => {
  it('adds a review task when registration closes', () => {
    const tasks = eventStatusTasks('closed')
    expect(tasks).toHaveLength(1)
    expect(tasks[0]!.title).toBe('审核报名名单')
  })

  it('adds wrap-up tasks when the event finishes', () => {
    const titles = eventStatusTasks('finished').map((task) => task.title)
    expect(titles).toEqual(['导出报名数据', '整理活动照片', '活动复盘'])
  })

  it('adds nothing for open or draft', () => {
    expect(eventStatusTasks('open')).toEqual([])
    expect(eventStatusTasks('draft')).toEqual([])
  })
})
