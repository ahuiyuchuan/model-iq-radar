<!--
    SMTP 草稿独立于模型请求与其他设置；密码只写入本地 IPC，从不回读或放入配置备份。
    @author ahui
-->
<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { call } from '../bridge';
import type { EmailNotificationSettings, EmailSettingsInput } from '../types';
import ModelPicker from './ModelPicker.vue';

const props = defineProps<{
    settings?: EmailNotificationSettings;
    disabled: boolean;
    notificationError?: string | null;
    /** 写入后等待父级同步权威快照；false 表示已保存但需重读，禁止据此离页或重复写入。 */
    afterSave: () => Promise<boolean>;
}>();
const emit = defineEmits<{ dirty: [value: boolean] }>();
const defaults = (): EmailNotificationSettings => ({
    enabled: false,
    smtpHost: '',
    smtpPort: 465,
    security: 'tls',
    sender: '',
    username: '',
    recipients: [],
    onDegraded: true,
    onRecovered: true,
    onFailure: true,
    hasPassword: false,
});
const form = reactive(defaults());
const recipientsText = ref('');
const password = ref('');
const removePassword = ref(false);
const working = ref<'save' | 'test' | null>(null);
const message = ref('');
const localError = ref('');
const baseline = ref('');
const serialize = () =>
    JSON.stringify({
        ...form,
        recipientsText: recipientsText.value,
        password: password.value,
        removePassword: removePassword.value,
    });
const dirty = computed(() => Boolean(baseline.value) && serialize() !== baseline.value);
watch(dirty, (value) => emit('dirty', value));
/** 后台刷新不能覆盖未保存密码或邮件设置；保存成功后才清空敏感草稿。 */
function discard(): void {
    Object.assign(form, defaults(), props.settings ?? {});
    recipientsText.value = form.recipients.join('; ');
    password.value = '';
    removePassword.value = false;
    baseline.value = serialize();
    localError.value = '';
    message.value = '';
}
watch(
    () => props.settings,
    () => {
        if (!dirty.value) discard();
    },
    { immediate: true, deep: true },
);
watch(serialize, () => {
    message.value = '';
});

/** 试发采用当前完整草稿；后端负责最终校验和 TLS，不因保存开关而自动发信。 */
function input(): EmailSettingsInput {
    const recipients = recipientsText.value
        .split(/[;,\n]/u)
        .map((value) => value.trim())
        .filter(Boolean);
    return {
        settings: { ...form, recipients },
        password: password.value,
        removePassword: removePassword.value,
    };
}
async function perform(kind: 'save' | 'test'): Promise<boolean> {
    if (working.value || props.disabled) return false;
    working.value = kind;
    localError.value = '';
    message.value = '';
    const submitted = serialize();
    try {
        await call(kind === 'save' ? 'save_email_settings' : 'send_test_email', { input: input() });
        if (kind === 'save') {
            // 后端已提交便清除明文；快照同步仍属于本次保存，不能提前允许离页后回填旧设置。
            form.recipients = input().settings.recipients;
            form.hasPassword = !removePassword.value && (Boolean(password.value) || form.hasPassword);
            password.value = '';
            removePassword.value = false;
            baseline.value = serialize();
            if (!(await props.afterSave())) return false;
            message.value = '邮件设置已保存。';
        } else if (submitted === serialize()) message.value = '测试邮件已发送，请检查收件箱或垃圾邮件。';
        return true;
    } catch (cause) {
        localError.value = String(cause);
        return false;
    } finally {
        working.value = null;
    }
}
const save = () => perform('save');
const isWorking = computed(() => Boolean(working.value));
defineExpose({ save, discard, dirty, isWorking });
</script>

