/**
 * 两类窗口共享的只读快照与交互状态。并发控制最终由 Rust 执行，前端防抖仅改善反馈。
 * @author ahui
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { call, subscribe } from './bridge';
import type { AppSnapshot, TestRecord } from './types';

/**
 * 为当前窗口建立快照订阅，监听注册完成后再加载，避免丢失启动期更新。
 * @returns 快照、加载状态、当前结果和手动测试入口。
 */
export function useProbe() {
    const snapshot = ref<AppSnapshot | null>(null);
    const error = ref('');
    const pending = ref(false);
    // 与快照一起记录本地单调时钟；隐藏窗口恢复后仍可按真实经过时间计算倒计时。
    const snapshotReceivedAt = ref(performance.now());
    let unsubscribe: (() => void) | undefined;
    let disposed = false;
    let refreshGeneration = 0;

    /** 旧刷新结果不能覆盖新刷新结果；关闭窗口后忽略异步回调。 */
    async function refresh(): Promise<void> {
        const generation = ++refreshGeneration;
        try {
            const next = await call<AppSnapshot>('get_snapshot');
            if (!disposed && generation === refreshGeneration) {
                snapshotReceivedAt.value = performance.now();
                snapshot.value = next;
            }
        } catch (cause) {
            if (!disposed) error.value = String(cause);
        }
    }

    onMounted(async () => {
        try {
            const cleanup = await subscribe(() => void refresh());
            if (disposed) cleanup();
            else unsubscribe = cleanup;
            await refresh();
        } catch (cause) {
            error.value = String(cause);
        }
    });
    onUnmounted(() => {
        disposed = true;
        unsubscribe?.();
    });

    const profile = computed(() =>
        snapshot.value?.config.profiles.find((item) => item.id === snapshot.value?.config.selectedProfileId),
    );
    const latest = computed<TestRecord | null>(
        () => snapshot.value?.history.find((item) => item.profileId === profile.value?.id) ?? null,
    );
    const busy = computed(() => pending.value || Boolean(snapshot.value?.running));

    /** 不自动重试；后端保存错误/取消记录，但绝不将其记为未通过。 */
    async function start(): Promise<void> {
        if (busy.value) return;
        error.value = '';
        pending.value = true;
        try {
            await call('start_test');
        } catch (cause) {
            error.value = String(cause);
        } finally {
            pending.value = false;
            await refresh();
        }
    }

    /** 请求取消后仍等待后端结算，避免旧任务写回后覆盖新任务。 */
    async function cancel(): Promise<void> {
        try {
            await call('cancel_test');
        } catch (cause) {
            error.value = String(cause);
        }
    }

    return { snapshot, snapshotReceivedAt, error, pending, profile, latest, busy, refresh, start, cancel };
}

/** 将有时区的 ISO 时间按用户系统时区显示；非法历史时间不导致界面崩溃。 */
export function formatTime(value: string | undefined): string {
    if (!value) return '尚未测试';
    const time = new Date(value);
    if (Number.isNaN(time.getTime())) return '时间不可用';
    return new Intl.DateTimeFormat('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
    }).format(time);
}
