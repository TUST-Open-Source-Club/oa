/** 在活动筹备看板里加卡片（自动带 eventId）。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  const id = getRouterParam(event, 'id') ?? ''
  const body = (await readBody(event)) as {
    title?: string
    columnId?: string
    descriptionMd?: string
    dueAt?: string | null
  }
  if (!body.title?.trim()) {
    throw createError({ statusCode: 422, statusMessage: 'TITLE_REQUIRED' })
  }
  const config = useRuntimeConfig(event)
  const headers = { authorization }
  type EventDetail = { taskBoardId: string | null }
  const detail = (await $fetch(
    `${config.eventServiceUrl}/api/v1/event/events/${id}`,
    { headers },
  )) as EventDetail
  if (!detail.taskBoardId) {
    throw createError({ statusCode: 422, statusMessage: 'CHECKLIST_NOT_GENERATED' })
  }
  let columnId = body.columnId
  if (!columnId) {
    const columns = (await $fetch(
      `${config.taskServiceUrl}/api/v1/task/boards/${detail.taskBoardId}/columns`,
      { headers },
    )) as Array<{ id: string }>
    columnId = columns[0]?.id
  }
  if (!columnId) throw createError({ statusCode: 502, statusMessage: 'TASK_COLUMN_MISSING' })
  return $fetch(
    `${config.taskServiceUrl}/api/v1/task/columns/${columnId}/tasks`,
    {
      method: 'POST',
      headers,
      body: {
        title: body.title.trim(),
        descriptionMd: body.descriptionMd ?? '',
        dueAt: body.dueAt ?? null,
        eventId: id,
      },
    },
  )
})
