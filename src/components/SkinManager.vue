<!-- 设置中的皮肤管理：预览只作用于本页，保存后才切换真实桌宠；导入包无脚本。@author ahui -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue';
import { Upload, Trash2, Check } from 'lucide-vue-next';
import { call, desktop } from '../bridge';
import { BUILTIN_SKINS, loadSkin, validatePack } from '../pet/skinLoader';
import {
    DEFAULT_SKIN,
    type LoadedSkin,
    type PetAction,
    type SkinEntry,
    type SkinPreferences,
} from '../pet/types';
import PetActor from './PetActor.vue';
import { resolveIdleAction } from '../pet/controller';
const props = defineProps<{ modelValue?: SkinPreferences; disabled: boolean }>();
const emit = defineEmits<{ 'update:modelValue': [value: SkinPreferences]; working: [value: boolean] }>();
const selected = computed(() => ({ ...DEFAULT_SKIN, ...props.modelValue }));
const entries = ref<SkinEntry[]>([...BUILTIN_SKINS]);
const loaded = shallowRef<LoadedSkin | null>(null);
const action = ref<PetAction>('idle');
const error = ref(''),
    message = ref(''),
    working = ref(false);
const input = ref<HTMLInputElement>();
const pendingRemoval = ref<string | null>(null);
let generation = 0,
    disposed = false;
const actions: { id: PetAction; label: string }[] = [
    { id: 'idle', label: '待机' },
    { id: 'petting', label: '摸头' },
    { id: 'dragging', label: '拖动' },
    { id: 'working', label: '测试中' },
    { id: 'success', label: '通过' },
    { id: 'seated', label: '侧坐' },
    { id: 'wave', label: '招手' },
    { id: 'stretch', label: '伸懒腰' },
    { id: 'landing', label: '放下站稳' },
    { id: 'bow', label: '鞠躬' },
    { id: 'failed', label: '降智' },
    { id: 'error', label: '测试失败' },
];
// 所有已提供的动作都可作为常态外观；悬空和结果姿态仅绘图，不模拟拖动或测试结算。
const idleActions = computed(() => actions.filter((item) => loaded.value?.manifest.actions[item.id]));
const defaultIdle = computed(() =>
    resolveIdleAction(selected.value.idleAction, Object.keys(loaded.value?.manifest.actions ?? {})),
);

/** 修改默认姿态时立即在本页预览；保存仍统一经过设置页的偏好校验和持久化入口。 */
function selectIdle(event: Event): void {
    const next = resolveIdleAction(
        (event.target as HTMLSelectElement).value,
        Object.keys(loaded.value?.manifest.actions ?? {}),
    );
    update({ idleAction: next });
    action.value = next;
}
function update(change: Partial<SkinPreferences>): void {
    emit('update:modelValue', { ...selected.value, ...change });
}
async function list(): Promise<void> {
    if (desktop) entries.value = [...BUILTIN_SKINS, ...(await call<SkinEntry[]>('list_skins'))];
}
watch(
    () => selected.value.id,
    async (id) => {
        const current = ++generation;
        error.value = '';
        action.value = 'idle';
        if (id === 'classic') {
            loaded.value?.dispose();
            loaded.value = null;
            return;
        }
        try {
            const next = await loadSkin(id);
            if (disposed || current !== generation) {
                next.dispose();
                return;
            }
            loaded.value?.dispose();
            loaded.value = next;
            action.value = resolveIdleAction(selected.value.idleAction, Object.keys(next.manifest.actions));
        } catch (cause) {
            if (!disposed && current === generation) {
                loaded.value?.dispose();
                loaded.value = null;
                error.value = String(cause);
            }
        }
    },
    { immediate: true },
);
watch(working, (value) => emit('working', value));

