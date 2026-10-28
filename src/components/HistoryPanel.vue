<!-- 历史浏览器过滤本机日期、展示请求快照和文字/HTML 回答；可见行复用卡片预览，不重判旧记录。@author ahui -->
<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue';
import { ArrowUpRight, Search, History, Clock3 } from 'lucide-vue-next';
import { formatTime } from '../useProbe';
import { localDate, statusText, reasoningLabel } from '../historyExport';
import { ruleAnswers } from '../rules';
import type { ModelProfile, TestRecord } from '../types';
import ModelPicker from './ModelPicker.vue';
import AnswerContent from './AnswerContent.vue';
import HistoryAnswerPreview from './HistoryAnswerPreview.vue';
import { previewAllowed } from '../htmlAnswer';
import DatePicker from './DatePicker.vue';
import { call } from '../bridge';
const previewError = ref('');
const props = defineProps<{
    history: TestRecord[];
    profiles: ModelProfile[];
    modelValue: TestRecord | null;
    /** 桌面传入摘要及数据库序号；本地布局预览可继续传入完整记录。 */
    remote?: boolean;
    sequences?: Record<string, number>;
}>();
const emit = defineEmits<{
    'update:modelValue': [value: TestRecord | null];
    filtered: [records: TestRecord[]];
    working: [value: boolean];
    refresh: [];
}>();
const selectedRecord = computed({
    get: () => props.modelValue,
    set: (value) => emit('update:modelValue', value),
});
/** 详情只读取当次请求快照；按重要程度先呈现卡片的四项选择，缺失字段明确标为未记录。 */
const detailFields = computed(() => {
    const record = selectedRecord.value;
    if (!record) return [];
    const protocol = (mode: string | null | undefined): string =>
        mode === 'responses' ? 'Responses' : mode === 'chat_completions' ? 'Chat Completions' : '未记录';
    const fields = [
        { label: '接口分组', value: record.profileName },
        { label: '模型', value: record.model },
        { label: '规则预设', value: record.ruleName || '未记录' },
        { label: '思考强度', value: reasoningLabel(record) },
        { label: '请求耗时', value: `${(record.durationMs / 1000).toFixed(2)} 秒` },
        {
            label: '测试来源',
            value:
                record.source === 'automatic'
                    ? '自动测试'
                    : record.source === 'manual'
                      ? '手动测试'
                      : '未记录',
        },
        { label: '接口模式', value: protocol(record.interfaceMode) },
        {
            label: '失败切换协议',
            value:
                record.protocolFallbackEnabled == null
                    ? '未记录'
                    : record.protocolFallbackEnabled
                      ? '开启'
                      : '关闭',
        },
        {
            label: '协议尝试顺序',
            value: record.attemptedModes?.length ? record.attemptedModes.map(protocol).join(' → ') : '未记录',
        },
        {
            label: '有效超时',
            value: record.effectiveTimeoutSeconds ? `${record.effectiveTimeoutSeconds} 秒` : '未记录',
        },
    ];
    if (record.httpStatus != null) fields.push({ label: 'HTTP 状态码', value: String(record.httpStatus) });
    if (record.requestId) fields.push({ label: '服务商请求编号', value: record.requestId });
    if (record.errorKind) fields.push({ label: '错误类型', value: record.errorKind });
    return fields;
});
watch(
    () => props.modelValue?.id,
    () => {
        previewError.value = '';
    },
);
const historySearch = ref('');
const historyGroup = ref('');
const historyModel = ref('');
const historyRule = ref('');
const historyEffort = ref('');
const historyStatus = ref('');
const historySource = ref('');
const historyFrom = ref('');
const historyTo = ref('');
// 检索只在停止输入 250ms 后提交；世代隔离让旧搜索/旧页不能覆盖当前筛选。
const searchIds = shallowRef<Set<string> | null>(null);
const searchPending = ref(false);
const pagePending = ref(false);
const historyError = ref('');
const currentPage = ref(1);
const pageSize = 10;
const pageRecords = shallowRef<TestRecord[]>([]);
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let searchGeneration = 0;
let pageGeneration = 0;
let disposed = false;
// 同一组件只允许一条全文扫描在途；较新的输入合并到它之后，避免慢盘上排队积压。
let searchInFlight: Promise<string[]> | undefined;
async function searchWhenIdle(query: string, generation: number): Promise<void> {
    if (searchInFlight) await searchInFlight.catch(() => {});
    if (disposed || generation !== searchGeneration) return;
    const request = call<string[]>('search_history', { query });
    searchInFlight = request;
    try {
        const ids = await request;
        if (!disposed && generation === searchGeneration) searchIds.value = new Set(ids);
    } catch (cause) {
        if (!disposed && generation === searchGeneration) historyError.value = String(cause);
    } finally {
        if (searchInFlight === request) searchInFlight = undefined;
        if (!disposed && generation === searchGeneration) searchPending.value = false;
    }
}
watch(
    [historySearch, () => props.history],
    () => {
        if (!props.remote) return;
        const generation = ++searchGeneration;
        clearTimeout(searchTimer);
        historyError.value = '';
        searchIds.value = null;
        searchPending.value = Boolean(historySearch.value);
        if (!historySearch.value) return;
        const query = historySearch.value;
        searchTimer = setTimeout(() => void searchWhenIdle(query, generation), 250);
    },
    { immediate: true },
);
/** 历史按稳定配置 ID 筛选；当前已删除的配置仍以历史快照名称提供筛选。 */
const historyGroups = computed(() => {
    const groups = new Map<string, string>();
    const counts = new Map<string, number>();
    for (const item of props.profiles) groups.set(item.id, item.name);
    for (const record of props.history) {
        counts.set(record.profileId, (counts.get(record.profileId) ?? 0) + 1);
        if (!groups.has(record.profileId)) groups.set(record.profileId, record.profileName);
    }
    return [
        { id: '', name: '全部分组', model: '显示所有测试记录' },
        ...Array.from(groups, ([id, name]) => ({
            id,
            name,
            model: `${counts.get(id) ?? 0} 条记录`,
        })),
    ];
});
/** 候选来自已保存快照，保留已删模型/规则；规则按当时名称筛选，不反查当前配置。 */
const historyModels = computed(() => [
    { id: '', name: '全部模型', model: '' },
    ...[...new Set(props.history.map((record) => record.model))]
        .sort()
        .map((model) => ({ id: model, name: model, model: '' })),
]);
// JSON 元组编码区分“未记录”和任意用户名称，避免哨兵字符串与合法规则名冲突。
const ruleFilterKey = (record: TestRecord): string => JSON.stringify([record.ruleName || null]);
const historyRules = computed(() => {
    const rules = new Map(
        props.history.map((record) => [ruleFilterKey(record), record.ruleName || '未记录']),
    );
    return [
        { id: '', name: '全部规则预设', model: '' },
        ...[...rules]
            .sort((left, right) => left[1].localeCompare(right[1]))
            .map(([id, name]) => ({ id, name, model: '' })),
    ];
});
const historyEfforts = computed(() => [
    { id: '', name: '全部思考强度', model: '' },
    ...[...new Set(props.history.map(reasoningLabel))]
        .sort()
        .map((effort) => ({ id: effort, name: effort, model: '' })),
]);
const filteredHistory = computed(() => {
    const query = historySearch.value.toLocaleLowerCase();
    return props.history.filter((item) => {
        if (
            (historyGroup.value && item.profileId !== historyGroup.value) ||
            (historyModel.value && item.model !== historyModel.value) ||
            (historyRule.value && ruleFilterKey(item) !== historyRule.value) ||
            (historyEffort.value && reasoningLabel(item) !== historyEffort.value) ||
            (historyStatus.value && item.status !== historyStatus.value) ||
            (historySource.value && item.source !== historySource.value)
        )
            return false;
        if (historyFrom.value || historyTo.value) {
            const date = localDate(item.startedAt);
            if (
                !date ||
                (historyFrom.value && date < historyFrom.value) ||
                (historyTo.value && date > historyTo.value)
            )
                return false;
        }
        // 历史正文可达 128 MiB；没有搜索词时不能为了匹配空串逐条复制、转小写整段回答。
        return (
            !query ||
            (props.remote
                ? Boolean(searchIds.value?.has(item.id))
                : `${item.profileName} ${item.model} ${reasoningLabel(item)} ${item.answer} ${item.ruleName ?? ''} ${ruleAnswers(item).join(' ')}`
                      .toLocaleLowerCase()
                      .includes(query))
        );
    });
});
watch(historyGroups, (groups) => {
    if (!groups.some((group) => group.id === historyGroup.value)) historyGroup.value = '';
});
// 清空/保留策略删除最后一个候选时撤销失效筛选，避免界面留着不存在的隐藏过滤条件。
watch(historyModels, (models) => {
    if (!models.some((item) => item.id === historyModel.value)) historyModel.value = '';
});
watch(historyRules, (rules) => {
    if (!rules.some((item) => item.id === historyRule.value)) historyRule.value = '';
});
watch(historyEfforts, (efforts) => {
    if (!efforts.some((item) => item.id === historyEffort.value)) historyEffort.value = '';
});

