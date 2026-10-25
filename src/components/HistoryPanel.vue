<!-- 历史浏览器统一过滤本机日期、展示请求快照并复用安全 Markdown；不重新判定旧回答。@author ahui -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { ArrowUpRight, Search, History, Clock3 } from 'lucide-vue-next';
import { formatTime } from '../useProbe';
import { localDate, statusText, reasoningLabel } from '../historyExport';
import { ruleAnswers } from '../rules';
import type { ModelProfile, TestRecord } from '../types';
import ModelPicker from './ModelPicker.vue';
import MarkdownAnswer from './MarkdownAnswer.vue';
import DatePicker from './DatePicker.vue';
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
const historySearch = ref('');
const historyGroup = ref('');
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
const filteredHistory = computed(() => {
    const query = historySearch.value.toLocaleLowerCase();
    return props.history.filter((item) => {
        if (
            (historyGroup.value && item.profileId !== historyGroup.value) ||
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
            `${item.profileName} ${item.model} ${item.answer} ${item.ruleName ?? ''} ${ruleAnswers(item).join(' ')}`
                .toLocaleLowerCase()
                .includes(query)
        );
    });
});
watch(historyGroups, (groups) => {
    if (!groups.some((group) => group.id === historyGroup.value)) historyGroup.value = '';
});

// 将当前可见集合提供给固定底栏导出，原始记录对象始终保持完整，筛选不截断回答。
watch(filteredHistory, (records) => emit('filtered', records), { immediate: true });
</script>
<template>
    <div class="history-page">
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
                <dt>测试来源</dt>
                <dd>
                    {{
                        selectedRecord.source === 'automatic'
                            ? '自动测试'
                            : selectedRecord.source === 'manual'
                              ? '手动测试'
                              : '未记录'
                    }}
                </dd>
                <dt>接口模式</dt>
                <dd>
                    {{
                        selectedRecord.interfaceMode === 'responses'
                            ? 'Responses'
                            : selectedRecord.interfaceMode === 'chat_completions'
                              ? 'Chat Completions'
                              : '未记录'
                    }}
                </dd>
                <dt>失败切换协议</dt>
                <dd>
                    {{
                        selectedRecord.protocolFallbackEnabled == null
                            ? '未记录'
                            : selectedRecord.protocolFallbackEnabled
                              ? '开启'
                              : '关闭'
                    }}
                </dd>
                <dt>协议尝试顺序</dt>
                <dd>
                    {{
                        selectedRecord.attemptedModes?.length
                            ? selectedRecord.attemptedModes
                                  .map((mode) =>
                                      mode === 'responses'
                                          ? 'Responses'
                                          : mode === 'chat_completions'
                                            ? 'Chat Completions'
                                            : '未知协议',
                                  )
                                  .join(' → ')
                            : '未记录'
                    }}
                </dd>
                <dt>思考强度</dt>
                <dd>{{ reasoningLabel(selectedRecord) }}</dd>
                <dt>有效超时</dt>
                <dd>
                    {{
                        selectedRecord.effectiveTimeoutSeconds
                            ? `${selectedRecord.effectiveTimeoutSeconds} 秒`
                            : '未记录'
                    }}
                </dd>
                <template v-if="selectedRecord.httpStatus != null"
                    ><dt>HTTP 状态码</dt>
                    <dd>{{ selectedRecord.httpStatus }}</dd></template
                >
                <template v-if="selectedRecord.requestId"
                    ><dt>服务商请求编号</dt>
                    <dd class="pre-wrap">{{ selectedRecord.requestId }}</dd></template
                >
                <template v-if="selectedRecord.errorKind"
                    ><dt>错误类型</dt>
                    <dd>{{ selectedRecord.errorKind }}</dd></template
                >
                <dt>规则预设</dt>
                <dd>{{ selectedRecord.ruleName ?? '未记录' }}</dd>
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
            <p v-if="historyFrom && historyTo && historyFrom > historyTo" class="inline-error" role="alert">
                开始日期不能晚于结束日期。
            </p>
            <div v-if="!filteredHistory.length" class="empty-state">
                <History :size="34" />
                <h2>
                    {{
                        historySearch ||
                        historyGroup ||
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
            <button
                v-for="record in filteredHistory"
                :key="record.id"
                class="history-item"
                @click="selectedRecord = record"
            >
                <div>
                    <span class="history-model">{{ record.profileName }}</span
                    ><span class="result-pill" :class="record.status">{{ statusText[record.status] }}</span>
                    <p>{{ record.model }}</p>
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
                <ArrowUpRight :size="16" />
            </button>
        </template>
    </div>
</template>
