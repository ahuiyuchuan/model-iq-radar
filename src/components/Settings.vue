<!--
    独立设置窗口：接口、规则、桌面偏好与可追溯历史；草稿仅在明确保存后生效。
    @author ahui
-->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import {
    Target,
    SlidersHorizontal,
    Monitor,
    History,
    ListChecks,
    Minus,
    Square,
    X,
    Eye,
    EyeOff,
    Save,
    Plus,
    Trash2,
    Check,
    ArrowUpRight,
    Search,
    RotateCcw,
    Info,
    Clock3,
    ChevronRight,
    KeyRound,
    CircleAlert,
    Copy,
    Zap,
} from 'lucide-vue-next';
import { call, DEFAULT_PROMPT, desktop, dragWindow, windowAction } from '../bridge';
import { formatTime, useProbe } from '../useProbe';
import type {
    ModelProfile,
    Preferences,
    ProfileInput,
    TestRecord,
    TestRule,
    UpdateResult,
    FetchedModel,
    ConnectionResult,
} from '../types';
import ModelPicker from './ModelPicker.vue';
import MarkdownAnswer from './MarkdownAnswer.vue';
import AboutPanel from './AboutPanel.vue';

const { snapshot, busy, error, refresh } = useProbe();
const page = ref('api');
const contentScroll = ref<HTMLElement>();
const saving = ref(false);
const savedMessage = ref('');
const showKey = ref(false);
const editingId = ref<string | null>(null);
const form = reactive<ProfileInput>({
    id: '',
    name: '',
    baseUrl: '',
    model: '',
    apiKey: '',
    removeKey: false,
});
const rule = reactive<TestRule>({ prompt: DEFAULT_PROMPT, keyword: '高市早苗' });
const preferences = reactive<Preferences>({
    alwaysOnTop: true,
    snapToEdge: true,
    opacity: 97,
    timeoutSeconds: 60,
    shortcutEnabled: true,
    autoTestEnabled: true,
    autoTestIntervalMinutes: 3,
});
const sampleAnswer = ref('');
const historySearch = ref('');
const historyGroup = ref('');
const fetchedModels = ref<FetchedModel[]>([]);
const diagnostic = ref<'models' | 'connection' | null>(null);
const diagnosticError = ref('');
const diagnosticMessage = ref('');
let diagnosticGeneration = 0;
const selectedRecord = ref<TestRecord | null>(null);
const confirmDelete = ref(false);
const confirmClear = ref(false);
const restoredDefaults = ref(false);
const init = ref(false);
// 自启是 Windows 实际状态，独立于待保存的悬浮设置，切换不重置自动测试周期。
const autostartEnabled = ref(false);
const autostartReady = ref(false);
const autostartBusy = ref(false);
const autostartError = ref('');
// 更新仅由明确按钮触发，状态留在设置窗口；切页不重复检查，也不占用模型请求门禁。
const checkingUpdate = ref(false);
const updateResult = ref<UpdateResult | null>(null);
const updateError = ref('');
const checkedAt = ref('');