<template>
    <form id="mail-settings" class="settings-form mail-settings" @submit.prevent="save">
        <div class="notice">
            应用需要保持运行并联网。相同模型和测试条件的持续同类异常只通知一次；手动取消不发信，发送失败不影响测试结果。
        </div>
        <div v-if="notificationError" class="inline-error" role="alert">
            最近通知发送失败：{{ notificationError }}。模型测试结果不受影响。
        </div>
        <div v-if="localError" class="inline-error" role="alert">{{ localError }}</div>
        <div v-if="message" class="success-message" role="status">{{ message }}</div>
        <fieldset :disabled="disabled || Boolean(working)">
            <div class="preference-row">
                <div>
                    <label for="mail-enabled">启用邮箱通知</label>
                    <p>事件通知开关；保存后生效。</p>
                </div>
                <input
                    id="mail-enabled"
                    v-model="form.enabled"
                    type="checkbox"
                    class="switch"
                    role="switch"
                />
            </div>
            <div class="field-row">
                <label for="smtp-host">SMTP 服务</label>
                <div class="field">
                    <input
                        id="smtp-host"
                        v-model="form.smtpHost"
                        maxlength="253"
                        placeholder="smtp.example.com"
                        autocomplete="off"
                    />
                    <p>只填域名或 IP，不带协议和路径。</p>
                </div>
            </div>
            <div class="field-row">
                <label>加密模式</label>
                <div class="field">
                    <ModelPicker
                        label="SMTP 加密模式"
                        context="settings"
                        :profiles="[
                            { id: 'tls', name: 'TLS（通常 465）', model: '' },
                            { id: 'starttls', name: 'STARTTLS（通常 587）', model: '' },
                        ]"
                        :selected-id="form.security"
                        :expanded="false"
                        :disabled="disabled || Boolean(working)"
                        :show-model="false"
                        @select="form.security = $event as 'tls' | 'starttls'"
                    />
                    <p>必须使用加密连接，不支持明文或跳过证书验证。</p>
                </div>
            </div>
            <div class="field-row">
                <label for="smtp-port">端口</label>
                <div class="field">
                    <input
                        id="smtp-port"
                        v-model.number="form.smtpPort"
                        type="number"
                        min="1"
                        max="65535"
                        required
                    />
                </div>
            </div>
            <div class="field-row">
                <label for="mail-sender">发件邮箱</label>
                <div class="field">
                    <input
                        id="mail-sender"
                        v-model="form.sender"
                        type="email"
                        maxlength="254"
                        placeholder="radar@example.com"
                    />
                </div>
            </div>
            <div class="field-row">
                <label for="smtp-username">发件账号</label>
                <div class="field">
                    <input id="smtp-username" v-model="form.username" maxlength="254" autocomplete="off" />
                    <p>一般与发件邮箱相同，以邮件服务商要求为准。</p>
                </div>
            </div>
            <div class="field-row">
                <label for="smtp-password">授权码</label>
                <div class="field">
                    <input
                        id="smtp-password"
                        v-model="password"
                        type="password"
                        maxlength="2500"
                        autocomplete="new-password"
                        :placeholder="form.hasPassword ? '已安全保存，留空保持不变' : '输入 SMTP 授权码'"
                    />
                    <p>保存到 Windows 凭据管理器；更换 SMTP 服务或账号必须重新填写。</p>
                    <label v-if="form.hasPassword" class="toggle-line"
                        ><input
                            v-model="removePassword"
                            type="checkbox"
                            class="switch"
                            role="switch"
                        />移除已保存授权码</label
                    >
                </div>
            </div>
            <div class="field-row">
                <label for="mail-recipients">收件邮箱</label>
                <div class="field">
                    <textarea
                        id="mail-recipients"
                        v-model="recipientsText"
                        rows="3"
                        maxlength="3000"
                        placeholder="最多 10 个，用分号、逗号或换行分隔"
                    />
                </div>
            </div>
            <div class="field-row">
                <label>通知事件</label>
                <div class="field">
                    <label class="toggle-line"
                        ><input
                            v-model="form.onDegraded"
                            type="checkbox"
                            class="switch"
                            role="switch"
                        />变为降智</label
                    ><label class="toggle-line"
                        ><input
                            v-model="form.onRecovered"
                            type="checkbox"
                            class="switch"
                            role="switch"
                        />恢复不降智</label
                    ><label class="toggle-line"
                        ><input
                            v-model="form.onFailure"
                            type="checkbox"
                            class="switch"
                            role="switch"
                        />测试失败</label
                    >
                    <p>邮件包含当次模型、规则、请求信息、结果及完整回答；请求失败时附错误详情。</p>
                </div>
            </div>
        </fieldset>
        <button
            type="button"
            class="button secondary"
            :disabled="disabled || Boolean(working)"
            @click="perform('test')"
        >
            {{ working === 'test' ? '正在发送…' : '发送测试邮件' }}
        </button>
        <p class="field-help">
            试发使用当前草稿，不保存；即使未启用事件通知也可试发。{{ working === 'save' ? '正在保存…' : '' }}
        </p>
    </form>
</template>
