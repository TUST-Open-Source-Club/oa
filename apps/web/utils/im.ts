/** IM 页面的纯逻辑（便于单测，页面只做渲染与请求）。 */

/** 消息内容里用到的字段（宽松类型，兼容接口返回）。 */
export interface ImAttachment {
  id: string
  name: string
  mime: string
  url: string
}

/** 消息。 */
export interface ImMessage {
  id?: string
  type?: string
  content?: { text?: string; attachments?: ImAttachment[] }
  status?: string
}

/** 会话。 */
export interface ImConversation {
  id: string
  pinned?: boolean
  hasMention?: boolean
  updatedAt: string
}

/** 会话排序：置顶 > 未读 @ > 更新时间倒序。 */
export function sortConversations<T extends ImConversation>(rows: T[]): T[] {
  return [...rows].sort((left, right) => {
    if (Boolean(left.pinned) !== Boolean(right.pinned)) return left.pinned ? -1 : 1
    if (Boolean(left.hasMention) !== Boolean(right.hasMention)) return left.hasMention ? -1 : 1
    return new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime()
  })
}

/** 会话列表里一条记录的预览文案。 */
export function messagePreview(message: ImMessage | null | undefined): string {
  if (!message) return '暂无消息'
  if (message.type === 'text') return message.content?.text ?? ''
  if (message.type === 'image') return '[图片]'
  if (message.type === 'file') {
    const name = message.content?.attachments?.[0]?.name
    return name ? `[文件] ${name}` : '[文件]'
  }
  return `[${message.type ?? '未知'}]`
}

/** 引用块/引用条的单行摘要。 */
export function quoteText(message: ImMessage | null | undefined): string {
  if (!message) return '原消息不可用'
  if (message.status === 'recalled') return '该消息已撤回'
  if (message.type === 'text') return message.content?.text ?? ''
  return `[${message.type ?? '未知'}]`
}

/** 附件地址改写为同源 BFF 路径（`<img>` 可直接引用）。 */
export function attachmentSrc(url: string): string {
  return url.replace('/api/v1/im/', '/api/im/')
}