const nav = [
    { id: 'api', label: '接口配置', icon: SlidersHorizontal },
    { id: 'rules', label: '测试规则', icon: ListChecks },
    { id: 'widget', label: '悬浮设置', icon: Monitor },
    { id: 'history', label: '测试历史', icon: History },
    { id: 'about', label: '关于', icon: Info },
];
const selectedProfile = computed(() =>
    snapshot.value?.config.profiles.find((item) => item.id === editingId.value),
);
const matching = computed(() => Boolean(rule.keyword.trim()) && sampleAnswer.value.includes(rule.keyword));
/** 历史按稳定配置 ID 筛选；当前已删除的配置仍以历史快照名称提供筛选。 */
const historyGroups = computed(() => {
    const groups = new Map<string, string>();
    for (const item of snapshot.value?.config.profiles ?? []) groups.set(item.id, item.name);
    for (const record of snapshot.value?.history ?? []) {
        if (!groups.has(record.profileId)) groups.set(record.profileId, record.profileName);
    }
    return [
        { id: '', name: '全部分组', model: '显示所有测试记录' },
        ...Array.from(groups, ([id, name]) => ({
            id,
            name,
            model: `${snapshot.value?.history.filter((r) => r.profileId === id).length ?? 0} 条记录`,
        })),
    ];
});
const catalogOptions = computed(() =>
    fetchedModels.value.map((item) => ({ id: item.id, name: item.id, model: item.ownedBy ?? '可用模型 ID' })),
);
const filteredHistory = computed(() => {
    const query = historySearch.value.toLocaleLowerCase();
    return (
        snapshot.value?.history.filter(
            (item) =>
                (!historyGroup.value || item.profileId === historyGroup.value) &&
                `${item.profileName} ${item.model} ${item.answer} ${item.keyword}`
                    .toLocaleLowerCase()
                    .includes(query),
        ) ?? []
    );
});
watch(historyGroups, (groups) => {
    if (!groups.some((group) => group.id === historyGroup.value)) historyGroup.value = '';
});
// 草稿地址、凭据或目标改变后旧目录不再适用；异步旧响应也不能填回新配置。
watch(
    () => [editingId.value, form.baseUrl, form.apiKey, form.removeKey],
    () => {
        diagnosticGeneration++;
        fetchedModels.value = [];
        diagnosticMessage.value = '';
        diagnosticError.value = '';
    },
    { flush: 'sync' },
);
// 手动改模型时保留同服务目录，但旧连接测试结果不得显示成新模型的成功状态。
watch(
    () => form.model,
    () => {
        if (diagnostic.value === 'connection') diagnosticGeneration++;
        diagnosticMessage.value = '';
        diagnosticError.value = '';
    },
    { flush: 'sync' },
);
const statusText: Record<string, string> = {
    passed: '不降智',
    failed: '降智',
    error: '测试失败',
    cancelled: '已取消',
};

/** 初始化一次草稿，跨窗口刷新只更新快照，避免覆盖用户正在编辑的内容。 */
watch(snapshot, (value) => {
    if (!value || init.value) return;
    init.value = true;
    Object.assign(rule, value.config.rule);
    Object.assign(preferences, value.config.preferences);
    editProfile(value.config.profiles.find((item) => item.id === value.config.selectedProfileId) ?? null);
});
watch(page, () => {
    error.value = '';
    savedMessage.value = '';
    confirmDelete.value = false;
    confirmClear.value = false;
    if (contentScroll.value) contentScroll.value.scrollTop = 0;
    if (page.value === 'widget') void syncAutostart();
});
// 用户从任务管理器返回时重新读取，避免缓存的开关与系统状态不一致。
function refreshAutostartOnFocus(): void {
    if (page.value === 'widget') void syncAutostart();
}
onMounted(() => window.addEventListener('focus', refreshAutostartOnFocus));
onUnmounted(() => window.removeEventListener('focus', refreshAutostartOnFocus));

/** 查询失败不能显示为正常关闭；提供重试入口，不影响其他设置的保存。 */
async function syncAutostart(): Promise<void> {
    if (autostartBusy.value) return;
    autostartBusy.value = true;
    autostartError.value = '';
    try {
        autostartEnabled.value = await call<boolean>('get_autostart');
        autostartReady.value = true;
    } catch (cause) {
        autostartReady.value = false;
        autostartError.value = String(cause);
    } finally {
        autostartBusy.value = false;
    }
}

/** 立即提交系统开关；等待期间去重，失败重新查询，不提前显示成功或保存数据库副本。 */
async function toggleAutostart(event: Event): Promise<void> {
    // 原生 checkbox 会先自行翻转；恢复已确认值，等后端成功才更新，失败不会留假勾选。
    (event.target as HTMLInputElement).checked = autostartEnabled.value;
    if (autostartBusy.value || !autostartReady.value) return;
    autostartBusy.value = true;
    autostartError.value = '';
    try {
        autostartEnabled.value = await call<boolean>('set_autostart', { enabled: !autostartEnabled.value });
    } catch (cause) {
        autostartError.value = String(cause);
        try {
            autostartEnabled.value = await call<boolean>('get_autostart');
        } catch {
            autostartReady.value = false;
        }
    } finally {
        autostartBusy.value = false;
    }
}
// 保存和错误反馈位于主体顶部，跳回可见位置；底部操作栏不参与滚动。
watch([error, savedMessage], ([message, saved]) => {
    if ((message || saved) && contentScroll.value) contentScroll.value.scrollTop = 0;
});

