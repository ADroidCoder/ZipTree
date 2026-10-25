//! macOS 专有：设为压缩包的默认打开程序

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};

const BUNDLE_ID: &str = "com.ziptree.desktop";
const ROLES_ALL: u32 = 0xFFFF_FFFF;

/// 设为默认程序的格式。macOS 会对每种格式单独弹窗让用户确认，所以只设最常用的三种
const CONTENT_TYPES: [&str; 3] = ["public.zip-archive", "org.7-zip.7-zip-archive", "com.rarlab.rar-archive"];

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSSetDefaultRoleHandlerForContentType(content_type: CFStringRef, role: u32, bundle_id: CFStringRef) -> i32;
    fn LSCopyDefaultRoleHandlerForContentType(content_type: CFStringRef, role: u32) -> CFStringRef;
}

fn handler(content_type: &str) -> Option<String> {
    let t = CFString::new(content_type);
    let r = unsafe { LSCopyDefaultRoleHandlerForContentType(t.as_concrete_TypeRef(), ROLES_ALL) };
    (!r.is_null()).then(|| unsafe { CFString::wrap_under_create_rule(r) }.to_string())
}

/// 常见压缩格式是不是都默认用 ZipTree 打开
pub fn is_default_app() -> bool {
    CONTENT_TYPES.iter().all(|t| handler(t).is_some_and(|h| h.eq_ignore_ascii_case(BUNDLE_ID)))
}

/// 设为默认打开程序
pub fn set_default_app() -> Result<(), i32> {
    let id = CFString::new(BUNDLE_ID);
    for t in CONTENT_TYPES {
        let t = CFString::new(t);
        let status = unsafe { LSSetDefaultRoleHandlerForContentType(t.as_concrete_TypeRef(), ROLES_ALL, id.as_concrete_TypeRef()) };
        if status != 0 {
            return Err(status);
        }
    }
    Ok(())
}

// ---------- 快速查看 ----------
// 用系统的快速查看面板（和访达里按空格一样）显示一个文件

use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_foundation::{NSInteger, NSString, NSURL};
use objc2_quick_look_ui::{QLPreviewItem, QLPreviewPanel, QLPreviewPanelDataSource};
use std::cell::RefCell;
use std::path::Path;

struct SourceIvars {
    url: Retained<NSURL>,
}

define_class!(
    /// 告诉面板要显示哪个文件
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SourceIvars]
    struct PreviewSource;

    unsafe impl NSObjectProtocol for PreviewSource {}

    unsafe impl QLPreviewPanelDataSource for PreviewSource {
        #[unsafe(method(numberOfPreviewItemsInPreviewPanel:))]
        fn number_of_items(&self, _panel: Option<&QLPreviewPanel>) -> NSInteger {
            1
        }

        #[unsafe(method_id(previewPanel:previewItemAtIndex:))]
        fn item(&self, _panel: Option<&QLPreviewPanel>, _index: NSInteger) -> Option<Retained<ProtocolObject<dyn QLPreviewItem>>> {
            Some(ProtocolObject::from_retained(self.ivars().url.clone()))
        }
    }
);

impl PreviewSource {
    fn new(mtm: MainThreadMarker, url: Retained<NSURL>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(SourceIvars { url });
        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    /// 面板不持有数据源，要自己留着
    static SOURCE: RefCell<Option<Retained<PreviewSource>>> = const { RefCell::new(None) };
}

/// 显示快速查看面板。只能在主线程调用
pub fn quick_look(path: &Path) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let source = PreviewSource::new(mtm, url);
    unsafe {
        let Some(panel) = QLPreviewPanel::sharedPreviewPanel(mtm) else { return };
        panel.setDataSource(Some(ProtocolObject::from_ref(&*source)));
        panel.reloadData();
        panel.makeKeyAndOrderFront(None);
    }
    SOURCE.with(|s| *s.borrow_mut() = Some(source));
}

/// 关掉快速查看面板，返回之前是不是开着。只能在主线程调用
pub fn close_quick_look() -> bool {
    let Some(mtm) = MainThreadMarker::new() else { return false };
    unsafe {
        if !QLPreviewPanel::sharedPreviewPanelExists(mtm) {
            return false;
        }
        let Some(panel) = QLPreviewPanel::sharedPreviewPanel(mtm) else { return false };
        let open = panel.isVisible();
        panel.orderOut(None);
        open
    }
}
