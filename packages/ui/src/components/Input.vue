<script setup lang="ts">
/**
 * 表单输入框：带标签与错误提示，v-model 双向绑定。
 *
 * 32px 高度与按钮对齐；聚焦时用柔和的外圈，不用强投影。
 */
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    /** 绑定值 */
    modelValue?: string
    /** 标签 */
    label?: string
    /** 占位符 */
    placeholder?: string
    /** 错误提示（存在时高亮边框） */
    error?: string
    /** 输入类型 */
    type?: string
    /** 是否禁用 */
    disabled?: boolean
    /** 自动完成提示 */
    autocomplete?: string
  }>(),
  { modelValue: '', label: '', placeholder: '', error: '', type: 'text', disabled: false, autocomplete: 'off' },
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()

/** 输入时向上同步字符串值。 */
function onInput(event: Event) {
  const target = event.target as HTMLInputElement
  emit('update:modelValue', target.value)
}

const inputClasses = computed(() => [
  'h-8 w-full rounded-[var(--radius-field)] border bg-[var(--surface)] px-2.5 text-[13px] text-neutral-900 transition-colors',
  'placeholder:text-neutral-400 focus:outline-none focus:ring-2',
  'dark:text-neutral-100',
  props.error
    ? 'border-danger-500 focus:border-danger-500 focus:ring-danger-500/25'
    : 'border-[var(--border-strong)] focus:border-primary-500 focus:ring-primary-500/25',
  props.disabled ? 'cursor-not-allowed bg-[var(--surface-muted)] text-neutral-500' : '',
])
</script>

<template>
  <label class="block">
    <span v-if="label" class="mb-1.5 block text-[12px] font-medium text-neutral-600 dark:text-neutral-300">
      {{ label }}
    </span>
    <input
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :autocomplete="autocomplete"
      :class="inputClasses"
      :aria-invalid="Boolean(error)"
      @input="onInput"
    />
    <span v-if="error" role="alert" class="mt-1 block text-[11px] text-danger-500">{{ error }}</span>
  </label>
</template>