/** 显式切换编辑目标时清空密钥草稿，已有密钥只显示保存状态，不读取明文。 */
function editProfile(profile: ModelProfile | null): void {
    editingId.value = profile?.id ?? null;
    Object.assign(form, {
        id: profile?.id ?? '',
        name: profile?.name ?? '',
        baseUrl: profile?.baseUrl ?? '',
        model: profile?.model ?? '',
        apiKey: '',
        removeKey: false,
    });
    showKey.value = false;
    confirmDelete.value = false;
}

/**
 * 串行执行保存并保留失败草稿；后端再次验证所有字段，前端验证用于即时反馈。
 * @param command 保存命令名。
 * @param args 草稿副本，包含密钥时不写日志。
 */
async function save(command: string, args: Record<string, unknown>): Promise<void> {
    if (saving.value || busy.value) return;
    saving.value = true;
    error.value = '';
    savedMessage.value = '';
    try {
        await call(command, args);
        await refresh();
        if (command === 'save_profile') {
            editProfile(
                snapshot.value?.config.profiles.find(
                    (item) => item.id === snapshot.value?.config.selectedProfileId,
                ) ?? null,
            );
        }
        savedMessage.value = desktop ? '已保存，更改已生效' : '预览已更新；密钥不会保存在浏览器中';
    } catch (cause) {
        error.value = String(cause);
    } finally {
        saving.value = false;
    }
}

/** 输入地址可以为版本根路径或完整 chat/completions 路径，由后端统一规范。 */
async function saveProfile(): Promise<void> {
    if (!form.name.trim() || !form.baseUrl.trim() || !form.model.trim()) {
        error.value = '请填写配置名称、接口地址和模型名称。';
        return;
    }
    await save('save_profile', { input: { ...form } });
}

/** 只拒绝全空规则，不对有意义的前后空格、换行或大小写作隐藏修改。 */
async function saveRule(): Promise<void> {
    if (!rule.prompt.trim() || !rule.keyword.trim()) {
        error.value = '题目和判定字符串均不能为空或只有空白。';
        return;
    }
    await save('save_rule', { rule: { ...rule } });
    restoredDefaults.value = false;
}

/** 恢复仅更改草稿，点击保存才影响后续请求；历史快照保持原规则。 */
function restoreRule(): void {
    Object.assign(rule, { prompt: DEFAULT_PROMPT, keyword: '高市早苗' });
    restoredDefaults.value = true;
}

/** 二次点击确认后删除配置及对应凭据，历史记录仍保留测试时快照。 */
async function deleteProfile(): Promise<void> {
    if (!confirmDelete.value) {
        confirmDelete.value = true;
        return;
    }
    await save('delete_profile', { id: editingId.value });
    if (!error.value) editProfile(snapshot.value?.config.profiles[0] ?? null);
}

/** 清空是明确的破坏性操作，提供行内二次确认，避免模态窗口阻塞悬浮窗。 */
async function clearHistory(): Promise<void> {
    if (!confirmClear.value) {
        confirmClear.value = true;
        return;
    }
    await save('clear_history', {});
    if (!error.value) {
        selectedRecord.value = null;
        confirmClear.value = false;
    }
}

/** 系统窗口操作失败以可见消息呈现，不影响保存的配置。 */
async function control(action: 'close' | 'minimize' | 'maximize'): Promise<void> {
    try {
        await windowAction(action);
    } catch (cause) {
        error.value = String(cause);
    }
}

/** 历史详情复制包含规则快照，方便复核；不包含接口密钥。 */
async function copyRecord(): Promise<void> {
    const record = selectedRecord.value;
    if (!record) return;
    try {
        await navigator.clipboard.writeText(
            `模型：${record.model}\n时间：${record.startedAt}\n判定：${statusText[record.status]}\n判定字符串：${record.keyword}\n题目：\n${record.prompt}\n\n回答：\n${record.answer}\n${record.error ?? ''}`,
        );
        savedMessage.value = '已复制本条测试记录';
    } catch {
        error.value = '复制失败，请手动选择详情中的文本。';
    }
}

/** 查询固定项目的 GitHub 正式发布；失败保留独立反馈，不改写测试快照。 */
async function checkUpdates(): Promise<void> {
    if (checkingUpdate.value) return;
    checkingUpdate.value = true;
    updateError.value = '';
    updateResult.value = null;
    try {
        updateResult.value = await call<UpdateResult>('check_updates');
    } catch (cause) {
        updateError.value = String(cause);
    } finally {
        checkedAt.value = new Date().toLocaleString('zh-CN', { hour12: false });
        checkingUpdate.value = false;
    }
}

