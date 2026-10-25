/**
 * 历史导出与日期过滤共用纯函数；原始回答在 JSON 中保持不变，CSV 防止表格公式执行。
 * @author ahui
 */
import type { TestRecord } from './types';
export const statusText: Record<string, string> = {
    passed: '不降智',
    failed: '降智',
    error: '测试失败',
    cancelled: '已取消',
};

/**
 * 只有完整条件摘要证明新请求明确保存设置时，null 才表示“不指定”；旧记录不猜补。
 * @param record 未修改的历史快照；旧记录可缺少强度和条件摘要字段。
 * @returns 保存的强度、“不指定”或“未记录”，不重新推断模型能力。
 */
export function reasoningLabel(record: TestRecord): string {
    return record.reasoningEffort ?? (record.conditionFingerprint ? '不指定' : '未记录');
}

/**
 * 将真实时间戳转成本机日历日，避免 UTC 午夜偏移造成日期筛选丢记录。
 * @param value 带时区的历史时间戳，或 Date 对象。
 * @returns 本机 YYYY-MM-DD；无效输入返回空串，日期过滤时应排除。
 */
export function localDate(value: string | Date): string {
    const date = value instanceof Date ? value : new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/**
 * CSV 面向电子表格导入；危险前缀添加单引号，防止不可信回答或模型名触发公式。
 * 原文无损留档请使用 JSON，复制回答也始终复制原始 Markdown。
 * @param records 当前筛选结果或完整历史，传入空数组时仅返回表头。
 * @returns 含 UTF-8 BOM、CRLF 行分隔和完整引号转义的 CSV 文本，不修改原记录。
 */
export function historyCsv(records: TestRecord[]): string {
    const columns = [
        'id',
        'profileId',
        'profileName',
        'model',
        'status',
        'source',
        'interfaceMode',
        'protocolFallbackEnabled',
        'attemptedModes',
        'reasoningEffort',
        'effectiveTimeoutSeconds',
        'startedAt',
        'durationMs',
        'prompt',
        'keyword',
        'alternativeKeywords',
        'ruleName',
        'answer',
        'error',
        'errorKind',
        'httpStatus',
        'requestId',
        'ruleVersion',
    ] as const;
    const quote = (value: unknown): string => {
        let text = String(value ?? '');
        if (/^[\s\uFEFF]*[=+@-]/u.test(text) || /^[\t\r\n]/u.test(text)) text = `'${text}`;
        return `"${text.replace(/"/g, '""')}"`;
    };
    return (
        '\ufeff' +
        [
            columns.join(','),
            ...records.map((record) =>
                columns
                    .map((key) =>
                        quote(
                            key === 'alternativeKeywords' || key === 'attemptedModes'
                                ? JSON.stringify(record[key] ?? [])
                                : record[key],
                        ),
                    )
                    .join(','),
            ),
        ].join('\r\n')
    );
}

/**
 * 延迟释放 Blob URL，允许 WebView 完成下载接管；无网络请求或第三方存储。
 * @param filename 用户可见文件名，调用方仅使用固定应用名称和本机日期生成。
 * @param text 已按导出契约生成的完整文本。
 * @param type 包含字符集的 MIME 类型。
 */
export function downloadText(filename: string, text: string, type: string): void {
    const url = URL.createObjectURL(new Blob([text], { type }));
    // 捕获创建时的实现，延迟清理不依赖页面或测试环境随后替换的全局 URL 对象。
    const revoke = URL.revokeObjectURL.bind(URL);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = filename;
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    window.setTimeout(() => revoke(url), 1000);
}
