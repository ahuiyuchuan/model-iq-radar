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
    ChevronRight,
    KeyRound,
    CircleAlert,
    Copy,
    Zap,
    Download,
    RefreshCw,
} from 'lucide-vue-next';
// 界面和脱敏诊断共用工程版本，发布时随清单更新，避免与关于页及原生程序版本不一致。
import { version } from '../../package.json';
import {
    call,
    DEFAULT_PROMPT,
    desktop,
    dragWindow,
    windowAction,
    protectWindowClose,
    closeSettingsWindow,
    subscribeUpdateStatus,
} from '../bridge';
import { downloadText, historyCsv, localDate, statusText, reasoningLabel } from '../historyExport';
import { formatTime, useProbe } from '../useProbe';
import { reasoningSupport } from '../reasoning';
import { ruleAnswers, ruleError } from '../rules';
import type {
    ModelProfile,
    Preferences,
    ProfileInput,
    TestRecord,
    TestRule,
    UpdateResult,
    FetchedModel,
    ModelCatalog,
    ConnectionResult,
} from '../types';
import ModelPicker from './ModelPicker.vue';
import HistoryPanel from './HistoryPanel.vue';
import AboutPanel from './AboutPanel.vue';
import ActionDialog from './ActionDialog.vue';
import MailSettings from './MailSettings.vue';

// 设置页是唯一需要完整历史的窗口；悬浮窗只加载当前模型的最新记录。
const { snapshot, busy, error, refresh } = useProbe({ includeHistory: true });
const page = ref('api');
const contentScroll = ref<HTMLElement>();
const saving = ref(false);
// 预览/导入同样锁住当前草稿，避免异步确认期间输入被保存基线吞掉。
const preparing = ref(false);
// 写入成功但快照读取失败时，禁止再次提交旧草稿，尤其避免新建接口重复创建。
const refreshRequired = ref(false);
const closing = ref(false);
const savedMessage = ref('');
// 原生收尾警告不代表草稿未保存；跨页面保留，避免“保存并继续”立即清掉故障提示。
const savedWarning = ref('');
const showKey = ref(false);
const editingId = ref<string | null>(null);
const form = reactive<ProfileInput>({
    id: '',
    name: '',
    baseUrl: '',
    model: '',
    apiKey: '',
    removeKey: false,
    interfaceMode: 'responses',
    protocolFallbackEnabled: true,
    reasoningEffort: 'low',
    timeoutSeconds: null,
    rulePresetId: null,
    catalogToken: null,
});
const rule = reactive<TestRule>({ prompt: DEFAULT_PROMPT, keyword: '高市早苗' });
const presetName = ref('');
const ruleTargetId = ref('default');
const mailPanel = ref<InstanceType<typeof MailSettings>>();
const mailDirty = ref(false);
const exportScope = ref('filtered');
const exportFormat = ref('json');
const importBackup = ref<unknown>(null);
const importStrategy = ref('skip');
const dialog = ref<{
    title: string;
    message: string;
    actions: { id: string; label: string; primary?: boolean; danger?: boolean }[];
    kind?: string;
} | null>(null);
let dialogResolve: ((choice: string) => void) | null = null;
let closeCleanup: (() => void) | null = null;
let disposed = false;
const preferences = reactive<Preferences>({
    alwaysOnTop: true,
    snapToEdge: true,
    opacity: 97,
    timeoutSeconds: 60,
    shortcutEnabled: true,
    autoTestEnabled: true,
    autoTestIntervalMinutes: 3,
    historyMaxRecords: 500,
    historyRetentionDays: null,
});
const sampleAnswer = ref('');
const filteredHistory = ref<TestRecord[]>([]);
const fetchedModels = ref<FetchedModel[]>([]);
const diagnostic = ref<'models' | 'connection' | null>(null);
const diagnosticError = ref('');
const diagnosticMessage = ref('');
let diagnosticGeneration = 0;
const selectedRecord = ref<TestRecord | null>(null);
const confirmDelete = ref(false);
const confirmClear = ref(false);
const restoredMessage = ref('');
const init = ref(false);
const profileBaseline = ref('');
const ruleBaseline = ref('');
const preferencesBaseline = ref('');
// 自启是 Windows 实际状态，独立于待保存的悬浮设置，切换不重置自动测试周期。
const autostartEnabled = ref(false);
const autostartReady = ref(false);
const autostartBusy = ref(false);
const autostartError = ref('');
// 后台仅刷新缓存标记；主动检查的反馈独立保存，不能把后台失败写入页面错误。
const updateResult = ref<UpdateResult | null>(null);
const checkingUpdate = ref(false);
const updateError = ref('');
const updateCheckedAt = ref('');
let updateRevision = 0;
let updateCleanup: (() => void) | undefined;
const importInput = ref<HTMLInputElement>();