/** 关于链接与回答链接共用后端协议白名单；只在用户点击时打开系统浏览器。 */
async function openProjectLink(url: string): Promise<void> {
    try {
        await call('open_external_url', { url });
    } catch (cause) {
        error.value = String(cause);
    }
}

/** 主动诊断使用草稿；无需先保存，留空密钥按后端规则复用原配置，不修改历史与周期。 */
async function runDiagnostic(kind: 'models' | 'connection'): Promise<void> {
    if (diagnostic.value || busy.value || saving.value) return;
    if (!form.baseUrl.trim() || (kind === 'connection' && !form.model.trim())) {
        diagnosticError.value = kind === 'models' ? '请先填写接口地址。' : '请先填写接口地址和模型 ID。';
        return;
    }
    const generation = ++diagnosticGeneration;
    diagnostic.value = kind;
    diagnosticError.value = '';
    diagnosticMessage.value = '';
    try {
        const args = { input: { ...form } };
        if (kind === 'models') {
            const models = await call<FetchedModel[]>('fetch_models', args);
            if (generation !== diagnosticGeneration) return;
            fetchedModels.value = models;
            diagnosticMessage.value = models.length
                ? `已获取 ${models.length} 个模型，请从下拉框选择或继续手动填写。`
                : '接口未返回可用模型，请手动填写模型 ID。';
        } else {
            const result = await call<ConnectionResult>('test_connection', args);
            if (generation !== diagnosticGeneration) return;
            diagnosticMessage.value = `连接成功 · ${(result.durationMs / 1000).toFixed(2)}s · 模型已完成独立测试请求`;
        }
    } catch (cause) {
        if (generation === diagnosticGeneration) diagnosticError.value = String(cause);
    } finally {
        diagnostic.value = null;
    }
}
</script>

