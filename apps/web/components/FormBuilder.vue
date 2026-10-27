<script setup lang="ts">
/** 报名表单设计器：字段增删/排序、类型、必填、选项编辑（对接后端 JSONB schema）。 */
import { Input } from '@club-oa/ui'

/** 选项。 */
interface FieldOption { label: string; value: string }
/** 字段。 */
export interface FormFieldItem {
  key: string
  type: string
  label: string
  required?: boolean
  maxLength?: number
  options?: FieldOption[]
}
/** Schema。 */
export interface FormSchema {
  version?: number
  fields: FormFieldItem[]
}

const props = defineProps<{ modelValue: FormSchema }>()
const emit = defineEmits<{ 'update:modelValue': [value: FormSchema] }>()

const types = [
  { value: 'text', label: '单行文本' },
  { value: 'textarea', label: '多行文本' },
  { value: 'number', label: '数字' },
  { value: 'email', label: '邮箱' },
  { value: 'phone', label: '手机号' },
  { value: 'date', label: '日期' },
  { value: 'radio', label: '单选框' },
  { value: 'multiselect', label: '复选框（多选）' },
  { value: 'checkbox', label: '勾选确认' },
  { value: 'select', label: '下拉选择' },
]
const optionTypes = ['radio', 'multiselect', 'select']

const fields = computed(() => props.modelValue.fields ?? [])

/** 更新并回传。 */
function update(next: FormFieldItem[]) {
  emit('update:modelValue', { ...props.modelValue, fields: next })
}

/** 新增字段。 */
function addField() {
  const index = fields.value.length + 1
  update([
    ...fields.value,
    { key: `field_${index}`, type: 'text', label: `字段 ${index}`, required: false },
  ])
}

/** 修改字段属性。 */
function patchField(index: number, patch: Partial<FormFieldItem>) {
  const next = fields.value.map((field, i) => (i === index ? { ...field, ...patch } : field))
  update(next)
}

/** 删除字段。 */
function removeField(index: number) {
  update(fields.value.filter((_, i) => i !== index))
}

/** 上移/下移。 */
function moveField(index: number, offset: number) {
  const target = index + offset
  if (target < 0 || target >= fields.value.length) return
  const next = [...fields.value]
  const [item] = next.splice(index, 1)
  next.splice(target, 0, item)
  update(next)
}

/** 切换类型时补/清选项。 */
function changeType(index: number, type: string) {
  const field = fields.value[index]
  if (optionTypes.includes(type)) {
    patchField(index, {
      type,
      options: field.options?.length ? field.options : [{ label: '选项 1', value: 'option_1' }],
    })
  } else {
    patchField(index, { type })
  }
}

/** 选项增删改。 */
function addOption(index: number) {
  const field = fields.value[index]
  const options = field.options ?? []
  patchField(index, { options: [...options, { label: `选项 ${options.length + 1}`, value: `option_${options.length + 1}` }] })
}
function patchOption(index: number, optionIndex: number, patch: Partial<FieldOption>) {
  const field = fields.value[index]
  const options = (field.options ?? []).map((option, i) => (i === optionIndex ? { ...option, ...patch } : option))
  patchField(index, { options })
}
function removeOption(index: number, optionIndex: number) {
  const field = fields.value[index]
  patchField(index, { options: (field.options ?? []).filter((_, i) => i !== optionIndex) })
}
</script>

<template>
  <div class="space-y-3">
    <div
      v-for="(field, index) in fields"
      :key="index"
      class="rounded-[var(--radius-field)] border border-neutral-200 p-3 dark:border-neutral-700"
    >
      <div class="flex flex-wrap items-end gap-2">
        <label class="min-w-32 flex-1">
          <span class="mb-1 block text-xs text-neutral-500">标题</span>
          <Input :model-value="field.label" @update:model-value="patchField(index, { label: $event })" />
        </label>
        <label class="w-28">
          <span class="mb-1 block text-xs text-neutral-500">标识 key</span>
          <Input :model-value="field.key" @update:model-value="patchField(index, { key: $event })" />
        </label>
        <label class="w-36">
          <span class="mb-1 block text-xs text-neutral-500">类型</span>
          <select
            :value="field.type"
            class="h-10 w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-2 text-sm dark:border-neutral-600"
            @change="changeType(index, ($event.target as HTMLSelectElement).value)"
          >
            <option v-for="type in types" :key="type.value" :value="type.value">{{ type.label }}</option>
          </select>
        </label>
        <label class="flex h-10 items-center gap-1.5 text-sm">
          <input
            type="checkbox"
            :checked="field.required"
            @change="patchField(index, { required: ($event.target as HTMLInputElement).checked })"
          />
          必填
        </label>
        <div class="flex h-10 items-center gap-0.5 text-neutral-400">
          <button class="rounded p-1 hover:bg-neutral-100 disabled:opacity-30 dark:hover:bg-neutral-800" :disabled="index === 0" @click="moveField(index, -1)">↑</button>
          <button class="rounded p-1 hover:bg-neutral-100 disabled:opacity-30 dark:hover:bg-neutral-800" :disabled="index === fields.length - 1" @click="moveField(index, 1)">↓</button>
          <button class="rounded p-1 text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10" @click="removeField(index)">✕</button>
        </div>
      </div>
      <div v-if="optionTypes.includes(field.type)" class="mt-2 space-y-1.5 border-t border-dashed border-neutral-200 pt-2 dark:border-neutral-700">
        <div
          v-for="(option, optionIndex) in field.options ?? []"
          :key="optionIndex"
          class="flex items-center gap-2"
        >
          <Input
            class="flex-1"
            :model-value="option.label"
            placeholder="选项显示名"
            @update:model-value="patchOption(index, optionIndex, { label: $event })"
          />
          <Input
            class="w-36"
            :model-value="option.value"
            placeholder="值"
            @update:model-value="patchOption(index, optionIndex, { value: $event })"
          />
          <button class="text-xs text-danger-500 hover:underline" @click="removeOption(index, optionIndex)">删除</button>
        </div>
        <button class="text-xs text-primary-600 hover:underline" @click="addOption(index)">+ 添加选项</button>
      </div>
    </div>

    <button
      class="w-full rounded-[var(--radius-field)] border border-dashed border-neutral-300 py-2 text-sm text-neutral-500 transition hover:border-primary-400 hover:text-primary-600 dark:border-neutral-600"
      @click="addField"
    >
      + 添加字段
    </button>
  </div>
</template>
