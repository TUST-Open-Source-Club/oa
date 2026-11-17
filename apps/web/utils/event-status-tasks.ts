/** 活动状态变化时自动补建的任务。 */

/** 状态联动任务。 */
export interface EventStatusTask {
  /** 标题。 */
  title: string
  /** 描述。 */
  descriptionMd: string
  /** 优先级。 */
  priority: string
}

/** 报名截止/结束后的收尾任务。 */
const STATUS_TASKS: Record<string, EventStatusTask[]> = {
  closed: [
    {
      title: '审核报名名单',
      descriptionMd: '核对报名信息，处理待审核、候补与拒绝。',
      priority: 'high',
    },
  ],
  finished: [
    {
      title: '导出报名数据',
      descriptionMd: '导出报名与签到数据，归档到网盘。',
      priority: 'normal',
    },
    {
      title: '整理活动照片',
      descriptionMd: '挑选照片、压缩后归档。',
      priority: 'low',
    },
    {
      title: '活动复盘',
      descriptionMd: '记录人数、问题与改进项，输出复盘结论。',
      priority: 'normal',
    },
  ],
}

/** 状态对应的自动任务（open/draft 无）。 */
export function eventStatusTasks(status: string): EventStatusTask[] {
  return STATUS_TASKS[status] ?? []
}
