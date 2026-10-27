<script setup lang="ts">
/**
 * Milkdown 所见即所得编辑器（仅客户端）：Typora 风格工具栏 + 正文排版。
 * 通过 getMarkdown() 暴露最新 Markdown；初始化失败时 emit failed 供父级回退。
 */
import { onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import '@milkdown/kit/prose/view/style/prosemirror.css'

const props = withDefaults(defineProps<{ modelValue: string; readonly?: boolean }>(), { readonly: false })
const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'failed'): void
}>()

const root = ref<HTMLElement>()
const toolbar = ref<HTMLElement>()
const failed = ref(false)
const ready = ref(false)
const editor = shallowRef<{ destroy: () => Promise<void>; action: (fn: unknown) => void } | null>(null)
let latest = props.modelValue
let callCommand: ((command: unknown, payload?: unknown) => unknown) | null = null
let commands: Record<string, { key: unknown }> = {}

onMounted(async () => {
  if (!root.value) return
  try {
    const [{ Editor, rootCtx, defaultValueCtx, editorViewOptionsCtx }, { commonmark }, { listener, listenerCtx }, utils, preset] =
      await Promise.all([
        import('@milkdown/kit/core'),
        import('@milkdown/kit/preset/commonmark'),
        import('@milkdown/kit/plugin/listener'),
        import('@milkdown/kit/utils'),
        import('@milkdown/kit/preset/commonmark'),
      ])
    callCommand = utils.callCommand as typeof callCommand
    commands = {
      bold: preset.toggleStrongCommand,
      italic: preset.toggleEmphasisCommand,
      code: preset.toggleInlineCodeCommand,
      h1: preset.wrapInHeadingCommand,
      h2: preset.wrapInHeadingCommand,
      h3: preset.wrapInHeadingCommand,
      bullet: preset.wrapInBulletListCommand,
      ordered: preset.wrapInOrderedListCommand,
      quote: preset.wrapInBlockquoteCommand,
      codeBlock: preset.createCodeBlockCommand,
      hr: preset.insertHrCommand,
    }
    const instance = await Editor.make()
      .config((ctx) => {
        ctx.set(rootCtx, root.value as HTMLElement)
        ctx.set(defaultValueCtx, props.modelValue)
        ctx.set(editorViewOptionsCtx, { editable: () => !props.readonly })
        ctx.get(listenerCtx).markdownUpdated((_, markdown) => {
          latest = markdown
          emit('update:modelValue', markdown)
        })
      })
      .use(commonmark)
      .use(listener)
      .create()
    editor.value = instance as typeof editor.value
    ready.value = true
  } catch (error) {
    console.error('Milkdown 初始化失败', error)
    failed.value = true
    emit('failed')
  }
})

onBeforeUnmount(async () => {
  await editor.value?.destroy()
  editor.value = null
})

/** 触发格式化命令。 */
function run(name: string, payload?: unknown) {
  const command = commands[name]
  if (!command || !callCommand) return
  editor.value?.action(callCommand(command.key, payload))
}

const tools = [
  { name: 'bold', label: 'B', class: 'font-bold', payload: undefined },
  { name: 'italic', label: 'I', class: 'italic', payload: undefined },
  { name: 'code', label: '</>', class: 'font-mono text-xs', payload: undefined },
  { name: 'h1', label: 'H1', class: 'font-semibold', payload: 1 },
  { name: 'h2', label: 'H2', class: 'font-semibold', payload: 2 },
  { name: 'h3', label: 'H3', class: 'font-semibold', payload: 3 },
  { name: 'bullet', label: '• 列表', class: '', payload: undefined },
  { name: 'ordered', label: '1. 列表', class: '', payload: undefined },
  { name: 'quote', label: '❝', class: '', payload: undefined },
  { name: 'codeBlock', label: '{ }', class: 'font-mono text-xs', payload: undefined },
  { name: 'hr', label: '—', class: '', payload: undefined },
]

