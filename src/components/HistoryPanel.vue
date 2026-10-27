<!-- 历史浏览器过滤本机日期、展示请求快照和文字/HTML 回答；可见行复用卡片预览，不重判旧记录。@author ahui -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
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
const previewError = ref('');
const props = defineProps<{
    history: TestRecord[];
    profiles: ModelProfile[];
    modelValue: TestRecord | null;
}>();
const emit = defineEmits<{
    'update:modelValue': [value: TestRecord | null];
    filtered: [records: TestRecord[]];
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
            `${item.profileName} ${item.model} ${reasoningLabel(item)} ${item.answer} ${item.ruleName ?? ''} ${ruleAnswers(item).join(' ')}`
                .toLocaleLowerCase()
                .includes(query)
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

// 将当前可见集合提供给固定底栏导出，原始记录对象始终保持完整，筛选不截断回答。
watch(filteredHistory, (records) => emit('filtered', records), { immediate: true });
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
            <div v-if="!filteredHistory.length" class="empty-state">
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
            <div v-for="record in filteredHistory" :key="record.id" class="history-item">
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
