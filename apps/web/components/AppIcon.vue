<script setup lang="ts">
/** 内联图标集（24x24 描边），避免引入图标库。 */
const props = withDefaults(defineProps<{ name: string; class?: string }>(), { class: 'size-5' })

const paths: Record<string, string> = {
  home: 'M3 10.5 12 3l9 7.5M5 9.5V21h5v-6h4v6h5V9.5',
  chat: 'M21 12a8 8 0 0 1-8 8H4l2.5-2.5A8 8 0 1 1 21 12Z',
  board: 'M4 4h4v16H4zM10 4h4v10h-4zM16 4h4v13h-4z',
  doc: 'M7 3h7l5 5v13H7zM14 3v5h5',
  calendar: 'M4 6h16v14H4zM4 10h16M8 3v4M16 3v4',
  folder: 'M3 6h6l2 2h10v11H3z',
  bell: 'M6 9a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6M10 20a2 2 0 0 0 4 0',
  settings: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM4 12h2M18 12h2M12 4v2M12 18v2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4',
  plus: 'M12 5v14M5 12h14',
  search: 'M10 17a7 7 0 1 0 0-14 7 7 0 0 0 0 14ZM15 15l6 6',
  send: 'M4 12 20 4l-4 16-4-7zM20 4l-8 9',
  users: 'M8 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM2 21a6 6 0 0 1 12 0M16 3.5a4 4 0 0 1 0 7M18 21a6 6 0 0 0-2-4.5',
  logout: 'M15 4h4v16h-4M10 12H3m0 0 4-4m-4 4 4 4',
  menu: 'M4 6h16M4 12h16M4 18h16',
}
</script>

<template>
  <svg
    :class="props.class"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[props.name] ?? paths.home" />
  </svg>
</template>
