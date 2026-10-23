/** 不可信 Markdown 回归：常用语法、离线资源、恶意协议与失败隔离。@author ahui */
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js/lib/common';
import MarkdownAnswer from '../src/components/MarkdownAnswer.vue';
import { externalUrl } from '../src/markdown';

const fake = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock('../src/bridge', () => ({ call: fake.call }));
beforeEach(() => fake.call.mockReset().mockResolvedValue(undefined));
afterEach(() => vi.restoreAllMocks());

it('渲染 CommonMark 与 GFM 常用语法，任务只读，代码使用本地高亮', () => {
    const wrapper = mount(MarkdownAnswer, {
        props: {
            source: '# 标题\n\n**加粗** *斜体* ~~删除~~ `行内代码`\n\n> 引用\n\n1. 有序\n2. 第二项\n\n- 无序\n- [x] 完成\n- [ ] 未完成\n\n---\n\n[链接](https://example.com)\n\n| 列1 | 列2 |\n| --- | ---: |\n| A | B |\n\n```javascript\nconst value = 42;\n```',
        },
    });
    for (const selector of [
        'h1',
        'strong',
        'em',
        's',
        'blockquote',
        'ol',
        'ul',
        'hr',
        'a',
        'table',
        'pre code .hljs-keyword',
    ]) {
        expect(wrapper.find(selector).exists(), selector).toBe(true);
    }
    expect(wrapper.findAll('input[type="checkbox"]')).toHaveLength(2);
    expect(wrapper.get('input[checked]').attributes('disabled')).toBeDefined();
    expect(wrapper.get('pre code').element.textContent).toBe('const value = 42;\n');
    expect(fake.call).not.toHaveBeenCalled();
    wrapper.unmount();
});

it('HTML、脚本、SVG、图片及危险/相对协议均不产生可执行 DOM 或资源加载', () => {
    const source =
        '<script>alert(1)</script>\n<img src="https://tracking.example/a" onerror="alert(1)">\n<svg onload="alert(1)"></svg>\n\n![远程图片](https://tracking.example/remote.png)\n\n[脚本](javascript:alert%281%29) [实体](javascript&#58;alert%281%29) [文件](file:///C:/a.exe) [数据](data:text/html,x) [相对](//tracking.example) [本地](./a)';
    const wrapper = mount(MarkdownAnswer, { props: { source } });
    expect(wrapper.find('script,img,svg,iframe,object,style,form,video,audio').exists()).toBe(false);
    expect(wrapper.find('[src],[srcset],[onerror],[onload]').exists()).toBe(false);
    expect(
        wrapper.findAll('a').every((a) => a.attributes('href')?.startsWith('https://tracking.example/')),
    ).toBe(true);
    expect(wrapper.text()).toContain('<script>');
    expect(wrapper.text()).toContain('远程图片');
    expect(wrapper.text()).toContain('未加载');
    for (const url of [
        'javascript:alert(1)',
        'data:text/html,x',
        'file:///C:/a',
        'https://x:y@example.com',
        'https://example.com\n/a',
    ])
        expect(externalUrl(url)).toBeNull();
    expect(fake.call).not.toHaveBeenCalled();
    wrapper.unmount();
});

it('长表格/代码保留完整文本与滚动包装，未知语言不执行也不下载', () => {
    const columns = Array.from({ length: 15 }, (_, i) => `列${i}`);
    const line = 'x'.repeat(5000);
    const source = `| ${columns.join(' | ')} |\n| ${columns.map(() => '---').join(' | ')} |\n| ${columns.map(() => '数据').join(' | ')} |\n\n\`\`\`not-a-language\n${line}\n\`\`\``;
    const wrapper = mount(MarkdownAnswer, { props: { source } });
    expect(wrapper.findAll('.markdown-table-scroll th')).toHaveLength(15);
    expect(wrapper.get('pre code').element.textContent).toBe(`${line}\n`);
    expect(fake.call).not.toHaveBeenCalled();
    wrapper.unmount();
});

it('链接仅主动激活时调用系统浏览器，错误局限在回答组件', async () => {
    const wrapper = mount(MarkdownAnswer, { props: { source: '[网页](https://example.com/path?q=1)' } });
    expect(fake.call).not.toHaveBeenCalled();
    await wrapper.get('a').trigger('click');
    await flushPromises();
    expect(fake.call).toHaveBeenCalledWith('open_external_url', { url: 'https://example.com/path?q=1' });
    fake.call.mockRejectedValue(new Error('failed'));
    await wrapper.get('a').trigger('click');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('无法打开');
    expect(wrapper.get('a').text()).toBe('网页');
    wrapper.unmount();
});

it('高亮/净化失败及超长输入回退完整纯文本，不抛错或丢失原文', () => {
    const source = '**原始 Markdown**\n```javascript\nconst n = 1;\n```';
    const highlight = vi.spyOn(hljs, 'highlight').mockImplementation(() => {
        throw new Error('highlight failed');
    });
    let wrapper = mount(MarkdownAnswer, { props: { source } });
    expect(wrapper.get('.markdown-fallback').text()).toBe(source);
    wrapper.unmount();
    highlight.mockRestore();
    vi.spyOn(DOMPurify, 'sanitize').mockImplementation(() => {
        throw new Error('parse failed');
    });
    wrapper = mount(MarkdownAnswer, { props: { source } });
    expect(wrapper.get('.markdown-fallback').text()).toBe(source);
    wrapper.unmount();
    vi.restoreAllMocks();
    const long = `**${'长'.repeat(200001)}**`;
    wrapper = mount(MarkdownAnswer, { props: { source: long } });
    expect(wrapper.get('.markdown-fallback').text()).toBe(long);
    wrapper.unmount();
});