/** 本页先验证全部引用图集，再原子保存；导入不自动启用、不改变测试状态。 */
async function importSkin(event: Event): Promise<void> {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (input.value) input.value.value = '';
    if (!file || working.value || props.disabled) return;
    working.value = true;
    error.value = '';
    message.value = '';
    let preview: LoadedSkin | undefined;
    try {
        if (file.size > 48 * 1024 ** 2) throw new Error('皮肤包不能超过 48 MiB。');
        const pack = validatePack(JSON.parse(await file.text()));
        preview = await loadSkin(pack.manifest.id, pack);
        for (const a of Object.keys(pack.manifest.actions)) await preview.image(a as PetAction);
        await call('install_skin', { pack });
        await list();
        message.value = '皮肤已导入，选择并保存后启用。';
    } catch (cause) {
        error.value = String(cause);
    } finally {
        preview?.dispose();
        working.value = false;
    }
}
async function remove(id: string): Promise<void> {
    if (working.value || props.disabled) return;
    working.value = true;
    error.value = '';
    try {
        await call('remove_skin', { id });
        pendingRemoval.value = null;
        await list();
        message.value = '皮肤已移除。';
    } catch (cause) {
        error.value = String(cause);
    } finally {
        working.value = false;
    }
}
onMounted(
    () =>
        void list().catch((cause) => {
            error.value = String(cause);
        }),
);
onUnmounted(() => {
    disposed = true;
    generation++;
    loaded.value?.dispose();
});
</script>
<template>
    <div class="skin-manager">
        <div class="skin-toolbar">
            <p class="muted">选择皮肤并预览动作，保存后应用到桌面。</p>
            <button
                type="button"
                class="button secondary small"
                :disabled="disabled || working || !desktop"
                @click="input?.click()"
            >
                <Upload :size="14" />导入皮肤
            </button>
            <input
                ref="input"
                type="file"
                accept=".json"
                class="sr-only"
                aria-label="选择皮肤资源包"
                @change="importSkin"
            />
        </div>
        <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
        <p v-if="message" class="success-message" role="status">{{ message }}</p>
        <div class="skin-cards">
            <div
                v-for="item in entries"
                :key="item.id"
                class="skin-card"
                :class="{ selected: selected.id === item.id }"
            >
                <button
                    type="button"
                    :disabled="disabled || working"
                    :aria-pressed="selected.id === item.id"
                    @click="update({ id: item.id })"
                >
                    <img
                        v-if="item.id === 'blue-whale-maid'"
                        :src="'/skins/blue-whale-maid/idle.png'"
                        alt="蓝鲸女仆"
                    />
                    <div v-else class="skin-card-symbol">{{ item.id === 'classic' ? '◎' : '◇' }}</div>
                    <strong>{{ item.name }} <Check v-if="selected.id === item.id" :size="15" /></strong>
                    <span>{{ item.builtin ? '内置皮肤' : `作者 ${item.author}` }}</span>
                </button>
                <button
                    v-if="!item.builtin"
                    type="button"
                    class="skin-delete"
                    :aria-label="`移除皮肤 ${item.name}`"
                    :disabled="disabled || working || item.id === selected.id"
                    @click="pendingRemoval = item.id"
                >
                    <Trash2 :size="14" />
                </button>
            </div>
        </div>
        <div v-if="pendingRemoval" class="skin-removal" role="group" aria-label="确认移除皮肤">
            <p>移除此皮肤的本机资源？之后可用原资源包重新导入。</p>
            <button
                type="button"
                class="button secondary small"
                :disabled="working"
                @click="pendingRemoval = null"
            >
                取消
            </button>
            <button
                type="button"
                class="button danger small"
                :disabled="working || disabled"
                @click="remove(pendingRemoval)"
            >
                确认移除
            </button>
        </div>
        <section v-if="selected.id !== 'classic'" class="skin-preview-section">
            <div class="skin-live-preview">
                <PetActor
                    v-if="loaded"
                    :skin="loaded"
                    :action="action"
                    :animate="selected.animation"
                    :size="selected.size"
                    @error="error = $event"
                />
                <span v-else class="muted">{{ error ? '预览不可用' : '正在加载…' }}</span>
            </div>
            <div class="skin-preview-controls">
                <h2>角色预览</h2>
                <p class="muted">预览不会触发模型测试或修改桌面位置。</p>
                <div class="skin-action-list">
                    <button
                        v-for="a in actions"
                        :key="a.id"
                        type="button"
                        class="button secondary small"
                        :class="{ active: action === a.id }"
                        @click="action = a.id"
                    >
                        {{ a.label }}
                    </button>
                </div>
                <label class="skin-idle-label" for="skin-idle-action">默认待机动作</label>
                <select
                    id="skin-idle-action"
                    class="skin-idle-select"
                    :value="defaultIdle"
                    :disabled="disabled || working || !loaded"
                    aria-describedby="skin-idle-help"
                    @change="selectIdle"
                >
                    <option v-for="item in idleActions" :key="item.id" :value="item.id">
                        {{ item.label }}
                    </option>
                </select>
                <p id="skin-idle-help" class="muted">
                    当前皮肤的全部动作均可设为待机外观，不改变真实测试状态。互动结束后回到所选姿态；展开卡片仍侧坐，缺少资源时使用普通待机。
                </p>
                <label class="skin-size"
                    >角色尺寸 <output>{{ selected.size }} px</output
                    ><input
                        type="range"
                        min="96"
                        max="192"
                        step="8"
                        :value="selected.size"
                        :disabled="disabled"
                        @input="update({ size: Number(($event.target as HTMLInputElement).value) })"
                /></label>
                <label class="skin-option"
                    ><input
                        type="checkbox"
                        :checked="selected.animation"
                        :disabled="disabled"
                        @change="update({ animation: ($event.target as HTMLInputElement).checked })"
                    />启用角色动画</label
                >
                <label class="skin-option"
                    ><input
                        type="checkbox"
                        :checked="selected.autonomous"
                        :disabled="disabled"
                        @change="update({ autonomous: ($event.target as HTMLInputElement).checked })"
                    />空闲时自主招手、伸懒腰与休息</label
                >
                <label class="skin-option"
                    ><input
                        type="checkbox"
                        :checked="selected.bubbles"
                        :disabled="disabled"
                        @change="update({ bubbles: ($event.target as HTMLInputElement).checked })"
                    />显示临时互动提示</label
                >
            </div>
        </section>
    </div>
</template>
