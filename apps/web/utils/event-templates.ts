/** 活动筹备清单模板：内置 3 套，生成时按活动开始时间倒推截止日期。 */

/** 模板条目。 */
export interface ChecklistTemplateItem {
  /** 卡片标题。 */
  title: string
  /** 描述。 */
  descriptionMd: string
  /** 相对活动开始时间提前的天数。 */
  dueOffsetDays: number
  /** 优先级。 */
  priority: string
}

/** 模板。 */
export interface ChecklistTemplate {
  /** key。 */
  key: string
  /** 名称。 */
  name: string
  /** 条目。 */
  items: ChecklistTemplateItem[]
}

/** 内置模板。 */
export const EVENT_CHECKLIST_TEMPLATES: ChecklistTemplate[] = [
  {
    key: 'recruit',
    name: '招新',
    items: [
      { title: '确认宣讲场地', descriptionMd: '联系场地、确认时间与设备。', dueOffsetDays: 7, priority: 'high' },
      { title: '海报与推文', descriptionMd: '设计海报，发布招新推文。', dueOffsetDays: 5, priority: 'normal' },
      { title: '报名表单测试', descriptionMd: '走一遍报名、审核、签到流程。', dueOffsetDays: 3, priority: 'high' },
      { title: '采购物料', descriptionMd: '桌牌、签到表、周边。', dueOffsetDays: 2, priority: 'normal' },
      { title: '现场签到表', descriptionMd: '打印签到二维码与备用名单。', dueOffsetDays: 1, priority: 'normal' },
      { title: '复盘与归档', descriptionMd: '导出报名数据，整理照片与复盘结论。', dueOffsetDays: 0, priority: 'low' },
    ],
  },
  {
    key: 'lecture',
    name: '讲座',
    items: [
      { title: '讲师确认', descriptionMd: '确认讲师、主题与时间。', dueOffsetDays: 10, priority: 'high' },
      { title: '场地与设备', descriptionMd: '投影、麦克风、录播。', dueOffsetDays: 5, priority: 'high' },
      { title: '宣传与报名', descriptionMd: '发布通知并开放报名。', dueOffsetDays: 3, priority: 'normal' },
      { title: '签到与反馈表', descriptionMd: '准备签到二维码与反馈问卷。', dueOffsetDays: 1, priority: 'normal' },
    ],
  },
  {
    key: 'contest',
    name: '比赛',
    items: [
      { title: '赛制与评委', descriptionMd: '确定赛制、评分表与评委名单。', dueOffsetDays: 14, priority: 'high' },
      { title: '报名与分组', descriptionMd: '收集报名、完成分组与对阵。', dueOffsetDays: 7, priority: 'high' },
      { title: '场地布置', descriptionMd: '座位、背景板、指示牌。', dueOffsetDays: 2, priority: 'normal' },
      { title: '物资与奖品', descriptionMd: '奖杯、证书、饮用水。', dueOffsetDays: 2, priority: 'normal' },
      { title: '现场执行清单', descriptionMd: '流程表、计时、应急联系人。', dueOffsetDays: 1, priority: 'high' },
      { title: '结果公示', descriptionMd: '发布成绩与照片。', dueOffsetDays: 0, priority: 'low' },
    ],
  },
]

/** 按 key 找模板（未知 key 回退到招新）。 */
export function findChecklistTemplate(key: string | null | undefined): ChecklistTemplate {
  return (
    EVENT_CHECKLIST_TEMPLATES.find((item) => item.key === key) ??
    (EVENT_CHECKLIST_TEMPLATES[0] as ChecklistTemplate)
  )
}

/** 生成清单：按活动开始时间倒推截止时间。 */
export function buildChecklist(
  key: string | null | undefined,
  startAt: string | null | undefined,
  now: Date = new Date(),
): Array<ChecklistTemplateItem & { dueAt: string }> {
  const template = findChecklistTemplate(key)
  const start = startAt ? new Date(startAt) : now
  const base = Number.isNaN(start.getTime()) ? now : start
  return template.items.map((item) => {
    const due = new Date(base.getTime() - item.dueOffsetDays * 24 * 3600 * 1000)
    return { ...item, dueAt: due.toISOString() }
  })
}
