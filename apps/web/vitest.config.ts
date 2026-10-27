import { defineConfig } from 'vitest/config'

/**
 * Web 前端单测配置。
 * 覆盖率范围先聚焦纯逻辑层（stores/composables），随模块落地逐步扩展到组件与页面。
 */
export default defineConfig({
  test: {
    environment: 'node',
    include: ['stores/**/*.spec.ts', 'composables/**/*.spec.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['stores/**/*.ts', 'composables/**/*.ts'],
      exclude: ['**/*.spec.ts'],
      thresholds: {
        lines: 80,
        statements: 80,
        functions: 80,
        branches: 70,
      },
    },
  },
})
