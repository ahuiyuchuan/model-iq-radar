/** 本地开发仅监听回环地址，桌面生产构建不依赖开发服务器。@author ahui */
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vitest/config';

export default defineConfig({
    plugins: [vue()],
    clearScreen: false,
    server: {
        host: '127.0.0.1',
        port: 1420,
        strictPort: true,
        // Rust 构建会锁住 Windows EXE；前端无需监听原生产物，避免开发预览被 EBUSY 中断。
        watch: { ignored: ['**/src-tauri/target/**', '**/release-artifacts/**'] },
    },
    build: { target: 'es2022' },
    test: { environment: 'jsdom', include: ['tests/**/*.test.ts'] },
});
