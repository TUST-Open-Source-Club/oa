<template>
  <!-- 登录/激活等未登录页面：居中卡片布局 -->
  <main class="flex min-h-screen items-center justify-center bg-neutral-100 px-4 dark:bg-neutral-950">
    <div class="w-full max-w-md">
      <h1 class="mb-6 text-center text-2xl font-semibold text-primary-700 dark:text-primary-300">
        {{ $config.public.appName }}
      </h1>
      <slot />
    </div>
  </main>
</template>
