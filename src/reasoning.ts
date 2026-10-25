/**
 * 思考强度选项的保守能力提示，只限制已核实的精确模型 ID。
 *
 * 依据 OpenAI 官方模型页（2026-10-25 核实）：
 * https://developers.openai.com/api/docs/models/gpt-5.1
 * https://developers.openai.com/api/docs/models/gpt-5.2
 * GPT-5.1 列出 none/low/medium/high；GPT-5.2 另外支持 xhigh。
 * 本版本只实现 low/medium/high/xhigh 和“不指定参数”，不把不指定混同于 API 的 none。
 * /models 不声明推理能力，不能猜测未知模型、代理别名、pro/codex 等变体的能力。
 * @author ahui
 */
import type { ModelProfile } from './types';

type Effort = NonNullable<ModelProfile['reasoningEffort']>;

/** 与现有深色 ModelPicker 共用的数据结构，空 id 表示不发送推理参数。 */
export interface ReasoningOption {
    id: string;
    name: string;
    model: string;
}

/** 能力信息只指导用户选档，不触发请求、改写草稿或替用户自动换档。 */
export interface ReasoningSupport {
    options: ReasoningOption[];
    notice: string;
    /** 精确 ID 和协议均已核实；不表示兼容服务商承诺接受请求。 */
    recognized: boolean;
    /** null 表示未知能力；false 表示当前保存值不在已核实标准范围。 */
    currentSupported: boolean | null;
}

const COMMON_EFFORTS: readonly Effort[] = ['low', 'medium', 'high', 'xhigh'];
const STANDARD_EFFORTS: readonly Effort[] = ['low', 'medium', 'high'];
const KNOWN_MODELS: Readonly<Record<string, readonly Effort[]>> = {
    'gpt-5.1': STANDARD_EFFORTS,
    'gpt-5.1-2025-11-13': STANDARD_EFFORTS,
    'gpt-5.2': COMMON_EFFORTS,
    'gpt-5.2-2025-12-11': COMMON_EFFORTS,
};

/**
 * 获取该模型的可见档位与兼容提示；纯函数，无网络、持久化或表单副作用。
 *
 * @param model 用户实际发送的模型 ID；只 trim 首尾空白，保留大小写和服务商命名。
 * @param interfaceMode 所选协议；旧配置缺省按 Chat Completions，未知协议不猜能力。
 * @param current 当前草稿值；不支持的已有值仍保留一项并提示，禁止静默切成 low。
 * @returns 下拉选项和能力判断；未知模型显示常用档位并明确由服务商决定。
 */
export function reasoningSupport(
    model: string,
    interfaceMode: ModelProfile['interfaceMode'],
    current: ModelProfile['reasoningEffort'],
): ReasoningSupport {
    const protocol = interfaceMode ?? 'chat_completions';
    const id = model.trim();
    // 模型名来自不可信接口列表，不能把 toString/__proto__ 等继承键当作能力定义。
    const known =
        ['chat_completions', 'responses'].includes(protocol) && Object.hasOwn(KNOWN_MODELS, id)
            ? KNOWN_MODELS[id]
            : undefined;
    const efforts = known ?? COMMON_EFFORTS;
    const options: ReasoningOption[] = [
        { id: '', name: '不指定', model: '' },
        ...efforts.map((effort) => ({ id: effort, name: effort, model: '' })),
    ];
    const currentSupported = current == null ? true : known ? known.includes(current) : null;
    if (current != null && !efforts.includes(current)) {
        options.push({ id: current, name: `${current}（当前值，官方不支持）`, model: '' });
    }
    const notice = known
        ? `OpenAI 标准模型支持 ${known.join('、')}；兼容服务仍以服务商声明为准。${currentSupported === false ? '当前值不在已核实范围，请明确重新选择，软件不会自动换档。' : ''}`
        : '该模型未核实具体档位，以下为常用选项；/models 不提供思考能力信息，请按服务商支持选择，不支持时选“不指定”。';
    return {
        options,
        notice: `${notice}“不指定”表示不发送参数，不等同于关闭推理；请求失败不会换档重试。`,
        recognized: known != null,
        currentSupported,
    };
}
