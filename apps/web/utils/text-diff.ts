/** 逐行差异（LCS），供文档版本对比使用。 */
export type DiffKind = 'same' | 'add' | 'remove'

/** 一行差异。 */
export interface DiffLine {
  kind: DiffKind
  text: string
}

/** 拆分文本为行（忽略末尾空行）。 */
function splitLines(text: string): string[] {
  if (!text) return []
  const lines = text.split('\n')
  if (lines.length && lines[lines.length - 1] === '') lines.pop()
  return lines
}

/** 计算 before → after 的逐行差异。 */
export function diffLines(before: string, after: string): DiffLine[] {
  const left = splitLines(before)
  const right = splitLines(after)
  const rows = left.length
  const cols = right.length
  // LCS 动态规划表
  const table: number[][] = Array.from({ length: rows + 1 }, () =>
    new Array<number>(cols + 1).fill(0),
  )
  for (let i = rows - 1; i >= 0; i -= 1) {
    for (let j = cols - 1; j >= 0; j -= 1) {
      table[i]![j] =
        left[i] === right[j]
          ? table[i + 1]![j + 1]! + 1
          : Math.max(table[i + 1]![j]!, table[i]![j + 1]!)
    }
  }
  const result: DiffLine[] = []
  let i = 0
  let j = 0
  while (i < rows && j < cols) {
    if (left[i] === right[j]) {
      result.push({ kind: 'same', text: left[i]! })
      i += 1
      j += 1
    } else if (table[i + 1]![j]! >= table[i]![j + 1]!) {
      result.push({ kind: 'remove', text: left[i]! })
      i += 1
    } else {
      result.push({ kind: 'add', text: right[j]! })
      j += 1
    }
  }
  while (i < rows) {
    result.push({ kind: 'remove', text: left[i]! })
    i += 1
  }
  while (j < cols) {
    result.push({ kind: 'add', text: right[j]! })
    j += 1
  }
  return result
}