<template>
    <main class="settings panel" aria-label="测智雷达设置">
        <header class="settings-titlebar drag-area" @mousedown.left="dragWindow().catch(() => {})">
            <div class="brand"><Target :size="19" class="brand-mark" /><span>测智雷达</span></div>
            <span class="titlebar-caption">设置</span>
            <div class="window-controls" @mousedown.stop>
                <button aria-label="最小化" @click="control('minimize')"><Minus :size="15" /></button>
                <button aria-label="最大化或还原" @click="control('maximize')"><Square :size="12" /></button>
                <button class="window-close" aria-label="关闭设置" @click="control('close')">
                    <X :size="17" />
                </button>
            </div>
        </header>
        <aside class="settings-sidebar">
            <p class="nav-label">工作空间</p>
            <nav aria-label="设置分类">
                <button
                    v-for="item in nav"
                    :key="item.id"
                    :class="{ active: page === item.id }"
                    :aria-current="page === item.id ? 'page' : undefined"
                    @click="page = item.id"
                >
                    <component :is="item.icon" :size="17" /><span>{{ item.label }}</span
                    ><ChevronRight v-if="page === item.id" :size="13" />
                </button>
            </nav>
            <div class="sidebar-bottom">
                <span class="local-dot"></span> 本地运行 <span class="version">v1.0.0</span>
            </div>
        </aside>

        <section class="settings-content">
            <div ref="contentScroll" class="settings-scroll" :data-page="page">
                <div class="page-intro">
                    <div>
                        <p class="eyebrow">
                            IQ RADAR /
                            {{
                                page === 'api'
                                    ? 'CONNECTION'
                                    : page === 'rules'
                                      ? 'TEST RULE'
                                      : page === 'widget'
                                        ? 'APPEARANCE'
                                        : page === 'history'
                                          ? 'HISTORY'
                                          : 'ABOUT'
                            }}
                        </p>
                        <h1>{{ nav.find((item) => item.id === page)?.label }}</h1>
                        <p class="page-description">
                            {{
                                page === 'api'
                                    ? '连接你要测试的模型，每次请求都是一次独立对话。'
                                    : page === 'rules'
                                      ? '定义你的问题，以及完整回答必须包含的连续字符串。'
                                      : page === 'widget'
                                        ? '让雷达以你习惯的方式，安静地留在桌面。'
                                        : page === 'history'
                                          ? '每一次结果，都保留当时的问题与判定依据。'
                                          : '版本信息、更新与项目反馈。'
                            }}
                        </p>
                    </div>
                    <span class="section-icon"
                        ><SlidersHorizontal v-if="page === 'api'" :size="22" /><ListChecks
                            v-else-if="page === 'rules'"
                            :size="22" /><Monitor v-else-if="page === 'widget'" :size="22" /><History
                            v-else-if="page === 'history'"
                            :size="22" /><Info v-else :size="22"
                    /></span>
                </div>

                <div v-if="!snapshot && !error" class="empty-state">正在读取本地配置…</div>
                <div v-if="busy" class="notice">
                    <Info :size="15" />测试正在进行，请完成或取消后再修改配置。
                </div>
                <div v-if="error" class="inline-error" role="alert">
                    <CircleAlert :size="15" />{{ error }}
                </div>
                <div v-if="savedMessage" class="success-message" role="status">
                    <Check :size="15" />{{ savedMessage }}
                </div>

                <form
                    v-if="page === 'api'"
                    id="profile-settings"
                    class="settings-form"
                    @submit.prevent="saveProfile"
                >
                    <div class="profile-toolbar">
                        <ModelPicker
                            label="编辑接口配置"
                            context="settings"
                            allow-new
                            :profiles="snapshot?.config.profiles ?? []"
                            :selected-id="editingId ?? ''"
                            :expanded="false"
                            :disabled="saving || busy"
                            @select="
                                editProfile(
                                    snapshot?.config.profiles.find((item) => item.id === $event) ?? null,
                                )
                            "
                            @error="error = $event"
                        />
                        <button
                            class="button subtle small"
                            type="button"
                            :disabled="busy"
                            @click="editProfile(null)"
                        >
                            <Plus :size="14" />新增
                        </button>
                    </div>
                    <div class="field-row">
                        <label for="profile-name">配置名称</label>
                        <div class="field">
                            <input
                                id="profile-name"
                                v-model="form.name"
                                required
                                maxlength="80"
                                placeholder="例如：我的模型"
                                :disabled="busy"
                            />
                            <p>用于区分不同的模型或服务商。</p>
                        </div>
                    </div>
                    <div class="field-row">
                        <label for="base-url">接口地址</label>
                        <div class="field">
                            <input
                                id="base-url"
                                v-model="form.baseUrl"
                                required
                                maxlength="2000"
                                placeholder="https://api.example.com/v1"
                                spellcheck="false"
                                :disabled="busy"
                            />
                            <p>Chat Completions 兼容接口；本地服务允许 HTTP。</p>
                        </div>
                    </div>
                    <div class="field-row">
                        <label for="api-key">API Key</label>
                        <div class="field">
                            <div class="input-affix">
                                <input
                                    id="api-key"
                                    v-model="form.apiKey"
                                    :type="showKey ? 'text' : 'password'"
                                    autocomplete="off"
                                    maxlength="2500"
                                    :placeholder="
                                        selectedProfile?.hasKey
                                            ? '已安全保存，留空保持不变'
                                            : '输入密钥；无需鉴权的本地服务可留空'
                                    "
                                    :disabled="busy"
                                /><button
                                    type="button"
                                    class="icon-button"
                                    :aria-label="showKey ? '隐藏密钥' : '显示当前输入的密钥'"
                                    @click="showKey = !showKey"
                                >
                                    <EyeOff v-if="showKey" :size="16" /><Eye v-else :size="16" />
                                </button>
                            </div>
                            <p><KeyRound :size="11" /> 密钥保存在 Windows 凭据管理器，不写入数据库。</p>
                            <label v-if="selectedProfile?.hasKey" class="checkbox-line"
                                ><input
                                    v-model="form.removeKey"
                                    type="checkbox"
                                    :disabled="busy"
                                />移除已保存的密钥</label
                            >
                        </div>
                    </div>
                    <div class="field-row">
                        <label for="model-name">模型名称</label>
                        <div class="field">
                            <input
                                id="model-name"
                                v-model="form.model"
                                required
                                maxlength="200"
                                placeholder="填写服务商提供的模型 ID"
                                spellcheck="false"
                                :disabled="busy"
                            />
                            <p>请填写准确的模型 ID，名称不会被应用自动替换。</p>
                            <div v-if="fetchedModels.length" class="catalog-picker">
                                <ModelPicker
                                    label="拉取到的模型"
                                    context="settings"
                                    :profiles="catalogOptions"
                                    :selected-id="form.model"
                                    :expanded="false"
                                    :disabled="busy || Boolean(diagnostic)"
                                    :show-model="false"
                                    @select="form.model = $event"
                                />
                            </div>
                            <div class="connection-actions">
                                <button
                                    type="button"
                                    class="button secondary small"
                                    :disabled="busy || saving || Boolean(diagnostic)"
                                    @click="runDiagnostic('models')"
                                >
                                    <RotateCcw :size="14" :class="{ spin: diagnostic === 'models' }" />{{
                                        diagnostic === 'models' ? '正在拉取' : '拉取模型'
                                    }}
                                </button>
                                <button
                                    type="button"
                                    class="button secondary small"
                                    :disabled="busy || saving || Boolean(diagnostic)"
                                    @click="runDiagnostic('connection')"
                                >
                                    <Zap :size="14" />{{
                                        diagnostic === 'connection' ? '正在测试' : '测试连接'
                                    }}
                                </button>
                            </div>
                            <p>
                                拉取模型不发起推理；测试连接发送独立短消息，可能产生少量 API
                                费用，不记录测智历史；超时沿用悬浮设置（{{
                                    snapshot?.config.preferences.timeoutSeconds ?? 60
                                }}
                                秒）。
                            </p>
                            <p v-if="diagnosticError" class="diagnostic-error" role="alert">
                                {{ diagnosticError }}
                            </p>
                            <p v-if="diagnosticMessage" class="diagnostic-message" role="status">
                                {{ diagnosticMessage }}
                            </p>
                        </div>
                    </div>
                </form>

                <form
                    v-else-if="page === 'rules'"
                    id="rule-settings"
                    class="rules-form"
                    @submit.prevent="saveRule"
                >
                    <div class="notice">
                        <Info :size="15" />适用于 GPT 6 及以上系列模型，其他模型无法保证测试准确度。
                    </div>
                    <div class="section-heading">
                        <label for="test-prompt">测试题目</label
                        ><span class="muted small-text">原样发送，不附加判定答案</span>
                    </div>
                    <textarea
                        id="test-prompt"
                        v-model="rule.prompt"
                        required
                        maxlength="20000"
                        :disabled="busy"
                        rows="11"
                        placeholder="填写发送给模型的完整问题和要求…"
                    ></textarea>
                    <div class="keyword-field">
                        <label for="keyword">必须包含的连续字符串</label
                        ><textarea
                            id="keyword"
                            v-model="rule.keyword"
                            required
                            maxlength="500"
                            rows="2"
                            :disabled="busy"
                            placeholder="例如：高市早苗"
                        ></textarea>
                        <p class="field-help">区分大小写，保留空格与换行，不使用正则或语义匹配。</p>
                    </div>
                    <details class="rule-sandbox">
                        <summary><Search :size="14" />本地试匹配<span>不调用 API</span></summary>
                        <label for="sample-answer" class="sr-only">用于试匹配的回答</label
                        ><textarea
                            id="sample-answer"
                            v-model="sampleAnswer"
                            rows="2"
                            placeholder="粘贴一段回答，检查是否命中…"
                        ></textarea>
                        <p v-if="sampleAnswer" :class="matching ? 'text-passed' : 'text-failed'">
                            {{ matching ? '命中 → 不降智' : '未命中 → 降智' }}
                        </p>
                    </details>
                    <p class="rule-disclaimer">
                        <Info :size="13" />单题关键词命中仅作为自定义检测信号，不代表模型整体能力。
                    </p>
                    <p v-if="restoredDefaults" class="muted small-text">已恢复默认草稿，保存后生效。</p>
                </form>

                <form
                    v-else-if="page === 'widget'"
                    id="widget-settings"
                    class="preferences-form"
                    @submit.prevent="save('save_preferences', { preferences: { ...preferences } })"
                >
                    <div class="preference-row">
                        <div>
                            <label for="autostart">开机自启</label>
                            <p>登录 Windows 后自动启动，切换立即生效；移动 EXE 后请关闭再开启。</p>
                            <p v-if="autostartBusy" role="status">正在同步系统设置…</p>
                            <p v-if="autostartError" class="text-failed" role="alert">
                                {{ autostartError }}
                                <button
                                    type="button"
                                    class="button secondary small"
                                    :disabled="autostartBusy"
                                    @click="syncAutostart"
                                >
                                    重试
                                </button>
                            </p>
                        </div>
                        <input
                            id="autostart"
                            class="switch"
                            type="checkbox"
                            :checked="autostartEnabled"
                            :disabled="autostartBusy || !autostartReady"
                            @change="toggleAutostart"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="always-top">始终置顶</label>
                            <p>让悬浮组件保持在普通窗口上方。</p>
                        </div>
                        <input
                            id="always-top"
                            v-model="preferences.alwaysOnTop"
                            class="switch"
                            type="checkbox"
                            :disabled="busy"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="snap-edge">边缘吸附与自动收起</label>
                            <p>左、右、顶部缩成状态图标，鼠标悬停时展开。</p>
                        </div>
                        <input
                            id="snap-edge"
                            v-model="preferences.snapToEdge"
                            class="switch"
                            type="checkbox"
                            :disabled="busy"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="auto-test">定时测试</label>
                            <p>自动调用当前模型，收起后继续运行；可能产生 API 费用。</p>
                        </div>
                        <input
                            id="auto-test"
                            v-model="preferences.autoTestEnabled"
                            class="switch"
                            type="checkbox"
                            :disabled="busy"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="auto-interval">测试间隔</label>
                            <p>默认 3 分钟；手动开始测试或保存设置后重新计时。</p>
                        </div>
                        <div class="number-control">
                            <input
                                id="auto-interval"
                                v-model.number="preferences.autoTestIntervalMinutes"
                                type="number"
                                min="1"
                                max="1440"
                                required
                                :disabled="busy || !preferences.autoTestEnabled"
                            /><span>分钟</span>
                        </div>
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="shortcut">快捷键显示 / 隐藏</label>
                            <p>Ctrl + Alt + P，隐藏后仍可从托盘恢复。</p>
                        </div>
                        <input
                            id="shortcut"
                            v-model="preferences.shortcutEnabled"
                            class="switch"
                            type="checkbox"
                            :disabled="busy"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="opacity">面板不透明度</label>
                            <p>只改变背景，保持文字清楚可读。</p>
                        </div>
                        <div class="range-control">
                            <input
                                id="opacity"
                                v-model.number="preferences.opacity"
                                type="range"
                                min="80"
                                max="100"
                                :disabled="busy"
                            /><span>{{ preferences.opacity }}%</span>
                        </div>
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="timeout">请求超时</label>
                            <p>超时后终止等待，不自动重试。</p>
                        </div>
                        <div class="number-control">
                            <input
                                id="timeout"
                                v-model.number="preferences.timeoutSeconds"
                                type="number"
                                min="10"
                                max="300"
                                required
                                :disabled="busy"
                            /><span>秒</span>
                        </div>
                    </div>
                    <div class="notice">
                        <Info :size="15" />关闭悬浮窗会隐藏到托盘；完全退出请使用托盘菜单。
                    </div>
                </form>

                <AboutPanel
                    v-else-if="page === 'about'"
                    :result="updateResult"
                    :checking="checkingUpdate"
                    :error="updateError"
                    :checked-at="checkedAt"
                    @open="openProjectLink"
                />
                <div v-else class="history-page">
                    <div v-if="selectedRecord" class="history-detail">
                        <div class="history-detail-title">
                            <span class="result-pill" :class="selectedRecord.status">{{
                                statusText[selectedRecord.status]
                            }}</span
                            ><span>{{ formatTime(selectedRecord.startedAt) }}</span>
                        </div>
                        <dl>
                            <dt>模型</dt>
                            <dd>{{ selectedRecord.profileName }} / {{ selectedRecord.model }}</dd>
                            <dt>请求耗时</dt>
                            <dd>{{ (selectedRecord.durationMs / 1000).toFixed(2) }} 秒</dd>
                            <dt>判定字符串</dt>
                            <dd class="pre-wrap">{{ selectedRecord.keyword }}</dd>
                            <dt>当时的完整题目</dt>
                            <dd class="history-text">{{ selectedRecord.prompt }}</dd>
                            <dt>模型回答</dt>
                            <dd class="history-text history-answer">
                                <MarkdownAnswer :source="selectedRecord.answer || '无正文'" />
                            </dd>
                            <template v-if="selectedRecord.error"
                                ><dt>错误说明</dt>
                                <dd>{{ selectedRecord.error }}</dd></template
                            >
                        </dl>
                    </div>
                    <template v-else>
                        <div class="history-toolbar">
                            <ModelPicker
                                label="按分组筛选历史"
                                context="settings"
                                :profiles="historyGroups"
                                :selected-id="historyGroup"
                                :expanded="false"
                                :disabled="false"
                                :show-model="false"
                                @select="historyGroup = $event"
                            />
                            <div class="input-affix">
                                <Search :size="15" /><input
                                    v-model="historySearch"
                                    placeholder="搜索模型、回答或判定字符串"
                                    aria-label="搜索历史"
                                />
                            </div>
                        </div>
                        <div v-if="!filteredHistory.length" class="empty-state">
                            <History :size="34" />
                            <h2>{{ historySearch || historyGroup ? '没有匹配的记录' : '还没有测试记录' }}</h2>
                            <p>完成一次测试后，可在这里查看回答与规则快照。</p>
                        </div>
                        <button
                            v-for="record in filteredHistory"
                            :key="record.id"
                            class="history-item"
                            @click="selectedRecord = record"
                        >
                            <div>
                                <span class="history-model">{{ record.profileName }}</span
                                ><span class="result-pill" :class="record.status">{{
                                    statusText[record.status]
                                }}</span>
                                <p>{{ record.model }}</p>
                                <span class="history-time"
                                    ><Clock3 :size="11" />{{ formatTime(record.startedAt) }} ·
                                    {{ (record.durationMs / 1000).toFixed(2) }}s</span
                                >
                            </div>
                            <ArrowUpRight :size="16" />
                        </button>
                    </template>
                </div>
            </div>

            <!-- 外部 form 关联保留浏览器原生必填校验，按钮区域独占固定行，不遮挡主体。 -->
            <footer class="form-footer settings-actions" aria-label="页面操作">
                <template v-if="page === 'api'">
                    <button
                        v-if="editingId"
                        type="button"
                        class="button danger subtle small"
                        :disabled="saving || busy"
                        @click="deleteProfile"
                    >
                        <Trash2 :size="14" />{{ confirmDelete ? '再次点击确认删除' : '删除配置' }}
                    </button>
                    <span v-else class="muted small-text">保存后可在悬浮窗切换模型</span>
                    <button class="button" type="submit" form="profile-settings" :disabled="saving || busy">
                        <Save :size="15" />{{ saving ? '正在保存' : '保存配置' }}
                    </button>
                </template>
                <template v-else-if="page === 'rules'">
                    <button
                        type="button"
                        class="button subtle"
                        :disabled="saving || busy"
                        @click="restoreRule"
                    >
                        <RotateCcw :size="14" />恢复默认
                    </button>
                    <button class="button" type="submit" form="rule-settings" :disabled="saving || busy">
                        <Save :size="15" />{{ saving ? '正在保存' : '保存规则' }}
                    </button>
                </template>
                <template v-else-if="page === 'widget'">
                    <span class="muted small-text">开机自启立即生效，其他设置保存后生效</span>
                    <button class="button" type="submit" form="widget-settings" :disabled="saving || busy">
                        <Save :size="15" />{{ saving ? '正在保存' : '保存设置' }}
                    </button>
                </template>
                <template v-else-if="page === 'about'">
                    <span class="muted small-text">免安装版 · 手动检查更新</span>
                    <div class="about-footer-actions">
                        <button
                            v-if="updateResult?.status === 'available'"
                            type="button"
                            class="button secondary"
                            @click="openProjectLink(updateResult.releaseUrl)"
                        >
                            <ArrowUpRight :size="15" />前往下载 v{{ updateResult.latestVersion }}
                        </button>
                        <button type="button" class="button" :disabled="checkingUpdate" @click="checkUpdates">
                            <RotateCcw :size="15" :class="{ spin: checkingUpdate }" />{{
                                checkingUpdate ? '正在检查' : '检查更新'
                            }}
                        </button>
                    </div>
                </template>
                <template v-else-if="selectedRecord">
                    <button type="button" class="button subtle" @click="selectedRecord = null">
                        ← 返回记录
                    </button>
                    <button type="button" class="button secondary" @click="copyRecord">
                        <Copy :size="15" />复制测试记录
                    </button>
                </template>
                <template v-else>
                    <span class="muted small-text">本地保留最近 500 条记录 · 不包含 API Key</span>
                    <button
                        type="button"
                        class="button subtle danger"
                        :disabled="!snapshot?.history.length || busy || saving"
                        @click="clearHistory"
                    >
                        <Trash2 :size="14" />{{ confirmClear ? '确认清空' : '清空' }}
                    </button>
                </template>
            </footer>
        </section>
    </main>
</template>
