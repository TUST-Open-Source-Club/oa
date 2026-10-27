/**
 * 报名表单 schema 的类型与规整函数。
 *
 * 后端把表单设计存成 JSONB，返回结构不受前端约束；这里集中做一次
 * 「未知 JSON → FormSchema」的收敛，避免各页面重复写类型断言。
 */

/** 表单选项。 */
export interface FieldOption {
  label: string
  value: string
}

/** 表单字段。 */
export interface FormFieldItem {
  key: string
  type: string
  label: string
  required?: boolean
  maxLength?: number
  options?: FieldOption[]
}

/** 表单 schema。 */
export interface FormSchema {
  version?: number
  fields: FormFieldItem[]
}

/** 是否为普通对象（排除数组与 null）。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** 规整选项数组：丢弃空 label/value 的脏数据，空数组按「无选项」处理。 */
function toOptions(raw: unknown): FieldOption[] | undefined {
  if (!Array.isArray(raw)) return undefined
  const options = raw
    .filter(isRecord)
    .map((item) => ({ label: String(item.label ?? ''), value: String(item.value ?? '') }))
    .filter((item) => item.label !== '' || item.value !== '')
  return options.length > 0 ? options : undefined
}

/** 规整单个字段：缺失 key/type/label 时补默认值，非对象条目丢弃。 */
function toField(raw: unknown, index: number): FormFieldItem | null {
  if (!isRecord(raw)) return null
  const key = typeof raw.key === 'string' && raw.key !== '' ? raw.key : `field_${index + 1}`
  const type = typeof raw.type === 'string' && raw.type !== '' ? raw.type : 'text'
  const label = typeof raw.label === 'string' && raw.label !== '' ? raw.label : key
  const field: FormFieldItem = { key, type, label }
  if (typeof raw.required === 'boolean') field.required = raw.required
  if (typeof raw.maxLength === 'number') field.maxLength = raw.maxLength
  const options = toOptions(raw.options)
  if (options) field.options = options
  return field
}

/**
 * 把后端返回值规整为 `FormSchema`。
 *
 * 兼容两种形态：`{ schema: { version, fields } }` 包装（活动详情接口），
 * 以及直接 `{ version, fields }`。无法识别的输入返回空字段列表。
 */
export function toFormSchema(raw: unknown): FormSchema {
  const source = isRecord(raw) && isRecord(raw.schema) ? raw.schema : raw
  if (!isRecord(source)) return { fields: [] }
  const version = typeof source.version === 'number' ? source.version : undefined
  const fields = Array.isArray(source.fields)
    ? source.fields
        .map((item, index) => toField(item, index))
        .filter((item): item is FormFieldItem => item !== null)
    : []
  return version === undefined ? { fields } : { version, fields }
}