defineExpose({
  getMarkdown: () => latest ?? props.modelValue,
  failed,
})
</script>

<template>
  <div class="milkdown-shell overflow-hidden rounded-[var(--radius-field)] border border-neutral-200 bg-white dark:border-neutral-700 dark:bg-neutral-900">
    <div
      v-if="ready && !readonly"
      ref="toolbar"
      class="flex flex-wrap items-center gap-0.5 border-b border-neutral-200 bg-neutral-50 px-2 py-1.5 dark:border-neutral-700 dark:bg-neutral-800/60"
    >
      <button
        v-for="tool in tools"
        :key="tool.name"
        type="button"
        class="min-w-7 rounded px-1.5 py-1 text-sm text-neutral-600 transition hover:bg-white hover:text-primary-600 hover:shadow-sm dark:text-neutral-300 dark:hover:bg-neutral-700"
        :class="tool.class"
        :title="tool.name"
        @mousedown.prevent="run(tool.name, tool.payload)"
      >
        {{ tool.label }}
      </button>
    </div>
    <div
      v-if="!failed"
      ref="root"
      class="milkdown-content min-h-[24rem] px-8 py-6"
    />
    <div v-else class="p-6 text-sm text-neutral-400">编辑器加载失败，请刷新页面或使用下方纯文本模式。</div>
  </div>
</template>

<style>
/* Typora 风格正文排版 */
.milkdown-content .ProseMirror {
  outline: none;
  min-height: 22rem;
  font-size: 15px;
  line-height: 1.75;
  color: #1f2937;
}
.milkdown-content .ProseMirror > * + * {
  margin-top: 0.75em;
}
.milkdown-content .ProseMirror h1 {
  font-size: 1.75rem;
  font-weight: 700;
  line-height: 1.3;
  margin-top: 1.2em;
}
.milkdown-content .ProseMirror h2 {
  font-size: 1.4rem;
  font-weight: 650;
  margin-top: 1.1em;
}
.milkdown-content .ProseMirror h3 {
  font-size: 1.15rem;
  font-weight: 600;
  margin-top: 1em;
}
.milkdown-content .ProseMirror h4 {
  font-size: 1rem;
  font-weight: 600;
}
.milkdown-content .ProseMirror p {
  margin: 0.4em 0;
}
.milkdown-content .ProseMirror ul,
.milkdown-content .ProseMirror ol {
  padding-left: 1.4em;
}
.milkdown-content .ProseMirror ul {
  list-style: disc;
}
.milkdown-content .ProseMirror ol {
  list-style: decimal;
}
.milkdown-content .ProseMirror blockquote {
  border-left: 3px solid #a5b4fc;
  padding-left: 0.9em;
  color: #4b5563;
  background: #eef2ff;
  border-radius: 0 var(--radius-field) var(--radius-field) 0;
  padding-top: 0.3em;
  padding-bottom: 0.3em;
}
.milkdown-content .ProseMirror code {
  background: #f3f4f6;
  border-radius: 4px;
  padding: 0.15em 0.35em;
  font-size: 0.9em;
}
.milkdown-content .ProseMirror pre {
  background: #111827;
  color: #e5e7eb;
  border-radius: var(--radius-field);
  padding: 0.8em 1em;
  overflow-x: auto;
}
.milkdown-content .ProseMirror pre code {
  background: transparent;
  color: inherit;
  padding: 0;
}
.milkdown-content .ProseMirror a {
  color: #4f46e5;
  text-decoration: underline;
}
.milkdown-content .ProseMirror hr {
  border: none;
  border-top: 1px solid #e5e7eb;
  margin: 1.5em 0;
}
.milkdown-content .ProseMirror .ProseMirror-selectednode {
  outline: 2px solid #818cf8;
}
.dark .milkdown-content .ProseMirror {
  color: #e5e7eb;
}
.dark .milkdown-content .ProseMirror blockquote {
  background: #312e81;
  color: #c7d2fe;
}
.dark .milkdown-content .ProseMirror code {
  background: #374151;
}
</style>
