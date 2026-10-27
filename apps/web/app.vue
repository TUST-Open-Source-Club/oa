<script setup lang="ts">
/** 应用根组件：将通知未读数同步到桌面壳托盘/Dock 角标。 */
const notifications = useNotificationsStore()
const { setUnreadCount } = useShell()

watch(
  () => notifications.unreadCount,
  (count) => {
    void setUnreadCount(count)
  },
  { immediate: true },
)
</script>

<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>
