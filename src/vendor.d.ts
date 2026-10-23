/** 已核对 markdown-it-task-lists 2.1.1 的导出与选项；社区无独立类型包。@author ahui */
declare module 'markdown-it-task-lists' {
    import type { PluginWithOptions } from 'markdown-it';
    const taskLists: PluginWithOptions<{ enabled?: boolean; label?: boolean; labelAfter?: boolean }>;
    export default taskLists;
}