// “关于”固定放在最后；后续功能入口插在它之前，保持首版的导航惯例。
const nav = [
    { id: 'api', label: '接口配置', icon: SlidersHorizontal },
    { id: 'rules', label: '测试规则', icon: ListChecks },
    { id: 'widget', label: '悬浮设置', icon: Monitor },
    { id: 'history', label: '测试历史', icon: History },
    { id: 'mail', label: '邮箱通知', icon: Zap },
    { id: 'about', label: '关于', icon: Info },
];
const selectedProfile = computed(() =>
    snapshot.value?.config.profiles.find((item) => item.id === editingId.value),
);
/** 仅限制已核实的精确型号；未知代理别名不猜测，不自动改变用户当前档位。 */
const reasoning = computed(() => reasoningSupport(form.model, form.interfaceMode, form.reasoningEffort));
const draftEffectiveTimeout = computed(() =>
    form.timeoutSeconds === null || String(form.timeoutSeconds) === ''
        ? (snapshot.value?.config.preferences.timeoutSeconds ?? 60)
        : form.timeoutSeconds,
);
const matching = computed(
    () => !ruleError(rule) && ruleAnswers(rule).some((answer) => sampleAnswer.value.includes(answer)),
);
const catalogOptions = computed(() =>
    fetchedModels.value.map((item) => ({ id: item.id, name: item.id, model: item.ownedBy ?? '可用模型 ID' })),
);
// 草稿地址、凭据或目标改变后旧目录不再适用；异步旧响应也不能填回新配置。
watch(
    () => [editingId.value, form.baseUrl, form.apiKey, form.removeKey],
    () => {
        diagnosticGeneration++;
        fetchedModels.value = [];
        form.catalogToken = null;
        diagnosticMessage.value = '';
        diagnosticError.value = '';
    },
    { flush: 'sync' },
);
let catalogGeneration = 0;
/** 缓存异步读取使用世代隔离；切换接口/地址后旧结果不能填回当前输入框。 */
watch(
    () => form.catalogToken,
    async (token) => {
        const generation = ++catalogGeneration;
        if (!token) return;
        try {
            const models = await call<FetchedModel[]>('get_model_catalog', { token, baseUrl: form.baseUrl });
            if (generation === catalogGeneration && !disposed) fetchedModels.value = models ?? [];
        } catch (cause) {
            if (generation === catalogGeneration && !disposed)
                diagnosticError.value = `读取已保存模型列表失败：${String(cause)}`;
        }
    },
);
// 手动改模型时保留同服务目录，但旧连接测试结果不得显示成新模型的成功状态。
watch(
    () => [
        form.model,
        form.interfaceMode,
        form.protocolFallbackEnabled,
        form.reasoningEffort,
        draftEffectiveTimeout.value,
        form.rulePresetId,
    ],
    () => {
        if (diagnostic.value === 'connection') diagnosticGeneration++;
        diagnosticMessage.value = '';
        diagnosticError.value = '';
    },
    { flush: 'sync' },
);
/** 首次加载初始化；外部快捷开关只合并同名字段，不能覆盖其他未保存草稿。 */
watch(snapshot, (value, previous) => {
    if (!value) return;
    if (!init.value) {
        init.value = true;
        assignRule(value.config.rule);
        presetName.value = value.config.defaultRuleName ?? '默认规则';
        Object.assign(preferences, value.config.preferences);
        editProfile(value.config.profiles.find((item) => item.id === value.config.selectedProfileId) ?? null);
        ruleBaseline.value = ruleDraft();
        preferencesBaseline.value = JSON.stringify(preferences);
        return;
    }
    // 卡片快捷切换只同步未编辑的规则/强度字段，不覆盖其他草稿或用户主动选择的值。
    const baseline = profileBaseline.value ? (JSON.parse(profileBaseline.value) as ProfileInput) : null;
    const liveProfile = value.config.profiles.find((item) => item.id === editingId.value);
    if (baseline && liveProfile) {
        if (form.rulePresetId === baseline.rulePresetId) {
            form.rulePresetId = liveProfile.rulePresetId ?? null;
            baseline.rulePresetId = form.rulePresetId;
        }
        if (form.reasoningEffort === baseline.reasoningEffort) {
            form.reasoningEffort =
                liveProfile.reasoningEffort === undefined ? 'low' : liveProfile.reasoningEffort;
            baseline.reasoningEffort = form.reasoningEffort;
        }
        profileBaseline.value = JSON.stringify(baseline);
    }
    const live = value.config.preferences.autoTestEnabled;
    if (previous && live !== previous.config.preferences.autoTestEnabled) {
        preferences.autoTestEnabled = live;
        const saved = JSON.parse(preferencesBaseline.value) as Preferences;
        saved.autoTestEnabled = live;
        preferencesBaseline.value = JSON.stringify(saved);
    }
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
onMounted(async () => {
    window.addEventListener('beforeunload', protectDrafts);
    // 缓存读取失败与后台网络失败保持静默；不写入页面通用 error，也不清掉已有标记。
    void subscribeUpdateStatus((result) => {
        if (!disposed) {
            updateRevision++;
            updateResult.value = result;
        }
    })
        .then((cleanup) => {
            if (disposed) cleanup();
            else updateCleanup = cleanup;
        })
        .catch(() => {});
    try {
        const cleanup = await protectWindowClose(requestClose);
        if (disposed) cleanup();
        else closeCleanup = cleanup;
    } catch (cause) {
        if (!disposed) error.value = `未能启用原生关闭保护，请先保存再关闭窗口：${String(cause)}`;
    }
});
onUnmounted(() => {
    disposed = true;
    window.removeEventListener('beforeunload', protectDrafts);
    closeCleanup?.();
    updateCleanup?.();
    dialogResolve?.('cancel');
});
/** 浏览器刷新仍用浏览器保护；桌面原生关闭由统一自定义对话框处理。 */
function protectDrafts(event: BeforeUnloadEvent): void {
    if (!desktop && draftDirty()) {
        event.preventDefault();
        event.returnValue = '';
    }
}
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

/**
 * 显式切换编辑目标时清空密钥草稿，已有密钥只显示保存状态，不读取明文。
 * 新建默认 Responses 并开启协议回退；已有配置缺字段仍按首版 Chat/关闭解释，不能升级即改变请求策略。
 */
function editProfile(profile: ModelProfile | null): void {
    editingId.value = profile?.id ?? null;
    Object.assign(form, {
        id: profile?.id ?? '',
        name: profile?.name ?? '',
        baseUrl: profile?.baseUrl ?? '',
        model: profile?.model ?? '',
        apiKey: '',
        removeKey: false,
        interfaceMode: profile ? (profile.interfaceMode ?? 'chat_completions') : 'responses',
        protocolFallbackEnabled: profile ? (profile.protocolFallbackEnabled ?? false) : true,
        reasoningEffort:
            profile && Object.prototype.hasOwnProperty.call(profile, 'reasoningEffort')
                ? (profile.reasoningEffort ?? null)
                : 'low',
        timeoutSeconds: profile?.timeoutSeconds ?? null,
        rulePresetId: profile?.rulePresetId ?? null,
        catalogToken: profile?.catalogToken ?? null,
    });
    showKey.value = false;
    confirmDelete.value = false;
    profileBaseline.value = JSON.stringify(form);
}

/** 规则名称属于草稿，预设编辑不能被误当作全局规则保存。 */
function ruleDraft(): string {
    return JSON.stringify({ rule: { ...rule }, name: presetName.value });
}
function draftDirty(): boolean {
    if (page.value === 'api') return JSON.stringify(form) !== profileBaseline.value;
    if (page.value === 'rules') return ruleDraft() !== ruleBaseline.value;
    if (page.value === 'widget') return JSON.stringify(preferences) !== preferencesBaseline.value;
    return page.value === 'mail' && mailDirty.value;
}
/** 同时只保留一个用户决策；重复点击关闭/切页不会覆盖已有等待或丢弃草稿。 */
function ask(
    title: string,
    message: string,
    actions: NonNullable<typeof dialog.value>['actions'],
    kind?: string,
): Promise<string> {
    if (dialog.value) return Promise.resolve('cancel');
    dialog.value = { title, message, actions, kind };
    return new Promise((resolve) => {
        dialogResolve = resolve;
    });
}
function resolveDialog(choice: string): void {
    const resolve = dialogResolve;
    dialog.value = null;
    dialogResolve = null;
    resolve?.(choice);
}
function discardDraft(): void {
    if (page.value === 'api') Object.assign(form, JSON.parse(profileBaseline.value));
    if (page.value === 'rules') {
        const saved = JSON.parse(ruleBaseline.value) as { rule: TestRule; name: string };
        assignRule(saved.rule);
        presetName.value = saved.name;
    }
    if (page.value === 'widget') Object.assign(preferences, JSON.parse(preferencesBaseline.value));
    if (page.value === 'mail') mailPanel.value?.discard();
}
/** 保存失败继续留在原编辑页；放弃会恢复基线，取消不改变页面/编辑对象。 */
async function mayLeave(): Promise<boolean> {
    if (saving.value || preparing.value || refreshRequired.value) return false;
    if (page.value === 'mail' && mailPanel.value?.isWorking) {
        error.value = '邮件操作正在进行，请等待完成后再离开此页。';
        return false;
    }
    if (!draftDirty()) return true;
    const choice = await ask('有未保存的修改', '保存后继续，或放弃当前草稿；取消将留在此处。', [
        { id: 'cancel', label: '取消' },
        { id: 'discard', label: '放弃修改', danger: true },
        { id: 'save', label: '保存并继续', primary: true },
    ]);
    if (choice === 'discard') {
        discardDraft();
        return true;
    }
    if (choice !== 'save') return false;
    if (page.value === 'api') await saveProfile();
    else if (page.value === 'rules') await saveRule();
    else if (page.value === 'widget') await savePreferences();
    else if (page.value === 'mail') return (await mailPanel.value?.save()) ?? false;
    return !draftDirty() && !error.value;
}
async function changePage(next: string): Promise<void> {
    if (next !== page.value && (await mayLeave())) page.value = next;
}
async function changeProfile(id: string): Promise<void> {
    if (await mayLeave()) editProfile(snapshot.value?.config.profiles.find((item) => item.id === id) ?? null);
}
async function newProfile(): Promise<void> {
    if (await mayLeave()) editProfile(null);
}
async function requestClose(): Promise<void> {
    if (closing.value) return;
    closing.value = true;
    try {
        // 待重读表示后端已保存成功，不能再次把已提交草稿当成未保存修改。
        if (refreshRequired.value || (await mayLeave())) await closeSettingsWindow();
    } catch (cause) {
        error.value = `关闭设置失败：${String(cause)}`;
    } finally {
        closing.value = false;
    }
}

/**
 * 从已提交的后端状态恢复所有编辑基线；读取失败保持阻塞，避免旧表单覆盖新配置。
 * 此路径不重复执行写入，也不重新触发历史清理、凭据保存或模型请求。
 */
async function recoverSavedState(): Promise<void> {
    if (preparing.value || saving.value) return;
    preparing.value = true;
    try {
        if (!(await refresh()) || !snapshot.value) return;
        Object.assign(preferences, snapshot.value.config.preferences);
        preferencesBaseline.value = JSON.stringify(preferences);
        loadRuleTarget('default');
        editProfile(
            snapshot.value.config.profiles.find(
                (item) => item.id === snapshot.value?.config.selectedProfileId,
            ) ?? null,
        );
        refreshRequired.value = false;
        error.value = '';
        savedMessage.value = '已重新读取保存结果，请检查当前编辑对象。';
    } finally {
        preparing.value = false;
    }
}
/** 邮件与其他保存共用读取恢复边界；保存并继续必须等快照同步，不能从旧快照重建邮件表单。 */
async function synchronizeMailSave(): Promise<boolean> {
    preparing.value = true;
    error.value = '';
    try {
        if (await refresh()) return true;
        refreshRequired.value = true;
        error.value = '邮件设置已保存，但读取最新状态失败；请重新读取后继续编辑。';
        return false;
    } finally {
        preparing.value = false;
    }
}
/**
 * 串行执行保存并保留失败草稿；后端再次验证所有字段，前端验证用于即时反馈。
 * @param command 保存命令名。
 * @param args 草稿副本，包含密钥时不写日志。
 * @returns 仅写入与快照读取均成功时返回 true；写入失败保留草稿，已写入但读取失败进入专用恢复态。
 */
async function save(command: string, args: Record<string, unknown>): Promise<boolean> {
    if (saving.value || busy.value || refreshRequired.value) return false;
    saving.value = true;
    error.value = '';
    savedMessage.value = '';
    try {
        const outcome = await call<{ warning?: string | null } | null>(command, args);
        savedWarning.value = outcome?.warning ?? '';
        if (!(await refresh())) {
            refreshRequired.value = true;
            error.value = `更改已保存，但读取最新状态失败；请重新读取后继续编辑。${outcome?.warning ?? ''}`;
            return false;
        }
        if (command === 'save_profile') {
            editProfile(
                snapshot.value?.config.profiles.find(
                    (item) => item.id === snapshot.value?.config.selectedProfileId,
                ) ?? null,
            );
        }
        if (command === 'save_profile') profileBaseline.value = JSON.stringify(form);
        if (command === 'save_rule' || command === 'save_rule_preset') ruleBaseline.value = ruleDraft();
        if (command === 'save_preferences') preferencesBaseline.value = JSON.stringify(preferences);
        savedMessage.value = desktop ? '已保存，更改已生效' : '预览已更新；密钥不会保存在浏览器中';
        if (outcome?.warning) savedMessage.value = '更改已保存。';
        return true;
    } catch (cause) {
        error.value = String(cause);
        return false;
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
    const timeout = normalizeNullableNumber(form.timeoutSeconds);
    if (timeout !== null && (!Number.isInteger(timeout) || timeout < 10 || timeout > 300)) {
        error.value = '配置超时应为 10—300 秒的整数，或留空继承全局设置。';
        return;
    }
    await save('save_profile', {
        input: { ...form, timeoutSeconds: timeout },
    });
}

/** HTML number 输入清空会产生空串，IPC 前显式还原 null，禁止将空串变为 0。 */
function normalizeNullableNumber(value: unknown): number | null {
    return value === '' || value == null ? null : Number(value);
}
/** 严格保留有意义空白；全局规则与命名预设分别存储，编辑已有预设保持稳定 ID。 */
async function saveRule(): Promise<void> {
    const validation = ruleError(rule);
    if (validation) {
        error.value = validation;
        return;
    }
    if (!presetName.value.trim() || [...presetName.value].length > 80) {
        error.value = '规则名称必须为 1—80 个字符。';
        return;
    }
    if (ruleTargetId.value !== 'default') {
        const id = ruleTargetId.value === 'new' ? crypto.randomUUID() : ruleTargetId.value;
        const saved = await save('save_rule_preset', {
            preset: { id, name: presetName.value.trim(), ...rule },
        });
        if (saved) ruleTargetId.value = id;
    } else await save('save_rule', { rule: { ...rule }, name: presetName.value.trim() });
    restoredMessage.value = '';
}
/** 显式复制题答并清空不存在的替代项，防止多答案预设切到旧规则后遗留内容。 */
function assignRule(value: TestRule): void {
    rule.prompt = value.prompt;
    rule.keyword = value.keyword;
    if (value.alternativeKeywords?.length) rule.alternativeKeywords = [...value.alternativeKeywords];
    else delete rule.alternativeKeywords;
}
/** 单独输入每个答案，换行属于答案正文；最多 20 项，与后端约束一致。 */
function addAnswer(): void {
    if (saving.value || preparing.value || busy.value || ruleAnswers(rule).length >= 20) return;
    rule.alternativeKeywords = [...(rule.alternativeKeywords ?? []), ''];
}
/** 删除首项时提升下一项为兼容 keyword；至少保留一个输入框，空项由统一校验提示。 */
function removeAnswer(index: number): void {
    if (saving.value || preparing.value || busy.value) return;
    const remaining = ruleAnswers(rule).filter((_, position) => position !== index);
    rule.keyword = remaining[0] ?? '';
    if (remaining.length > 1) rule.alternativeKeywords = remaining.slice(1);
    else delete rule.alternativeKeywords;
}
function loadRuleTarget(id: string): void {
    ruleTargetId.value = id;
    const preset = snapshot.value?.config.rulePresets?.find((item) => item.id === id);
    assignRule(
        preset ??
            (id === 'new'
                ? { prompt: DEFAULT_PROMPT, keyword: '高市早苗' }
                : (snapshot.value?.config.rule ?? { prompt: DEFAULT_PROMPT, keyword: '高市早苗' })),
    );
    presetName.value =
        preset?.name ?? (id === 'default' ? (snapshot.value?.config.defaultRuleName ?? '默认规则') : '');
    restoredMessage.value = '';
    ruleBaseline.value = ruleDraft();
}
async function changeRuleTarget(id: string): Promise<void> {
    if (await mayLeave()) loadRuleTarget(id);
}
/** 删除引用规则采用后端原子解绑，确认后模型统一回退到默认规则。 */
async function deletePreset(id: string): Promise<void> {
    const count = snapshot.value?.config.profiles.filter((item) => item.rulePresetId === id).length ?? 0;
    const choice = await ask(
        '删除规则预设',
        count
            ? `有 ${count} 个模型正在使用此规则。删除后它们将沿用默认规则，原有结果需要重测；历史保留。`
            : '删除此规则预设，已保存历史不受影响。',
        [
            { id: 'cancel', label: '取消' },
            { id: 'delete', label: count ? '解绑并删除' : '删除预设', danger: true },
        ],
    );
    if (choice !== 'delete') return;
    const saved = await save('delete_rule_preset', { id, unbindProfiles: count > 0 });
    if (saved && ruleTargetId.value === id) loadRuleTarget('default');
}
/** 提前展示实际清理影响；确认前不修改设置或删除历史。 */
async function savePreferences(): Promise<void> {
    if (saving.value || preparing.value || busy.value) return;
    const input = {
        ...preferences,
        historyMaxRecords: normalizeNullableNumber(preferences.historyMaxRecords),
        historyRetentionDays: normalizeNullableNumber(preferences.historyRetentionDays),
    };
    const originalAutoState = snapshot.value?.config.preferences.autoTestEnabled;
    const savedAutoState = preferencesBaseline.value
        ? (JSON.parse(preferencesBaseline.value) as Preferences).autoTestEnabled
        : originalAutoState;
    // 未明确编辑定时开关时，让后端在持锁写入时保留权威值，覆盖尚未送达的托盘事件竞态。
    let preserveAutoTest = input.autoTestEnabled === savedAutoState;
    const values = [
        [input.timeoutSeconds, 10, 300],
        [input.autoTestIntervalMinutes, 1, 1440],
        [input.historyMaxRecords, 1, 5000],
        [input.historyRetentionDays, 1, 3650],
    ];
    if (
        values.some(
            ([value, min, max]) =>
                value !== null &&
                (!Number.isInteger(value) || Number(value) < Number(min) || Number(value) > Number(max)),
        )
    ) {
        error.value = '超时、间隔与历史保留范围必须填写有效整数。';
        return;
    }
    preparing.value = true;
    try {
        const impact = await call<{ removedCount: number; remainingCount: number }>(
            'preview_history_retention',
            { preferences: input },
        );
        let confirmed = false;
        if (impact?.removedCount) {
            const choice = await ask(
                '确认历史清理范围',
                `保存后立即清理 ${impact.removedCount} 条历史，保留 ${impact.remainingCount} 条；此操作不可撤销，模型配置和凭据不受影响。`,
                [
                    { id: 'cancel', label: '取消' },
                    { id: 'confirm', label: '确认清理并保存', danger: true },
                ],
            );
            if (choice !== 'confirm') return;
            confirmed = true;
        }
        // 预览/确认期间托盘仍能切换自动测试；后发生的外部开关优先，不能被旧表单快照撤销。
        if (snapshot.value?.config.preferences.autoTestEnabled !== originalAutoState) {
            input.autoTestEnabled = preferences.autoTestEnabled;
            preserveAutoTest = true;
        }
        const saved = await save('save_preferences', {
            preferences: input,
            confirmCleanup: confirmed,
            preserveAutoTest,
        });
        if (saved) {
            Object.assign(preferences, input, {
                autoTestEnabled: snapshot.value?.config.preferences.autoTestEnabled ?? input.autoTestEnabled,
            });
            preferencesBaseline.value = JSON.stringify(preferences);
        }
    } catch (cause) {
        error.value = String(cause);
    } finally {
        preparing.value = false;
    }
}
/** 默认规则恢复内置题答但保留名称；自定义预设还原自身保存内容，绝不套入其他规则。 */
function restoreRule(): void {
    if (saving.value || preparing.value || busy.value) return;
    if (ruleTargetId.value === 'default') {
        assignRule({ prompt: DEFAULT_PROMPT, keyword: '高市早苗' });
        restoredMessage.value = '已恢复内置题答草稿，规则名称保留；保存后生效。';
    } else {
        const preset = snapshot.value?.config.rulePresets?.find((item) => item.id === ruleTargetId.value);
        if (!preset) return;
        assignRule(preset);
        presetName.value = preset.name;
        restoredMessage.value = '已还原当前预设的已保存内容。';
    }
}

/** 二次点击确认后删除配置及对应凭据，历史记录仍保留测试时快照。 */
async function deleteProfile(): Promise<void> {
    if (!confirmDelete.value) {
        confirmDelete.value = true;
        return;
    }
    const saved = await save('delete_profile', { id: editingId.value });
    if (saved) editProfile(snapshot.value?.config.profiles[0] ?? null);
}

/** 清空是明确的破坏性操作，提供行内二次确认，避免模态窗口阻塞悬浮窗。 */
async function clearHistory(): Promise<void> {
    if (!confirmClear.value) {
        confirmClear.value = true;
        return;
    }
    const saved = await save('clear_history', {});
    if (saved) {
        selectedRecord.value = null;
        confirmClear.value = false;
    }
}

/** 系统窗口操作失败以可见消息呈现，不影响保存的配置。 */
async function control(action: 'close' | 'minimize' | 'maximize'): Promise<void> {
    try {
        if (action === 'close') await requestClose();
        else await windowAction(action);
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
            `模型：${record.model}\n时间：${record.startedAt}\n判定：${statusText[record.status]}\n规则：${record.ruleName ?? '未记录'}\n判定答案（任一命中）：\n${ruleAnswers(
                record,
            )
                .map((answer, index) => `${index + 1}. ${answer}`)
                .join('\n')}\n题目：\n${record.prompt}\n\n回答：\n${record.answer}\n${record.error ?? ''}`,
        );
        savedMessage.value = '已复制本条测试记录';
    } catch {
        error.value = '复制失败，请手动选择详情中的文本。';
    }
}

/** 复制脱敏诊断摘要；仅包含版本、时间、错误和非敏感请求设置，不包含题目、回答或密钥。 */
async function copyDiagnostic(): Promise<void> {
    const record = selectedRecord.value;
    if (!record) return;
    const summary = [
        '测智雷达诊断信息',
        `版本：v${version}`,
        `时间：${record.startedAt}`,
        `模型：${record.profileName} / ${record.model}`,
        `接口模式：${record.interfaceMode ?? '未记录'}`,
        `失败切换协议：${record.protocolFallbackEnabled == null ? '未记录' : record.protocolFallbackEnabled ? '开启' : '关闭'}`,
        `协议尝试顺序：${record.attemptedModes?.length ? record.attemptedModes.join(' → ') : '未记录'}`,
        `思考强度：${reasoningLabel(record)}`,
        `有效超时：${record.effectiveTimeoutSeconds ?? '未记录'} 秒`,
        `错误类型：${record.errorKind ?? '未记录'}`,
        `HTTP 状态码：${record.httpStatus ?? '未记录'}`,
        `服务商请求编号：${record.requestId ?? '未记录'}`,
        `错误：${record.error ?? '无'}`,
    ].join('\n');
    try {
        await navigator.clipboard.writeText(summary);
        savedMessage.value = '已复制脱敏诊断信息';
    } catch {
        error.value = '复制失败，请手动选择错误详情。';
    }
}

/** JSON 无损保留原文；CSV 增加公式前缀转义，范围由用户明确选择。 */
function exportHistory(): void {
    const records = exportScope.value === 'all' ? (snapshot.value?.history ?? []) : filteredHistory.value;
    if (!records.length) {
        error.value = '当前范围没有可导出的历史记录。';
        return;
    }
    const format = exportFormat.value;
    downloadText(
        `测智雷达-历史-${localDate(new Date())}.${format}`,
        format === 'csv' ? historyCsv(records) : JSON.stringify(records, null, 2),
        format === 'csv' ? 'text/csv;charset=utf-8' : 'application/json;charset=utf-8',
    );
    savedMessage.value = `已导出 ${records.length} 条历史记录${format === 'csv' ? '（公式前缀已转义，原文请用 JSON）' : ''}`;
}
/** 备份由后端白名单生成，绝不直接序列化含凭据状态的整个运行快照。 */
async function exportConfig(): Promise<void> {
    try {
        const backup = await call('export_config_backup');
        downloadText(
            `测智雷达-配置-${localDate(new Date())}.json`,
            JSON.stringify(backup, null, 2),
            'application/json;charset=utf-8',
        );
        savedMessage.value = '已导出配置备份（不含 API Key 或邮箱授权码）';
    } catch (cause) {
        error.value = String(cause);
    }
}
async function chooseConfigImport(): Promise<void> {
    if (await mayLeave()) importInput.value?.click();
}
/** 有大小上限的备份先由后端全量验证，预览后单次事务导入，避免部分保存。 */
async function importConfig(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file || saving.value || preparing.value || busy.value) return;
    if (!(await mayLeave())) return;
    if (file.size > 4 * 1024 * 1024) {
        error.value = '配置备份不能超过 4 MiB。';
        return;
    }
    preparing.value = true;
    try {
        importBackup.value = JSON.parse(await file.text());
        const preview = await call<{
            profilesCount: number;
            presetsCount: number;
            duplicateProfiles: number;
            duplicatePresets: number;
            profilesMissingKeys: number;
            warnings: string[];
        }>('preview_config_import', {
            backup: importBackup.value,
        });
        const cleanup = await call<{ removedCount: number; remainingCount: number }>(
            'preview_history_retention',
            {
                preferences: (importBackup.value as { config: { preferences: Preferences } }).config
                    .preferences,
            },
        );
        const choice = await ask(
            '预览配置导入',
            `接口配置 ${preview.profilesCount} 个（重复 ${preview.duplicateProfiles} 个），规则预设 ${preview.presetsCount} 个（重复 ${preview.duplicatePresets} 个）。导入不含密钥和授权码；新配置需补填凭据。${preview.warnings?.length ? '\n注意：' + preview.warnings.join('；') : ''}\n全局规则和偏好会应用备份值。${cleanup.removedCount ? `\n应用后清理 ${cleanup.removedCount} 条历史，保留 ${cleanup.remainingCount} 条；此操作不可撤销。` : '历史保留范围无需清理。'}`,
            [
                { id: 'cancel', label: '取消' },
                { id: 'import', label: '确认导入', primary: true },
            ],
            'import',
        );
        if (choice !== 'import') return;
        const duplicateStrategy = importStrategy.value;
        const report = await call<{ warning?: string | null } | null>('import_config_backup', {
            backup: importBackup.value,
            duplicateStrategy,
            confirmCleanup: Boolean(cleanup.removedCount),
        });
        savedWarning.value = report?.warning ?? '';
        if (!(await refresh())) {
            refreshRequired.value = true;
            error.value = `配置已导入，但读取最新状态失败；请重新读取，不要重复导入。${report?.warning ?? ''}`;
            return;
        }
        if (snapshot.value) {
            Object.assign(preferences, snapshot.value.config.preferences);
            preferencesBaseline.value = JSON.stringify(preferences);
            loadRuleTarget('default');
            editProfile(
                snapshot.value.config.profiles.find(
                    (item) => item.id === snapshot.value?.config.selectedProfileId,
                ) ?? null,
            );
        }
        savedMessage.value = '配置已原子导入；请检查接口地址并补填 API Key 和邮箱授权码。';
    } catch (cause) {
        error.value = `导入失败：${String(cause)}`;
    } finally {
        importBackup.value = null;
        preparing.value = false;
    }
}
/** 主动检查单独反馈、前端去重；后台事件优先，关闭窗口后不回填过期 UI。 */
async function checkForUpdates(): Promise<void> {
    if (checkingUpdate.value || disposed) return;
    checkingUpdate.value = true;
    updateError.value = '';
    const revision = updateRevision;
    try {
        const result = await call<UpdateResult>('check_updates');
        if (disposed) return;
        if (revision === updateRevision) updateResult.value = result;
        updateCheckedAt.value = formatTime(new Date().toISOString());
    } catch (cause) {
        if (!disposed) updateError.value = String(cause);
    } finally {
        if (!disposed) checkingUpdate.value = false;
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
        const args = { input: { ...form, timeoutSeconds: normalizeNullableNumber(form.timeoutSeconds) } };
        if (kind === 'models') {
            const catalog = await call<ModelCatalog>('fetch_models', args);
            if (generation !== diagnosticGeneration) return;
            const models = catalog.models;
            // 首次加载缓存可能比网络刷新更晚返回；旧读取不能覆盖刚拉取的新目录。
            catalogGeneration++;
            form.catalogToken = catalog.token;
            fetchedModels.value = models;
            // 列表缓存是即时元数据；不把仅更新建议列表视为用户尚未保存的接口修改。
            const baseline = JSON.parse(profileBaseline.value) as ProfileInput;
            baseline.catalogToken = catalog.token;
            profileBaseline.value = JSON.stringify(baseline);
            diagnosticMessage.value = models.length
                ? `已更新 ${models.length} 个模型，点击模型名称选择或继续手动填写；列表已缓存。`
                : '接口未返回可用模型，请手动填写模型 ID。';
        } else {
            const result = await call<ConnectionResult>('test_connection', args);
            if (generation !== diagnosticGeneration) return;
            const mode =
                result.interfaceMode === 'responses'
                    ? 'Responses'
                    : result.interfaceMode === 'chat_completions'
                      ? 'Chat Completions'
                      : '所选协议';
            diagnosticMessage.value = `连接成功 · ${(result.durationMs / 1000).toFixed(2)}s · ${mode}${result.attemptedModes?.length > 1 ? '（已切换协议）' : ''} · 模型已完成独立测试请求`;
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
        <aside class="settings-sidebar" :inert="Boolean(dialog)">
            <p class="nav-label">工作空间</p>
            <nav aria-label="设置分类">
                <button
                    v-for="item in nav"
                    :key="item.id"
                    :class="{
                        active: page === item.id,
                        'has-update': item.id === 'about' && updateResult?.status === 'available',
                    }"
                    :disabled="preparing || saving || refreshRequired"
                    :aria-current="page === item.id ? 'page' : undefined"
                    @click="changePage(item.id)"
                >
                    <component :is="item.icon" :size="17" /><span>{{ item.label }}</span
                    ><span
                        v-if="item.id === 'about' && updateResult?.status === 'available'"
                        class="nav-update-badge"
                        :title="`发现新版本 v${updateResult.latestVersion}`"
                        >新版本</span
                    ><ChevronRight v-if="page === item.id" :size="13" />
                </button>
            </nav>
            <div class="sidebar-bottom">
                <span class="local-dot"></span> 本地运行 <span class="version">v{{ version }}</span>
            </div>
        </aside>

        <section class="settings-content" :inert="Boolean(dialog)">
            <div v-if="refreshRequired" class="inline-error snapshot-recovery" role="alert">
                <span>{{ error || '更改已经写入，请重新读取后继续编辑，避免重复提交。' }}</span>
                <button
                    type="button"
                    class="button secondary small"
                    :disabled="preparing"
                    @click="recoverSavedState"
                >
                    {{ preparing ? '正在读取…' : '重新读取已保存配置' }}
                </button>
            </div>
            <div ref="contentScroll" class="settings-scroll" :data-page="page" :inert="refreshRequired">
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
                                          : page === 'mail'
                                            ? 'EMAIL'
                                            : 'ABOUT'
                            }}
                        </p>
                        <h1>{{ nav.find((item) => item.id === page)?.label }}</h1>
                        <p class="page-description">
                            {{
                                page === 'api'
                                    ? '连接你要测试的模型，每次请求都是一次独立对话。'
                                    : page === 'rules'
                                      ? '定义题目和多个判定答案，完整回答命中任意一项即不降智。'
                                      : page === 'widget'
                                        ? '让雷达以你习惯的方式，安静地留在桌面。'
                                        : page === 'history'
                                          ? '每一次结果，都保留当时的问题与判定依据。'
                                          : page === 'mail'
                                            ? '通过加密 SMTP 接收模型状态变化，不打断桌面测试。'
                                            : '版本信息、更新与项目反馈。'
                            }}
                        </p>
                    </div>
                    <span class="section-icon"
                        ><SlidersHorizontal v-if="page === 'api'" :size="22" /><ListChecks
                            v-else-if="page === 'rules'"
                            :size="22" /><Monitor v-else-if="page === 'widget'" :size="22" /><History
                            v-else-if="page === 'history'"
                            :size="22" /><Zap v-else-if="page === 'mail'" :size="22" /><Info
                            v-else
                            :size="22"
                    /></span>
                </div>

                <div v-if="!snapshot && !error" class="empty-state">正在读取本地配置…</div>
                <div v-if="busy" class="notice">
                    <Info :size="15" />测试正在进行，请完成或取消后再修改配置。
                </div>
                <div v-if="error && !refreshRequired" class="inline-error" role="alert">
                    <CircleAlert :size="15" />{{ error }}
                </div>
                <div v-if="savedMessage" class="success-message" role="status">
                    <Check :size="15" />{{ savedMessage }}
                </div>
                <div v-if="savedWarning" class="inline-error save-warning" role="alert">
                    <CircleAlert :size="15" />{{ savedWarning }}
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
                            placeholder="新建接口配置（未保存）"
                            :profiles="snapshot?.config.profiles ?? []"
                            :selected-id="editingId ?? ''"
                            :expanded="false"
                            :disabled="saving || preparing || busy || !snapshot?.config.profiles.length"
                            @select="changeProfile($event)"
                            @error="error = $event"
                        />
                        <button
                            class="button subtle small"
                            type="button"
                            aria-label="新增接口配置"
                            :disabled="busy || saving || preparing"
                            @click="newProfile"
                        >
                            <Plus :size="14" aria-hidden="true" />新增
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
                                :disabled="busy || saving || preparing"
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
                                :disabled="busy || saving || preparing"
                            />
                            <p>
                                填写服务商 API 根路径或对应端点；本地服务允许 HTTP，协议由下方接口模式决定。
                            </p>
                        </div>
                    </div>
                    <div class="field-row">
                        <label>接口模式</label>
                        <div class="field">
                            <ModelPicker
                                label="接口模式"
                                context="settings"
                                :profiles="[
                                    { id: 'chat_completions', name: 'Chat Completions', model: '' },
                                    { id: 'responses', name: 'Responses', model: '' },
                                ]"
                                :selected-id="form.interfaceMode ?? 'responses'"
                                :expanded="false"
                                :disabled="busy || saving || preparing"
                                :show-model="false"
                                @select="form.interfaceMode = $event as 'chat_completions' | 'responses'"
                            />
                            <p>选择优先使用的请求协议；关闭下方开关时只调用这一种协议。</p>
                            <label class="toggle-line">
                                <input
                                    id="protocol-fallback"
                                    v-model="form.protocolFallbackEnabled"
                                    class="switch"
                                    role="switch"
                                    type="checkbox"
                                    :disabled="busy || saving || preparing"
                                />
                                失败时切换另一种协议
                            </label>
                            <p>
                                新建配置默认开启。请求失败最多尝试另一种协议一次，可能多产生一次 API
                                调用；两次共用总超时，取消或正常返回后不重试，不改写首选协议。
                            </p>
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
                                    :disabled="busy || saving || preparing"
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
                            <label v-if="selectedProfile?.hasKey" class="toggle-line"
                                ><input
                                    v-model="form.removeKey"
                                    class="switch"
                                    role="switch"
                                    type="checkbox"
                                    :disabled="busy || saving || preparing"
                                />移除已保存的密钥</label
                            >
                        </div>
                    </div>
                    <div class="field-row">
                        <label for="model-name">模型名称</label>
                        <div class="field">
                            <ModelPicker
                                input-id="model-name"
                                label="模型名称"
                                context="settings"
                                editable
                                v-model="form.model"
                                :profiles="catalogOptions"
                                :selected-id="form.model"
                                :expanded="false"
                                :disabled="busy || saving || preparing || Boolean(diagnostic)"
                                :show-model="false"
                            />
                            <p>可手动输入或展开选择已缓存的模型 ID，名称不会自动替换；再次拉取更新列表。</p>
                            <div class="connection-actions">
                                <button
                                    type="button"
                                    class="button secondary small"
                                    :disabled="busy || saving || preparing || Boolean(diagnostic)"
                                    @click="runDiagnostic('models')"
                                >
                                    <RotateCcw :size="14" :class="{ spin: diagnostic === 'models' }" />{{
                                        diagnostic === 'models' ? '正在拉取' : '拉取模型'
                                    }}
                                </button>
                                <button
                                    type="button"
                                    class="button secondary small"
                                    :disabled="busy || saving || preparing || Boolean(diagnostic)"
                                    @click="runDiagnostic('connection')"
                                >
                                    <Zap :size="14" />{{
                                        diagnostic === 'connection' ? '正在测试' : '测试连接'
                                    }}
                                </button>
                            </div>
                            <p>
                                拉取模型不发起推理；测试连接发送独立短消息，可能产生少量 API
                                费用，不记录测智历史；使用当前配置有效超时（{{ draftEffectiveTimeout }}
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
                    <div class="field-row">
                        <label>思考强度</label>
                        <div class="field">
                            <ModelPicker
                                label="思考强度"
                                context="settings"
                                :profiles="reasoning.options"
                                :selected-id="form.reasoningEffort ?? ''"
                                :expanded="false"
                                :disabled="busy || saving || preparing"
                                :show-model="false"
                                @select="
                                    form.reasoningEffort = ($event || null) as typeof form.reasoningEffort
                                "
                            />
                            <p>默认 low。{{ reasoning.notice }}</p>
                        </div>
                    </div>
                    <div class="field-row">
                        <label for="profile-timeout">配置超时</label>
                        <div class="field">
                            <div class="number-control">
                                <input
                                    id="profile-timeout"
                                    v-model.number="form.timeoutSeconds"
                                    type="number"
                                    min="10"
                                    max="300"
                                    placeholder="继承全局"
                                    :disabled="busy || saving || preparing"
                                /><span>秒</span>
                            </div>
                            <p>留空时继承悬浮设置中的全局超时，测试连接使用同一有效值。</p>
                        </div>
                    </div>
                    <div class="field-row">
                        <label>绑定规则预设</label>
                        <div class="field">
                            <ModelPicker
                                label="绑定规则预设"
                                context="settings"
                                :profiles="[
                                    {
                                        id: '',
                                        name: snapshot?.config.defaultRuleName ?? '默认规则',
                                        model: '',
                                    },
                                    ...(snapshot?.config.rulePresets ?? []).map((item) => ({
                                        id: item.id,
                                        name: item.name,
                                        model: '',
                                    })),
                                ]"
                                :selected-id="form.rulePresetId ?? ''"
                                :expanded="false"
                                :disabled="busy || saving || preparing"
                                :show-model="false"
                                @select="form.rulePresetId = $event || null"
                            />
                            <p>绑定后该模型自动使用预设题目；历史仍保存实际规则文本。</p>
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
                    <div class="profile-toolbar">
                        <ModelPicker
                            label="编辑测试规则"
                            context="settings"
                            :profiles="[
                                {
                                    id: 'default',
                                    name: snapshot?.config.defaultRuleName ?? '默认规则',
                                    model: '',
                                },
                                ...(snapshot?.config.rulePresets ?? []).map((item) => ({
                                    id: item.id,
                                    name: item.name,
                                    model: '',
                                })),
                            ]"
                            :selected-id="ruleTargetId"
                            placeholder="新建规则预设（未保存）"
                            :expanded="false"
                            :disabled="saving || preparing || busy"
                            :show-model="false"
                            @select="changeRuleTarget"
                        />
                        <!-- 与接口配置共用工具栏：选择已有项和新建草稿保持独立，不混入下拉选项。 -->
                        <button
                            type="button"
                            class="button subtle small"
                            aria-label="新增规则预设"
                            :disabled="saving || preparing || busy"
                            @click="changeRuleTarget('new')"
                        >
                            <Plus :size="14" aria-hidden="true" />新增
                        </button>
                    </div>
                    <div class="field-row">
                        <label for="preset-name">预设名称</label>
                        <div class="field">
                            <input
                                id="preset-name"
                                v-model="presetName"
                                maxlength="80"
                                required
                                :disabled="saving || preparing || busy"
                                placeholder="例如：日本首相检查"
                            />
                            <p>
                                {{
                                    ruleTargetId === 'default'
                                        ? '默认规则可改名；沿用默认的模型仍保持绑定。'
                                        : '保存后可在接口配置或卡片中绑定；改名保留模型绑定关系。'
                                }}
                            </p>
                        </div>
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
                        :disabled="busy || saving || preparing"
                        rows="11"
                        placeholder="填写发送给模型的完整问题和要求…"
                    ></textarea>
                    <div class="keyword-field">
                        <div class="section-heading">
                            <label for="keyword">判定答案（任意一项命中即不降智）</label>
                            <button
                                type="button"
                                class="button subtle small"
                                :disabled="busy || saving || preparing || ruleAnswers(rule).length >= 20"
                                @click="addAnswer"
                            >
                                <Plus :size="14" aria-hidden="true" />添加答案
                            </button>
                        </div>
                        <div class="rule-answer-row">
                            <label class="sr-only" for="keyword">判定答案 1</label
                            ><textarea
                                id="keyword"
                                v-model="rule.keyword"
                                required
                                maxlength="500"
                                rows="2"
                                :disabled="busy || saving || preparing"
                                placeholder="例如：高市早苗"
                            ></textarea>
                            <button
                                v-if="rule.alternativeKeywords?.length"
                                type="button"
                                class="icon-button danger"
                                aria-label="删除答案 1"
                                :disabled="busy || saving || preparing"
                                @click="removeAnswer(0)"
                            >
                                <Trash2 :size="15" />
                            </button>
                        </div>
                        <div
                            v-for="(_, index) in rule.alternativeKeywords"
                            :key="index"
                            class="rule-answer-row"
                        >
                            <label class="sr-only" :for="`answer-${index + 2}`"
                                >判定答案 {{ index + 2 }}</label
                            >
                            <textarea
                                :id="`answer-${index + 2}`"
                                v-model="rule.alternativeKeywords![index]"
                                required
                                maxlength="500"
                                rows="2"
                                :disabled="busy || saving || preparing"
                                :placeholder="`判定答案 ${index + 2}`"
                            ></textarea>
                            <button
                                type="button"
                                class="icon-button danger"
                                :aria-label="`删除答案 ${index + 2}`"
                                :disabled="busy || saving || preparing"
                                @click="removeAnswer(index + 1)"
                            >
                                <Trash2 :size="15" />
                            </button>
                        </div>
                        <p class="field-help">
                            最多 20 项，每项 1—500 字符；区分大小写，保留空格与换行，不使用正则或语义匹配。
                        </p>
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
                    <p v-if="restoredMessage" class="muted small-text" role="status">{{ restoredMessage }}</p>
                </form>

                <form
                    v-else-if="page === 'widget'"
                    id="widget-settings"
                    class="preferences-form"
                    @submit.prevent="savePreferences"
                >
                    <div class="preference-row">
                        <div>
                            <label for="auto-test">定时测试</label>
                            <p>自动调用当前模型，收起后继续运行；可能产生 API 费用。</p>
                        </div>
                        <input
                            id="auto-test"
                            v-model="preferences.autoTestEnabled"
                            class="switch"
                            role="switch"
                            type="checkbox"
                            :disabled="busy || saving || preparing"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="auto-interval">测试间隔</label>
                            <p>默认 3 分钟；成功开始手动测试、开启定时或修改间隔后重新计时。</p>
                        </div>
                        <div class="number-control">
                            <input
                                id="auto-interval"
                                v-model.number="preferences.autoTestIntervalMinutes"
                                type="number"
                                min="1"
                                max="1440"
                                required
                                :disabled="busy || saving || preparing || !preferences.autoTestEnabled"
                            /><span>分钟</span>
                        </div>
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="timeout">请求超时</label>
                            <p>超时后终止等待；协议回退也共用这一总期限，不重新计时。</p>
                        </div>
                        <div class="number-control">
                            <input
                                id="timeout"
                                v-model.number="preferences.timeoutSeconds"
                                type="number"
                                min="10"
                                max="300"
                                required
                                :disabled="busy || saving || preparing"
                            /><span>秒</span>
                        </div>
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
                            role="switch"
                            type="checkbox"
                            :disabled="busy || saving || preparing"
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
                            role="switch"
                            type="checkbox"
                            :disabled="busy || saving || preparing"
                        />
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
                            role="switch"
                            type="checkbox"
                            :disabled="busy || saving || preparing"
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
                                :disabled="busy || saving || preparing"
                            /><span>{{ preferences.opacity }}%</span>
                        </div>
                    </div>
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
                            role="switch"
                            type="checkbox"
                            :checked="autostartEnabled"
                            :disabled="autostartBusy || !autostartReady"
                            @change="toggleAutostart"
                        />
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="history-max-records">历史最多保留</label>
                            <p>按最新记录清理；留空使用 5000 条硬上限，配置和凭据不受影响。</p>
                        </div>
                        <div class="number-control">
                            <input
                                id="history-max-records"
                                v-model.number="preferences.historyMaxRecords"
                                type="number"
                                min="1"
                                max="5000"
                                placeholder="5000"
                                :disabled="busy || saving || preparing"
                            /><span>条</span>
                        </div>
                    </div>
                    <div class="preference-row">
                        <div>
                            <label for="history-retention-days">历史最多保留天数</label>
                            <p>
                                留空不按日期清理；保存前预览影响并确认，数量、日期及 128 MiB
                                上限以先触发者为准。
                            </p>
                        </div>
                        <div class="number-control">
                            <input
                                id="history-retention-days"
                                v-model.number="preferences.historyRetentionDays"
                                type="number"
                                min="1"
                                max="3650"
                                placeholder="不限制"
                                :disabled="busy || saving || preparing"
                            /><span>天</span>
                        </div>
                    </div>
                    <div class="notice">
                        <Info :size="15" />关闭悬浮窗会隐藏到托盘；完全退出请使用托盘菜单。
                    </div>
                </form>

                <MailSettings
                    v-else-if="page === 'mail'"
                    ref="mailPanel"
                    :settings="snapshot?.config.emailNotifications"
                    :notification-error="snapshot?.emailNotificationError"
                    :disabled="busy || saving || preparing || refreshRequired"
                    :after-save="synchronizeMailSave"
                    @dirty="mailDirty = $event"
                />

                <AboutPanel
                    v-else-if="page === 'about'"
                    :result="updateResult"
                    :checking="checkingUpdate"
                    :error="updateError"
                    :checked-at="updateCheckedAt"
                    @open="openProjectLink"
                />
                <HistoryPanel
                    v-else
                    v-model="selectedRecord"
                    :history="snapshot?.history ?? []"
                    :profiles="snapshot?.config.profiles ?? []"
                    @filtered="filteredHistory = $event"
                />
            </div>

            <!-- 外部 form 关联保留浏览器原生必填校验，按钮区域独占固定行，不遮挡主体。 -->
            <footer class="form-footer settings-actions" aria-label="页面操作" :inert="refreshRequired">
                <template v-if="page === 'api'">
                    <button
                        v-if="editingId"
                        type="button"
                        class="button danger subtle small"
                        :disabled="saving || preparing || busy"
                        @click="deleteProfile"
                    >
                        <Trash2 :size="14" />{{ confirmDelete ? '再次点击确认删除' : '删除配置' }}
                    </button>
                    <span v-else class="muted small-text">保存后可在悬浮窗切换模型</span>
                    <button
                        class="button"
                        type="submit"
                        form="profile-settings"
                        :disabled="saving || preparing || busy"
                    >
                        <Save :size="15" />{{ saving ? '正在保存' : '保存配置' }}
                    </button>
                </template>
                <template v-else-if="page === 'rules'">
                    <div class="rule-footer-actions">
                        <button
                            v-if="ruleTargetId !== 'default' && ruleTargetId !== 'new'"
                            type="button"
                            class="button danger subtle small"
                            :disabled="saving || preparing || busy"
                            @click="deletePreset(ruleTargetId)"
                        >
                            <Trash2 :size="14" aria-hidden="true" />删除预设
                        </button>
                        <button
                            v-if="ruleTargetId !== 'new'"
                            type="button"
                            class="button subtle"
                            :disabled="saving || preparing || busy"
                            @click="restoreRule"
                        >
                            <RotateCcw :size="14" />{{
                                ruleTargetId === 'default' ? '恢复默认' : '还原已保存'
                            }}
                        </button>
                    </div>
                    <button
                        class="button"
                        type="submit"
                        form="rule-settings"
                        :disabled="saving || preparing || busy"
                    >
                        <Save :size="15" />{{ saving ? '正在保存' : '保存规则' }}
                    </button>
                </template>
                <template v-else-if="page === 'widget'">
                    <span class="muted small-text footer-hint">开机自启立即生效，其他设置保存后生效</span>
                    <button
                        type="button"
                        class="button secondary small"
                        :disabled="saving || preparing || busy"
                        @click="exportConfig"
                    >
                        备份配置
                    </button>
                    <input
                        ref="importInput"
                        class="sr-only"
                        type="file"
                        accept="application/json,.json"
                        aria-label="选择配置备份"
                        @change="importConfig"
                    />
                    <button
                        type="button"
                        class="button secondary small"
                        :disabled="saving || preparing || busy"
                        @click="chooseConfigImport"
                    >
                        导入配置
                    </button>
                    <button
                        class="button"
                        type="submit"
                        form="widget-settings"
                        :disabled="saving || preparing || busy"
                    >
                        <Save :size="15" />{{ saving ? '正在保存' : '保存设置' }}
                    </button>
                </template>
                <template v-else-if="page === 'mail'">
                    <span class="muted small-text footer-hint">授权码不进入配置备份，试发不会保存草稿</span>
                    <button
                        class="button"
                        type="submit"
                        form="mail-settings"
                        :disabled="busy || saving || preparing || mailPanel?.isWorking"
                    >
                        <Save :size="15" />{{ mailPanel?.isWorking ? '正在处理…' : '保存邮件设置' }}
                    </button>
                </template>
                <template v-else-if="page === 'about'">
                    <span class="muted small-text">免安装版 · Windows x64</span>
                    <div class="about-footer-actions">
                        <button
                            v-if="updateResult?.status === 'available'"
                            type="button"
                            class="button secondary"
                            @click="openProjectLink(updateResult.releaseUrl)"
                        >
                            <ArrowUpRight :size="15" />前往下载 v{{ updateResult.latestVersion }}
                        </button>
                        <button
                            type="button"
                            class="button"
                            :disabled="checkingUpdate"
                            @click="checkForUpdates"
                        >
                            <RefreshCw :size="15" :class="{ spin: checkingUpdate }" />{{
                                checkingUpdate ? '正在检查…' : '检查更新'
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
                    <button
                        v-if="selectedRecord.error"
                        type="button"
                        class="button secondary"
                        @click="copyDiagnostic"
                    >
                        <Copy :size="15" />复制诊断信息
                    </button>
                </template>
                <template v-else>
                    <div class="history-export-controls">
                        <ModelPicker
                            label="导出范围"
                            context="settings"
                            :profiles="[
                                { id: 'filtered', name: '当前筛选', model: '' },
                                { id: 'all', name: '全部历史', model: '' },
                            ]"
                            :selected-id="exportScope"
                            :expanded="false"
                            :disabled="false"
                            :show-model="false"
                            @select="exportScope = $event"
                        />
                        <ModelPicker
                            label="导出格式"
                            context="settings"
                            :profiles="[
                                { id: 'json', name: 'JSON（原文）', model: '' },
                                { id: 'csv', name: 'CSV（表格）', model: '' },
                            ]"
                            :selected-id="exportFormat"
                            :expanded="false"
                            :disabled="false"
                            :show-model="false"
                            @select="exportFormat = $event"
                        />
                        <button
                            type="button"
                            class="button secondary small"
                            :disabled="
                                !(exportScope === 'all' ? snapshot?.history.length : filteredHistory.length)
                            "
                            @click="exportHistory"
                        >
                            <Download :size="14" />导出
                        </button>
                    </div>
                    <button
                        type="button"
                        class="button subtle danger"
                        :disabled="!snapshot?.history.length || busy || saving || preparing"
                        @click="clearHistory"
                    >
                        <Trash2 :size="14" />{{ confirmClear ? '确认清空' : '清空' }}
                    </button>
                </template>
            </footer>
        </section>
        <ActionDialog
            v-if="dialog"
            :title="dialog.title"
            :message="dialog.message"
            :actions="dialog.actions"
            @choose="resolveDialog"
        >
            <div v-if="dialog.kind === 'import'" class="import-options">
                <p>相同 ID 的配置与预设如何处理：</p>
                <ModelPicker
                    label="重复配置处理方式"
                    context="settings"
                    :profiles="[
                        { id: 'skip', name: '保留现有（跳过重复）', model: '' },
                        { id: 'replace', name: '替换重复项（保留现有密钥）', model: '' },
                        { id: 'copy', name: '作为副本导入（不含密钥）', model: '' },
                    ]"
                    :selected-id="importStrategy"
                    :expanded="false"
                    :disabled="false"
                    :show-model="false"
                    @select="importStrategy = $event"
                />
            </div>
        </ActionDialog>
    </main>
</template>
