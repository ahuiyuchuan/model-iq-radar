/** 只读业务到动作的映射；不发请求、不修改模型结论或自动测试期限。@author ahui */
import { ACTIONS, type PetAction } from './types';
import type { AppSnapshot, TestRecord } from '../types';

/** 进程内观察到任务后才处理结算，阻止恢复历史/换分组时重复庆祝。 */
export class ResultFeedback {
    private run: string | null = null;
    private consumed = new Set<string>();

    /**
     * @param snapshot 后端权威快照；null 表示尚未读取。
     * @param record 当前条件精确匹配记录；过期结果传 null。
     * @returns 新结算记录的状态，或 null（恢复历史、未完成、重复事件）。
     */
    update(snapshot: AppSnapshot | null, record: TestRecord | null): TestRecord['status'] | null {
        const active = snapshot?.running?.id;
        if (active) {
            this.run = active;
            return null;
        }
        if (!snapshot || !this.run) return null;
        const run = this.run;
        this.run = null;
        if (!record || record.id !== run || this.consumed.has(record.id)) return null;
        this.consumed.add(record.id);
        if (this.consumed.size > 64) this.consumed.delete(this.consumed.values().next().value!);
        return record.status;
    }
}

/** 显示动作优先级独立于业务状态；临时操作结束后重新计算，不记忆过期的返回动作。 */
export function chooseAction(
    dragging: boolean,
    transient: PetAction | null,
    busy: boolean,
    panel: boolean,
    result: PetAction | null = null,
    idle: PetAction = 'idle',
): PetAction {
    // 展开后固定侧坐接触关系，结果由常驻云朵表达，避免每次测试把人物从卡片上拔起。
    // 面板存在时连标题栏拖动也保持侧坐；拖动所有权仍由输入状态机管理，不依赖悬空动作。
    return panel ? 'seated' : dragging ? 'dragging' : (transient ?? (busy ? 'working' : (result ?? idle)));
}

/**
 * 解析用户选择的空闲外观，允许当前皮肤提供的全部动作；只返回姿态，不派发业务或输入事件。
 * @param requested 已保存的动作名；旧配置缺字段或非法值回退 idle。
 * @param available 当前皮肤实际提供的动作名，换肤缺资源时回退 idle。
 * @returns 可用动作或基础待机；实际测试状态和拖拽仍由上层优先级接管。
 */
export function resolveIdleAction(requested: unknown, available: string[]): PetAction {
    return ACTIONS.includes(requested as PetAction) && available.includes(requested as string)
        ? (requested as PetAction)
        : 'idle';
}

/** 非通过状态保持中性/准确提示，HTML 和取消不冒充模型通过。 */
export const FEEDBACK_TEXT: Record<TestRecord['status'], string> = {
    passed: '不降智',
    failed: '降智',
    error: '测试失败，请查看详情',
    cancelled: '已取消',
    html: '已生成 HTML，点击查看',
};

/** 云朵只读业务结果，临时互动不得改写文案或把 HTML/取消显示为模型能力结论。 */
export interface PetResult {
    key: 'running' | TestRecord['status'] | 'stale' | 'idle';
    label: string;
    detail: string;
    tone: 'blue' | 'green' | 'amber' | 'red' | 'neutral';
    action: PetAction | null;
}

/**
 * @param busy 当前权威任务是否进行中，优先于旧记录。
 * @param record 已按当前模型和条件过滤的记录；不在此函数重新判题。
 * @param stale 条件变化后存在旧记录，但不能继续显示为当前结论。
 * @returns 真实状态的文案、图标键、色彩和可选静态表情姿态。
 */
export function petResult(busy: boolean, record: TestRecord | null, stale: boolean): PetResult {
    if (busy)
        return { key: 'running', label: '测试中', detail: '点击查看进度', tone: 'blue', action: 'working' };
    if (stale)
        return { key: 'stale', label: '待重新测试', detail: '测试条件已变化', tone: 'neutral', action: null };
    if (!record)
        return { key: 'idle', label: '尚未测试', detail: '点击打开测智面板', tone: 'blue', action: null };
    const states: Record<TestRecord['status'], Omit<PetResult, 'key'>> = {
        passed: { label: '不降智', detail: '命中当前测试规则', tone: 'green', action: 'success' },
        failed: { label: '降智', detail: '未命中当前测试规则', tone: 'amber', action: 'failed' },
        error: { label: '测试失败', detail: '点击查看错误详情', tone: 'red', action: 'error' },
        cancelled: { label: '已取消', detail: '本次未产生测试结论', tone: 'neutral', action: null },
        html: { label: 'HTML 回答', detail: '点击查看，暂无能力结论', tone: 'blue', action: null },
    };
    return { key: record.status, ...states[record.status] };
}

/** 空闲动作仅从已提供资源中选择，避免连续重复；不借用通过庆祝表达自主活动。
 * @param available 皮肤实际提供的动作名，不能以缺资源回退冒充新动作。
 * @param previous 上次自主动作，首轮为空。
 * @param roll 0—1 的随机样本；外部传入方便用确定性用例验证选择与优先级。
 * @returns 下一个自主动作；没有可用资源时返回 null，继续普通待机。
 */
export function nextAmbient(available: string[], previous: PetAction | null, roll: number): PetAction | null {
    const pool = (['wave', 'stretch', 'seated', 'bow'] as PetAction[]).filter((a) => available.includes(a));
    const candidates = pool.length > 1 ? pool.filter((a) => a !== previous) : pool;
    return candidates.length
        ? candidates[Math.min(candidates.length - 1, Math.max(0, Math.floor(roll * candidates.length)))]
        : null;
}