// 底栏只用筛选集合的稳定 ID 选择导出范围，实际导出重新读取完整正文，不受分页限制。
watch(
    filteredHistory,
    (records) => {
        currentPage.value = 1;
        emit('filtered', records);
    },
    { immediate: true },
);
const pageCount = computed(() => Math.max(1, Math.ceil(filteredHistory.value.length / pageSize)));
const pageItems = computed(() =>
    filteredHistory.value.slice((currentPage.value - 1) * pageSize, currentPage.value * pageSize),
);
const visibleRecords = computed(() => (props.remote ? pageRecords.value : pageItems.value));

/** 原文仅为当前十条加载，翻页立即释放旧页；详情使用已加载对象，复制不依赖摘要。 */
async function loadPage(): Promise<void> {
    const generation = ++pageGeneration;
    if (props.remote && !selectedRecord.value) pageRecords.value = [];
    if (!props.remote || selectedRecord.value || searchPending.value) {
        pagePending.value = false;
        return;
    }
    pageRecords.value = [];
    pagePending.value = true;
    if (!historySearch.value || searchIds.value) historyError.value = '';
    try {
        const sequences = pageItems.value.map((record) => props.sequences?.[record.id]);
        if (sequences.some((sequence) => sequence == null)) throw new Error('历史索引已变化，请刷新后重试。');
        const records = sequences.length
            ? await call<TestRecord[]>('get_history_records', { sequences })
            : [];
        if (!disposed && generation === pageGeneration) pageRecords.value = records;
    } catch (cause) {
        if (!disposed && generation === pageGeneration) historyError.value = String(cause);
    } finally {
        if (!disposed && generation === pageGeneration) pagePending.value = false;
    }
}
watch([pageItems, () => props.sequences, selectedRecord, searchPending], () => void loadPage(), {
    immediate: true,
});
watch(
    [searchPending, pagePending, historyError],
    () => emit('working', searchPending.value || pagePending.value || Boolean(historyError.value)),
    { immediate: true },
);
onBeforeUnmount(() => {
    disposed = true;
    searchGeneration++;
    pageGeneration++;
    clearTimeout(searchTimer);
    emit('working', false);
});
</script>
<template>
    <div class="history-page">
        <div v-if="selectedRecord" class="history-detail">
            <div class="history-detail-title">
                <span class="result-pill" :class="selectedRecord.status">{{
                    selectedRecord.status === 'html' && previewError
                        ? 'HTML 预览失败'
                        : statusText[selectedRecord.status]
                }}</span
                ><span>{{ formatTime(selectedRecord.startedAt) }}</span>
            </div>
            <!-- 简短请求快照采用自适应多列，长题目/回答保持全宽，避免关键信息挤成一长列。 -->
            <dl class="history-detail-summary">
                <div v-for="field in detailFields" :key="field.label">
                    <dt>{{ field.label }}</dt>
                    <dd>{{ field.value }}</dd>
                </div>
            </dl>
            <dl class="history-detail-content">
                <dt>判定答案（任一命中）</dt>
                <dd>
                    <ol class="history-answers">
                        <li
                            v-for="(answer, index) in ruleAnswers(selectedRecord)"
                            :key="index"
                            class="pre-wrap"
                        >
                            {{ answer }}
                        </li>
                    </ol>
                </dd>
                <dt>当时的完整题目</dt>
                <dd class="history-text">{{ selectedRecord.prompt }}</dd>
                <dt>模型回答</dt>
                <dd class="history-text history-answer">
                    <AnswerContent
                        :source="selectedRecord.answer || '无正文'"
                        :answer-type="selectedRecord.answerType"
                        :executable="previewAllowed(selectedRecord)"
                        @failure="previewError = $event"
                    />
                </dd>
                <template v-if="selectedRecord.error"
                    ><dt>错误说明</dt>
                    <dd>{{ selectedRecord.error }}</dd></template
                >
            </dl>
        </div>
        <template v-else>
            <div class="history-toolbar">
                <div class="history-context-filters">
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
                    <ModelPicker
                        label="按模型筛选历史"
                        context="settings"
                        :profiles="historyModels"
                        :selected-id="historyModel"
                        :expanded="false"
                        :disabled="false"
                        :show-model="false"
                        @select="historyModel = $event"
                    />
                    <ModelPicker
                        label="按规则预设筛选历史"
                        context="settings"
                        :profiles="historyRules"
                        :selected-id="historyRule"
                        :expanded="false"
                        :disabled="false"
                        :show-model="false"
                        @select="historyRule = $event"
                    />
                    <ModelPicker
                        label="按思考强度筛选历史"
                        context="settings"
                        :profiles="historyEfforts"
                        :selected-id="historyEffort"
                        :expanded="false"
                        :disabled="false"
                        :show-model="false"
                        @select="historyEffort = $event"
                    />
                </div>
                <div class="history-secondary-filters">
                    <div class="input-affix">
                        <Search :size="15" /><input
                            v-model="historySearch"
                            placeholder="搜索分组、模型、规则、强度或回答"
                            aria-label="搜索历史"
                        />
                    </div>
                    <ModelPicker
                        label="按结果筛选历史"
                        context="settings"
                        :profiles="[
                            { id: '', name: '全部结果', model: '' },
                            ...Object.entries(statusText).map(([id, name]) => ({
                                id,
                                name,
                                model: '',
                            })),
                        ]"
                        :selected-id="historyStatus"
                        :expanded="false"
                        :disabled="false"
                        :show-model="false"
                        @select="historyStatus = $event"
                    />
                    <ModelPicker
                        label="按测试来源筛选历史"
                        context="settings"
                        :profiles="[
                            { id: '', name: '全部来源', model: '' },
                            { id: 'manual', name: '手动测试', model: '' },
                            { id: 'automatic', name: '自动测试', model: '' },
                        ]"
                        :selected-id="historySource"
                        :expanded="false"
                        :disabled="false"
                        :show-model="false"
                        @select="historySource = $event"
                    />
                    <div class="history-date-filter">
                        <span>从</span>
                        <DatePicker v-model="historyFrom" label="历史开始日期" />
                    </div>
                    <div class="history-date-filter">
                        <span>到</span>
                        <DatePicker v-model="historyTo" label="历史结束日期" />
                    </div>
                </div>
            </div>
            <p v-if="historyFrom && historyTo && historyFrom > historyTo" class="inline-error" role="alert">
                开始日期不能晚于结束日期。
            </p>
            <p v-if="searchPending || pagePending" class="muted" role="status">
                {{ searchPending ? '正在搜索…' : '正在读取记录…' }}
            </p>
            <p v-if="historyError" class="inline-error" role="alert">
                {{ historyError }}
                <button type="button" class="button subtle small" @click="$emit('refresh')">刷新历史</button>
            </p>
            <div
                v-if="!filteredHistory.length && !searchPending && !pagePending && !historyError"
                class="empty-state"
            >
                <History :size="34" />
                <h2>
                    {{
                        historySearch ||
                        historyGroup ||
                        historyModel ||
                        historyRule ||
                        historyEffort ||
                        historyStatus ||
                        historySource ||
                        historyFrom ||
                        historyTo
                            ? '没有匹配的记录'
                            : '还没有测试记录'
                    }}
                </h2>
                <p>完成一次测试后，可在这里查看回答与规则快照。</p>
            </div>
            <nav v-if="filteredHistory.length" class="history-pagination" aria-label="历史分页">
                <span>共 {{ filteredHistory.length }} 条 · 第 {{ currentPage }} / {{ pageCount }} 页</span>
                <button
                    type="button"
                    class="button subtle small"
                    :disabled="currentPage <= 1 || pagePending || searchPending"
                    @click="currentPage--"
                >
                    上一页
                </button>
                <button
                    type="button"
                    class="button subtle small"
                    :disabled="currentPage >= pageCount || pagePending || searchPending"
                    @click="currentPage++"
                >
                    下一页
                </button>
            </nav>
            <div v-for="record in visibleRecords" :key="record.id" class="history-item">
                <!-- 元信息与预览分别操作，避免放大、片段选择和重载被整行点击带入详情。 -->
                <div
                    class="history-record-info"
                    role="button"
                    tabindex="0"
                    @click="selectedRecord = record"
                    @keydown.enter.self.prevent="selectedRecord = record"
                    @keydown.space.self.prevent="selectedRecord = record"
                >
                    <div class="history-record-title">
                        <span class="history-model" :title="record.profileName">{{
                            record.profileName
                        }}</span>
                        <span class="result-pill" :class="record.status">{{
                            statusText[record.status]
                        }}</span>
                    </div>
                    <dl class="history-record-context">
                        <div class="history-record-model">
                            <dt>模型</dt>
                            <dd :title="record.model">{{ record.model }}</dd>
                        </div>
                        <div>
                            <dt>规则</dt>
                            <dd :title="record.ruleName || '未记录'">{{ record.ruleName || '未记录' }}</dd>
                        </div>
                        <div>
                            <dt>思考强度</dt>
                            <dd>{{ reasoningLabel(record) }}</dd>
                        </div>
                    </dl>
                    <span class="history-time"
                        ><Clock3 :size="11" />{{ formatTime(record.startedAt) }} ·
                        {{ (record.durationMs / 1000).toFixed(2) }}s ·
                        {{
                            record.source === 'automatic'
                                ? '自动'
                                : record.source === 'manual'
                                  ? '手动'
                                  : '未记录'
                        }}</span
                    >
                </div>
                <HistoryAnswerPreview :record="record" @details="selectedRecord = record" />
                <button
                    type="button"
                    class="icon-button history-record-details"
                    :aria-label="`查看 ${record.profileName} 的测试详情`"
                    @click="selectedRecord = record"
                >
                    <ArrowUpRight :size="16" />
                </button>
            </div>
        </template>
    </div>
</template>
