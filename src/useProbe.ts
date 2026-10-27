/**
 * 两类窗口共享的只读快照与交互状态。并发控制最终由 Rust 执行，前端防抖仅改善反馈。
 * @author ahui
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { call, subscribe } from './bridge';
import type { AppSnapshot, TestRecord } from './types';

/**
 * 为当前窗口建立快照订阅，监听注册完成后再加载，避免丢失启动期更新。
 * @param options 快照范围；includeHistory 默认 false，只读取当前条件优先匹配的一条记录，设置页传 true 读取全部保留历史。
 * @returns 快照、加载状态、当前结果和手动测试入口。
 */
export function useProbe(options: { includeHistory?: boolean } = {}) {
    const snapshot = ref<AppSnapshot | null>(null);
    const error = ref('');
    const pending = ref(false);
    // 与快照一起记录本地单调时钟；隐藏窗口恢复后仍可按真实经过时间计算倒计时。
    const snapshotReceivedAt = ref(performance.now());
    let unsubscribe: (() => void) | undefined;
    let disposed = false;
    let refreshGeneration = 0;

    /**
     * 旧刷新结果及错误不能覆盖新刷新结果；关闭窗口后忽略异步回调。
     * @returns 本次是否成功应用快照；false 表示读取失败、已卸载或被较新刷新取代。
     * 保存调用方据此区分“提交失败”与“已提交但读取失败”，避免重试产生重复配置。
     */
    async function refresh(): Promise<boolean> {
        const generation = ++refreshGeneration;
        try {
            const next = await call<AppSnapshot>('get_snapshot', {
                includeHistory: Boolean(options.includeHistory),
            });
            if (!disposed && generation === refreshGeneration) {
                snapshotReceivedAt.value = performance.now();
                snapshot.value = next;
                return true;
            }
        } catch (cause) {
            if (!disposed && generation === refreshGeneration) error.value = String(cause);
        }
        return false;
    }

    onMounted(async () => {
        try {
            const cleanup = await subscribe(() => void refresh());
            if (disposed) cleanup();
            else unsubscribe = cleanup;
            await refresh();
        } catch (cause) {
            if (!disposed) error.value = String(cause);
        }
    });
    onUnmounted(() => {
        disposed = true;
        unsubscribe?.();
    });

    const profile = computed(() =>
        snapshot.value?.config.profiles.find((item) => item.id === snapshot.value?.config.selectedProfileId),
    );
    const latestRecord = computed<TestRecord | null>(() => {
        const id = profile.value?.id;
        if (!id) return null;
        const records = snapshot.value?.history ?? [];
        const fingerprint = snapshot.value?.conditionFingerprints?.[id];
        // 桌面精简快照由后端查找，完整历史/测试快照在这里采用相同优先级；旧记录不猜测条件。
        // 此记录可用于展示/复制历史回答；当前结论必须另经 latest 的完整条件核对，不能混用。
        return (
            records.find(
                (item) => item.profileId === id && fingerprint && item.conditionFingerprint === fingerprint,
            ) ??
            records.find((item) => item.profileId === id) ??
            null
        );
    });
    const latest = computed<TestRecord | null>(() => {
        const record = latestRecord.value;
        const currentProfile = profile.value;
        if (!record || !currentProfile) return null;
        // 后端以规范化请求地址、模型、规则、协议、强度和有效超时生成指纹。
        // 前端不重复实现规范化，也不猜测旧记录缺失的请求条件；重命名不影响指纹。
        const fingerprint = snapshot.value?.conditionFingerprints?.[currentProfile.id];
        return fingerprint && record.conditionFingerprint === fingerprint ? record : null;
    });
    const busy = computed(() => pending.value || Boolean(snapshot.value?.running));

    /** 前端只提交一次；后端按模型开关最多回退一次协议，错误/取消绝不记为降智。 */
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

    /**
     * 请求取消后仍等待后端结算，避免旧任务写回后覆盖新任务。
     * @returns 是否收到取消命令确认；失败时允许用户再次取消，不锁死操作按钮。
     */
    async function cancel(): Promise<boolean> {
        try {
            await call('cancel_test');
            return true;
        } catch (cause) {
            error.value = String(cause);
            return false;
        }
    }

    const stale = computed(() => Boolean(latestRecord.value && !latest.value));
    return {
        snapshot,
        snapshotReceivedAt,
        error,
        pending,
        profile,
        latest,
        latestRecord,
        stale,
        busy,
        refresh,
        start,
        cancel,
    };
}

/**
 * 将有时区的 ISO 时间按用户系统时区显示；非法历史时间不导致界面崩溃。
 * @param value 带时区的 ISO 时间；undefined 或空串表示尚未测试。
 * @returns yyyy-MM-dd HH:mm:ss 格式的本机 24 小时时间；缺失时为“尚未测试”，无效时为“时间不可用”。
 */
export function formatTime(value: string | undefined): string {
    if (!value) return '尚未测试';
    const time = new Date(value);
    if (Number.isNaN(time.getTime())) return '时间不可用';
    // 显式拼接以固定分隔符与 00 点表示，避免系统区域设置输出斜杠或 24:00。
    const pad = (part: number): string => String(part).padStart(2, '0');
    return `${String(time.getFullYear()).padStart(4, '0')}-${pad(time.getMonth() + 1)}-${pad(time.getDate())} ${pad(time.getHours())}:${pad(time.getMinutes())}:${pad(time.getSeconds())}`;
}

/**
 * 表达结果时效，防止自动测试关闭后把上次结果误读为实时状态。
 * @param value 带时区的测试开始时间；缺失表示未测试。
 * @param now 当前系统毫秒时间；时钟回拨时按刚刚测试显示，不产生负时间。
 * @returns 简短相对时间，供完整日期时间的悬停说明补充结果时效。
 */
export function formatRelativeTime(value: string | undefined, now: number): string {
    if (!value) return '尚未测试';
    const started = Date.parse(value);
    if (!Number.isFinite(started)) return '时间不可用';
    const seconds = Math.max(0, (now - started) / 1000);
    if (seconds < 60) return '刚刚测试';
    if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
    return `${Math.floor(seconds / 86400)} 天前`;
}
