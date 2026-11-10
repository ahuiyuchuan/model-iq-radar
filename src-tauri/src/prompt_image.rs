//! 将原始题目排版为白底黑字 PNG，仅返回内存中的 data URL，不解释 Markdown/HTML。
//! 使用 Windows 自带 DirectWrite 字体回退、Direct2D/WIC；后台自动测试不依赖 WebView。
//! 每页按完整文字行分组，逐页释放位图；不附加答案、文件名或题目明文，不截断长题。
//! @author ahui

use tokio_util::sync::CancellationToken;

/// 图片布局与请求 detail 的语义版本，参与条件摘要；未来改变排版时必须更新。
pub const VERSION: &str = "prompt-png-v1";

/// 32px 正文字号、44px 行高，1440px 页宽留出左右各32px；不是题目总长度限制。
#[cfg(windows)]
const WIDTH: u32 = 1440;
#[cfg(windows)]
const PADDING: f32 = 32.0;
#[cfg(windows)]
const LINE_HEIGHT: f32 = 44.0;
/// 分页限制单张解码内存，不缩小字体、不丢弃页面或切断同一行。
#[cfg(windows)]
const LINES_PER_PAGE: usize = 44;

/// 阻塞渲染器的取消句柄；异步等待被超时/取消/卸载丢弃时，也停止后续页面生成。
pub struct Rendering {
    pub task: tokio::task::JoinHandle<Result<Vec<String>, String>>,
    stop: CancellationToken,
}

impl Drop for Rendering {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

/// 在阻塞池排版；仅传入题目，COM/位图始终在同一工作线程创建、使用和释放。
/// 返回的任务拒绝会由请求层转换为本地失败，不能触发协议回退或明文重发。
pub fn start(prompt: String) -> Rendering {
    let stop = CancellationToken::new();
    let worker_stop = stop.clone();
    Rendering {
        task: tokio::task::spawn_blocking(move || render(&prompt, &worker_stop)),
        stop,
    }
}

#[cfg(not(windows))]
fn render(_: &str, _: &CancellationToken) -> Result<Vec<String>, String> {
    Err("题目转图片目前仅支持 Windows 桌面版，请关闭转图片发送后重试。".into())
}

#[cfg(windows)]
fn render(prompt: &str, stop: &CancellationToken) -> Result<Vec<String>, String> {
    if stop.is_cancelled() {
        return Err("题目转图片已取消。".into());
    }
    // 保留可见字符、段落及空白；控制字符不具可绘制形式，明确拒绝而非静默丢弃。
    if prompt
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
    {
        return Err("图片题目包含无法显示的控制字符，请移除后重试。".into());
    }
    native::render(prompt, stop)
        .map_err(|_| "无法将题目转换为图片，请检查系统字体或稍后重试；也可关闭转图片发送。".into())
}

#[cfg(windows)]
mod native {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use windows::{
        core::{w, Error, Result},
        Win32::{
            Foundation::{E_ABORT, E_FAIL, HGLOBAL, RPC_E_CHANGED_MODE},
            Graphics::{
                Direct2D::{
                    Common::D2D1_COLOR_F, D2D1CreateFactory, ID2D1Factory,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                    D2D1_RENDER_TARGET_PROPERTIES, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                },
                DirectWrite::{
                    DWriteCreateFactory, IDWriteFactory, IDWriteTextLayout,
                    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_LINE_METRICS,
                    DWRITE_LINE_SPACING_METHOD_UNIFORM, DWRITE_TEXT_METRICS,
                    DWRITE_WORD_WRAPPING_WRAP,
                },
                Imaging::{
                    CLSID_WICImagingFactory, GUID_ContainerFormatPng,
                    GUID_WICPixelFormat32bppPBGRA, IWICBitmapFrameEncode, IWICImagingFactory,
                    WICBitmapCacheOnLoad, WICBitmapEncoderNoCache,
                },
            },
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize,
                StructuredStorage::{CreateStreamOnHGlobal, IPropertyBag2},
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, STATFLAG_NONAME, STATSTG,
                STREAM_SEEK_SET,
            },
        },
    };
    use windows_numerics::Vector2;

