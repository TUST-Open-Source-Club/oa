import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

/**
 * 设计系统测试配置：jsdom 环境 + v8 覆盖率 + 80% 阈值门禁。
 */
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.spec.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.{ts,vue}'],
      exclude: ['src/index.ts', 'src/**/*.spec.ts'],
      thresholds: {
        lines: 80,
        statements: 80,
        functions: 80,
        branches: 70,
      },
    },
  },
})
