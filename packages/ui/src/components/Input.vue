<script setup lang="ts">
/**
 * 表单输入框：带标签与错误提示，v-model 双向绑定。
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
  'h-10 w-full rounded-[var(--radius-field)] border bg-white px-3 text-sm text-neutral-900 shadow-sm transition-colors',
  'placeholder:text-neutral-400 focus:outline-none focus:ring-2',
  'dark:bg-neutral-800 dark:text-neutral-100',
  props.error
    ? 'border-danger-500 focus:border-danger-500 focus:ring-danger-500/30'
    : 'border-neutral-300 focus:border-primary-500 focus:ring-primary-500/30 dark:border-neutral-600',
  props.disabled ? 'cursor-not-allowed bg-neutral-100 text-neutral-500' : '',
])
</script>

<template>
  <label class="block">
    <span v-if="label" class="mb-1 block text-sm font-medium text-neutral-700 dark:text-neutral-200">
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
    <span v-if="error" role="alert" class="mt-1 block text-xs text-danger-500">{{ error }}</span>
  </label>
</template>