    /// 成功初始化（含 S_FALSE）与释放一一对应；借用已有 STA 时不反向释放其 COM。
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: 仅在当前线程初始化成功后执行，所有本地 COM 变量先于此守卫释放。
                unsafe { CoUninitialize() };
            }
        }
    }

    /// 完整测量原文并按行分页；所有 unsafe 均为当前线程有效 COM 对象/有界切片调用。
    pub(super) fn render(prompt: &str, stop: &CancellationToken) -> Result<Vec<String>> {
        // SAFETY: 不跨线程共享接口；WIC 流自主管理内存，失败路径由 RAII 释放全部对象。
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_MULTITHREADED);
            let _apartment = if initialized == RPC_E_CHANGED_MODE {
                Apartment(false)
            } else {
                initialized.ok()?;
                Apartment(true)
            };
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let draw: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let imaging: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let format = write.CreateTextFormat(
                w!("Microsoft YaHei"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                32.0,
                w!("zh-CN"),
            )?;
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
            format.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, LINE_HEIGHT, 34.0)?;
            let text: Vec<u16> = prompt.encode_utf16().collect();
            let measure = write.CreateTextLayout(
                &text,
                &format,
                WIDTH as f32 - PADDING * 2.0,
                (text.len() + 1) as f32 * LINE_HEIGHT,
            )?;
            let mut count = 0;
            // 第一次返回缓冲不足是 GetLineMetrics 的长度查询约定；实际读取必须成功。
            let _ = measure.GetLineMetrics(None, &mut count);
            if count == 0 {
                return Err(Error::from_hresult(E_FAIL));
            }
            let mut lines = vec![DWRITE_LINE_METRICS::default(); count as usize];
            measure.GetLineMetrics(Some(&mut lines), &mut count)?;
            if lines.iter().map(|line| line.length as usize).sum::<usize>() != text.len() {
                return Err(Error::from_hresult(E_FAIL));
            }
            let mut pages = Vec::new();
            let mut offset = 0;
            for group in lines.chunks(LINES_PER_PAGE) {
                if stop.is_cancelled() {
                    return Err(Error::from_hresult(E_ABORT));
                }
                let length: usize = group.iter().map(|line| line.length as usize).sum();
                let layout = write.CreateTextLayout(
                    &text[offset..offset + length],
                    &format,
                    WIDTH as f32 - PADDING * 2.0,
                    (LINES_PER_PAGE + 2) as f32 * LINE_HEIGHT,
                )?;
                pages.push(encode_page(&draw, &imaging, &layout)?);
                offset += length;
            }
            Ok(pages)
        }
    }

    /// 每页独立白底 PNG，中文/长单词自动换行；字形抗锯齿与系统字体回退由 DirectWrite 执行。
    unsafe fn encode_page(
        draw: &ID2D1Factory,
        imaging: &IWICImagingFactory,
        layout: &IDWriteTextLayout,
    ) -> Result<String> {
        // SAFETY: 调用方持有同一线程内的有效工厂和布局，所有输出长度从已提交的流查询。
        unsafe {
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            let height = (metrics.height.ceil() + PADDING * 2.0) as u32;
            let bitmap = imaging.CreateBitmap(
                WIDTH,
                height,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapCacheOnLoad,
            )?;
            let target = draw
                .CreateWicBitmapRenderTarget(&bitmap, &D2D1_RENDER_TARGET_PROPERTIES::default())?;
            let black = target.CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                None,
            )?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.BeginDraw();
            target.Clear(Some(&D2D1_COLOR_F {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }));
            target.DrawTextLayout(
                Vector2 {
                    X: PADDING,
                    Y: PADDING,
                },
                layout,
                &black,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
            );
            target.EndDraw(None, None)?;
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
            let encoder = imaging.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame: Option<IWICBitmapFrameEncode> = None;
            let mut options: Option<IPropertyBag2> = None;
            encoder.CreateNewFrame(&mut frame, &mut options)?;
            let frame = frame.ok_or_else(|| Error::from_hresult(E_FAIL))?;
            frame.Initialize(options.as_ref())?;
            frame.SetSize(WIDTH, height)?;
            let mut format = GUID_WICPixelFormat32bppPBGRA;
            frame.SetPixelFormat(&mut format)?;
            frame.WriteSource(&bitmap, std::ptr::null())?;
            frame.Commit()?;
            encoder.Commit()?;
            let mut stat = STATSTG::default();
            stream.Stat(&mut stat, STATFLAG_NONAME)?;
            let length = u32::try_from(stat.cbSize).map_err(|_| Error::from_hresult(E_FAIL))?;
            let mut png = vec![0_u8; length as usize];
            stream.Seek(0, STREAM_SEEK_SET, None)?;
            let mut read = 0;
            stream
                .Read(png.as_mut_ptr().cast(), length, Some(&mut read))
                .ok()?;
            if read != length {
                return Err(Error::from_hresult(E_FAIL));
            }
            Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    /// 真正调用无窗口排版/编码；全为虚构题目，既不读取桌面，也不调用模型或用户数据。
    #[test]
    fn chinese_unicode_and_long_prompt_produce_complete_png_pages() {
        let prompt = format!(
            "中文图片题目：三颗糖加两颗糖是多少？\n{}\n末尾校验：𠮷 😀 <svg> & 不执行脚本",
            "长单词abcDEF123 ".repeat(350)
        );
        let pages = render(&prompt, &CancellationToken::new()).unwrap();
        assert!(pages.len() > 1);
        for (index, page) in pages.iter().enumerate() {
            let png = STANDARD
                .decode(page.strip_prefix("data:image/png;base64,").unwrap())
                .unwrap();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), WIDTH);
            assert!(u32::from_be_bytes(png[20..24].try_into().unwrap()) <= 2048);
            // 可选导出仅用于代码层图像检查，目录由测试命令指定，不进入生产路径。
            if let Ok(directory) = std::env::var("MODEL_PROBE_IMAGE_QA_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    std::path::Path::new(&directory).join(format!("page-{index}.png")),
                    png,
                )
                .unwrap();
            }
        }
    }

    /// 沿用现有20000字符边界，连续长文本必须分页，不缩小字体或因画布高度裁剪。
    #[test]
    fn full_rule_length_uses_multiple_pages_without_downscaling() {
        let pages = render(&"题".repeat(20_000), &CancellationToken::new()).unwrap();
        assert!(pages.len() > 8);
        for page in pages {
            let png = STANDARD
                .decode(page.strip_prefix("data:image/png;base64,").unwrap())
                .unwrap();
            assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), WIDTH);
            assert!(u32::from_be_bytes(png[20..24].try_into().unwrap()) <= 2048);
        }
    }

    #[test]
    fn cancelled_and_unrenderable_input_never_become_text_fallback() {
        let stop = CancellationToken::new();
        stop.cancel();
        assert!(render("虚构题目", &stop).is_err());
        assert!(render("题目\0隐形内容", &CancellationToken::new()).is_err());
        assert_eq!(
            render("题目\n", &CancellationToken::new()).unwrap().len(),
            1
        );
    }
}
