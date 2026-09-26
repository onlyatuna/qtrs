use std::collections::HashMap;
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ffi::{c_char, CStr, CString};
use std::sync::{LazyLock, Mutex};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(pub *mut c_void);

unsafe impl Send for Id {}
unsafe impl Sync for Id {}

impl Id {
    pub const NIL: Self = Self(std::ptr::null_mut());

    #[inline]
    pub fn is_nil(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

/// Objective-C class pointer (`Class`)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Class(pub *mut c_void);

unsafe impl Send for Class {}
unsafe impl Sync for Class {}

impl Class {
    pub const NIL: Self = Self(std::ptr::null_mut());

    #[inline]
    pub fn is_nil(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn get(name: &str) -> Option<Self> {
        let cls = objc_get_class(name);
        if cls.is_nil() {
            None
        } else {
            Some(cls)
        }
    }
}

/// Objective-C selector pointer (`Sel`)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sel(pub *const c_void);

unsafe impl Send for Sel {}
unsafe impl Sync for Sel {}

impl Sel {
    pub const NIL: Self = Self(std::ptr::null());

    #[inline]
    pub fn is_nil(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn register(name: &str) -> Self {
        sel_register_name(name)
    }
}

/// Objective-C boolean type (`BOOL`: YES = 1, NO = 0)
pub type BOOL = i8;
pub const YES: BOOL = 1;
pub const NO: BOOL = 0;

pub type CGFloat = f64;
pub type NSInteger = isize;
pub type NSUInteger = usize;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

impl CGPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: CGFloat, y: CGFloat) -> Self {
        Self { x, y }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

impl CGSize {
    pub const ZERO: Self = Self { width: 0.0, height: 0.0 };
    pub fn new(width: CGFloat, height: CGFloat) -> Self {
        Self { width, height }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub const ZERO: Self = Self {
        origin: CGPoint::ZERO,
        size: CGSize::ZERO,
    };
    pub fn new(x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) -> Self {
        Self {
            origin: CGPoint::new(x, y),
            size: CGSize::new(width, height),
        }
    }
}

// ---------------------------------------------------------------------------
// AppKit constant definitions (aligned with macOS SDK)
// ---------------------------------------------------------------------------

pub const NS_VARIABLE_STATUS_ITEM_LENGTH: CGFloat = -1.0;
pub const NS_SQUARE_STATUS_ITEM_LENGTH: CGFloat = -2.0;

pub const NS_CONTROL_STATE_VALUE_OFF: NSInteger = 0;
pub const NS_CONTROL_STATE_VALUE_ON: NSInteger = 1;
pub const NS_CONTROL_STATE_VALUE_MIXED: NSInteger = -1;

pub const NS_WINDOW_STYLE_MASK_BORDERLESS: NSUInteger = 0;
pub const NS_WINDOW_STYLE_MASK_TITLED: NSUInteger = 1 << 0;
pub const NS_WINDOW_STYLE_MASK_CLOSABLE: NSUInteger = 1 << 1;
pub const NS_WINDOW_STYLE_MASK_MINIATURIZABLE: NSUInteger = 1 << 2;
pub const NS_WINDOW_STYLE_MASK_RESIZABLE: NSUInteger = 1 << 3;
pub const NS_WINDOW_STYLE_MASK_FULL_SIZE_CONTENT_VIEW: NSUInteger = 1 << 15;

pub const NS_BACKING_STORE_BUFFERED: NSUInteger = 2;

pub const NS_FLOATING_WINDOW_LEVEL: NSInteger = 3; // kCGFloatingWindowLevelKey
pub const NS_STATUS_WINDOW_LEVEL: NSInteger = 25; // System status bar window level

// ---------------------------------------------------------------------------
// Runtime and message dispatch (native vs mock)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod native_bindings {
    use super::*;

    #[link(name = "objc")]
    #[link(name = "AppKit", kind = "framework")]
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn objc_getClass(name: *const c_char) -> *mut c_void;
        pub fn sel_registerName(name: *const c_char) -> *const c_void;
        pub fn sel_getName(sel: *const c_void) -> *const c_char;
        pub fn objc_msgSend();
    }
}

/// Creates an NSString object from a UTF-8 string on macOS
#[cfg(target_os = "macos")]
pub fn nsstring_from_str(s: &str) -> Id {
    let cls = objc_get_class("NSString");
    let sel_alloc = Sel::register("alloc");
    let sel_init = Sel::register("initWithBytes:length:encoding:");
    let str_obj = ObjcMsg::send_class_0(cls, sel_alloc);
    unsafe {
        let msg_send: extern "C" fn(*mut c_void, *const c_void, *const u8, NSUInteger, NSUInteger) -> *mut c_void =
            std::mem::transmute(native_bindings::objc_msgSend as *const ());
        Id(msg_send(str_obj.0, sel_init.0, s.as_ptr(), s.len(), 4 /* NSUTF8StringEncoding */))
    }
}

/// Retrieves class pointer
pub fn objc_get_class(name: &str) -> Class {
    #[cfg(target_os = "macos")]
    {
        let c_str = CString::new(name).unwrap();
        Class(unsafe { native_bindings::objc_getClass(c_str.as_ptr()) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        MockObjcRuntime::instance().get_class(name)
    }
}

/// Registers or retrieves selector
pub fn sel_register_name(name: &str) -> Sel {
    #[cfg(target_os = "macos")]
    {
        let c_str = CString::new(name).unwrap();
        Sel(unsafe { native_bindings::sel_registerName(c_str.as_ptr()) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        MockObjcRuntime::instance().register_sel(name)
    }
}

// ---------------------------------------------------------------------------
// High-fidelity mock Objective-C runtime for cross-platform simulation (MockObjcRuntime)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MockObjectData {
    pub class_name: String,
    pub title: String,
    pub frame: CGRect,
    pub level: NSInteger,
    pub ignores_mouse_events: bool,
    pub is_flipped: bool,
    pub is_visible: bool,
    pub state: NSInteger,
    pub enabled: bool,
    pub children: Vec<Id>,
    pub parent: Id,
    pub menu: Id,
    pub button: Id,
    pub contents: Id,
    pub opacity: f64,
}

impl Default for MockObjectData {
    fn default() -> Self {
        Self {
            class_name: String::new(),
            title: String::new(),
            frame: CGRect::ZERO,
            level: 0,
            ignores_mouse_events: false,
            is_flipped: false,
            is_visible: false,
            state: NS_CONTROL_STATE_VALUE_OFF,
            enabled: true,
            children: Vec::new(),
            parent: Id::NIL,
            menu: Id::NIL,
            button: Id::NIL,
            contents: Id::NIL,
            opacity: 1.0,
        }
    }
}

pub struct MockObjcRuntime {
    classes: Mutex<HashMap<String, usize>>,
    selectors: Mutex<HashMap<String, usize>>,
    objects: Mutex<HashMap<usize, MockObjectData>>,
    next_id: Mutex<usize>,
}

impl MockObjcRuntime {
    pub fn new() -> Self {
        let mut classes = HashMap::new();
        let class_names = [
            "NSApplication",
            "NSStatusBar",
            "NSStatusItem",
            "NSStatusBarButton",
            "NSMenu",
            "NSMenuItem",
            "NSWindow",
            "NSView",
            "QNSView",
            "NSEvent",
            "CALayer",
        ];
        for (i, name) in class_names.iter().enumerate() {
            classes.insert(name.to_string(), i + 1);
        }

        Self {
            classes: Mutex::new(classes),
            selectors: Mutex::new(HashMap::new()),
            objects: Mutex::new(HashMap::new()),
            next_id: Mutex::new(100),
        }
    }

    pub fn instance() -> &'static Self {
        static INSTANCE: LazyLock<MockObjcRuntime> = LazyLock::new(MockObjcRuntime::new);
        &INSTANCE
    }

    pub fn get_class(&self, name: &str) -> Class {
        let mut lock = self.classes.lock().unwrap();
        let next = lock.len() + 1;
        let id = lock.entry(name.to_string()).or_insert(next);
        Class(*id as *mut c_void)
    }

    pub fn register_sel(&self, name: &str) -> Sel {
        let mut lock = self.selectors.lock().unwrap();
        let next = lock.len() + 1;
        let id = lock.entry(name.to_string()).or_insert(next);
        Sel(*id as *const c_void)
    }

    pub fn allocate_object(&self, class_name: &str) -> Id {
        let mut id_lock = self.next_id.lock().unwrap();
        let id = *id_lock;
        *id_lock += 1;

        let mut obj = MockObjectData::default();
        obj.class_name = class_name.to_string();

        if class_name == "NSStatusItem" {
            // Automatically create NSStatusBarButton for NSStatusItem
            let button_id = *id_lock;
            *id_lock += 1;
            let mut button = MockObjectData::default();
            button.class_name = "NSStatusBarButton".to_string();
            button.parent = Id(id as *mut c_void);
            self.objects.lock().unwrap().insert(button_id, button);
            obj.button = Id(button_id as *mut c_void);
        }

        if class_name == "QNSView" {
            // Align with Qt QNSView (qnsview_drawing.mm:67-70): isFlipped { return YES; }
            obj.is_flipped = true;
        }

        self.objects.lock().unwrap().insert(id, obj);
        Id(id as *mut c_void)
    }

    pub fn get_object_data(&self, id: Id) -> Option<MockObjectData> {
        self.objects.lock().unwrap().get(&(id.0 as usize)).cloned()
    }

    pub fn update_object<F: FnOnce(&mut MockObjectData)>(&self, id: Id, f: F) {
        if let Some(obj) = self.objects.lock().unwrap().get_mut(&(id.0 as usize)) {
            f(obj);
        }
    }

    pub fn remove_object(&self, id: Id) {
        self.objects.lock().unwrap().remove(&(id.0 as usize));
    }
}

// ---------------------------------------------------------------------------
// Message send wrapper (msg_send mock and native dispatch)
// ---------------------------------------------------------------------------

pub struct ObjcMsg;

impl ObjcMsg {
    /// Sends a zero-argument message
    pub fn send_0(receiver: Id, sel: Sel) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let msg_send: extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(receiver.0, sel.0))
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "button" {
                if let Some(data) = runtime.get_object_data(receiver) {
                    return data.button;
                }
            } else if sel_name == "menu" {
                if let Some(data) = runtime.get_object_data(receiver) {
                    return data.menu;
                }
            }
            receiver
        }
    }

    /// Sends a class factory message (e.g., [NSMenu alloc], [NSStatusBar systemStatusBar])
    pub fn send_class_0(class: Class, sel: Sel) -> Id {
        if class.is_nil() {
            return Id::NIL;
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let msg_send: extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(class.0, sel.0))
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let class_name = runtime.classes.lock().unwrap().iter()
                .find(|(_, &v)| v == (class.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "systemStatusBar" {
                runtime.allocate_object("NSStatusBar")
            } else if sel_name == "sharedApplication" {
                runtime.allocate_object("NSApplication")
            } else if sel_name == "alloc" {
                runtime.allocate_object(&class_name)
            } else {
                Id::NIL
            }
        }
    }

    /// Sends a message with 1 string argument (e.g., initWithTitle:, setTitle:)
    pub fn send_str(receiver: Id, _sel: Sel, text: &str) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            runtime.update_object(receiver, |data| {
                data.title = text.to_string();
            });
            receiver
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let ns_title = nsstring_from_str(text);
            let sel_name_ptr = native_bindings::sel_getName(sel.0 as *const c_void);
            let sel_str = if !sel_name_ptr.is_null() {
                CStr::from_ptr(sel_name_ptr).to_str().unwrap_or_default()
            } else {
                ""
            };

            if sel_str == "initWithTitle:action:keyEquivalent:" {
                let empty_key = nsstring_from_str("");
                let msg_send: extern "C" fn(*mut c_void, *const c_void, *mut c_void, *const c_void, *mut c_void) -> *mut c_void =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                Id(msg_send(receiver.0, sel.0, ns_title.0, std::ptr::null(), empty_key.0))
            } else {
                let msg_send: extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> *mut c_void =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                Id(msg_send(receiver.0, sel.0, ns_title.0))
            }
        }
    }

    /// Sends a message with 1 object argument (e.g., setMenu:, addItem:, addSubview:, removeStatusItem:)
    pub fn send_id(receiver: Id, sel: Sel, arg: Id) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "setMenu:" {
                runtime.update_object(receiver, |data| {
                    data.menu = arg;
                });
            } else if sel_name == "setContents:" {
                runtime.update_object(receiver, |data| {
                    data.contents = arg;
                });
            } else if sel_name == "addItem:" || sel_name == "addSubview:" || sel_name == "setContentView:" {
                runtime.update_object(receiver, |data| {
                    data.children.push(arg);
                });
                runtime.update_object(arg, |data| {
                    data.parent = receiver;
                });
            } else if sel_name == "removeStatusItem:" {
                runtime.remove_object(arg);
            }
            receiver
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let msg_send: extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(receiver.0, sel.0, arg.0))
        }
    }

    /// Sends a message with 1 float/length argument (e.g., statusItemWithLength:)
    pub fn send_length(receiver: Id, sel: Sel, length: CGFloat) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "setOpacity:" {
                runtime.update_object(receiver, |data| {
                    data.opacity = length;
                });
                receiver
            } else {
                let item = runtime.allocate_object("NSStatusItem");
                item
            }
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let msg_send: extern "C" fn(*mut c_void, *const c_void, CGFloat) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(receiver.0, sel.0, length))
        }
    }

    /// Sends a message with an integer argument (e.g., setLevel:, setState:)
    pub fn send_int(receiver: Id, sel: Sel, val: NSInteger) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "setLevel:" {
                runtime.update_object(receiver, |data| {
                    data.level = val;
                });
            } else if sel_name == "setState:" {
                runtime.update_object(receiver, |data| {
                    data.state = val;
                });
            }
            receiver
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let msg_send: extern "C" fn(*mut c_void, *const c_void, NSInteger) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(receiver.0, sel.0, val))
        }
    }

    /// Sends a message with a boolean argument (e.g., setIgnoresMouseEvents:, setEnabled:)
    pub fn send_bool(receiver: Id, sel: Sel, val: bool) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "setIgnoresMouseEvents:" {
                runtime.update_object(receiver, |data| {
                    data.ignores_mouse_events = val;
                });
            } else if sel_name == "setEnabled:" {
                runtime.update_object(receiver, |data| {
                    data.enabled = val;
                });
            }
            receiver
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let bool_val: BOOL = if val { YES } else { NO };
            let msg_send: extern "C" fn(*mut c_void, *const c_void, BOOL) -> *mut c_void =
                std::mem::transmute(native_bindings::objc_msgSend as *const ());
            Id(msg_send(receiver.0, sel.0, bool_val))
        }
    }

    /// Sends window initialization message (initWithContentRect:styleMask:backing:defer:)
    pub fn send_window_init(
        receiver: Id,
        _sel: Sel,
        rect: CGRect,
        _style_mask: NSUInteger,
        _backing: NSUInteger,
        _defer_flag: bool,
    ) -> Id {
        if receiver.is_nil() {
            return Id::NIL;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            runtime.update_object(receiver, |data| {
                data.frame = rect;
            });
            receiver
        }

        #[cfg(target_os = "macos")]
        unsafe {
            let sel_name_ptr = native_bindings::sel_getName(sel.0 as *const c_void);
            let sel_str = if !sel_name_ptr.is_null() {
                CStr::from_ptr(sel_name_ptr).to_str().unwrap_or_default()
            } else {
                ""
            };

            if sel_str.starts_with("initWithFrame:") {
                let msg_send: extern "C" fn(*mut c_void, *const c_void, CGRect) -> *mut c_void =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                Id(msg_send(receiver.0, sel.0, rect))
            } else if sel_str.starts_with("setFrame:display:") {
                let msg_send: extern "C" fn(*mut c_void, *const c_void, CGRect, BOOL) -> *mut c_void =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                Id(msg_send(receiver.0, sel.0, rect, YES))
            } else {
                let defer_b: BOOL = if defer_flag { YES } else { NO };
                let msg_send: extern "C" fn(*mut c_void, *const c_void, CGRect, NSUInteger, NSUInteger, BOOL) -> *mut c_void =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                Id(msg_send(receiver.0, sel.0, rect, style_mask, backing, defer_b))
            }
        }
    }

    /// Sends a zero-argument message and returns a boolean value (e.g., isFlipped)
    pub fn send_bool_return(receiver: Id, sel: Sel) -> bool {
        if receiver.is_nil() {
            return false;
        }

        #[cfg(not(target_os = "macos"))]
        {
            let runtime = MockObjcRuntime::instance();
            let sel_name = runtime.selectors.lock().unwrap().iter()
                .find(|(_, &v)| v == (sel.0 as usize))
                .map(|(k, _)| k.clone())
                .unwrap_or_default();

            if sel_name == "isFlipped" {
                if let Some(obj) = runtime.get_object_data(receiver) {
                    return obj.is_flipped;
                }
            }
            false
        }

        #[cfg(target_os = "macos")]
        {
            let sel = sel.0 as *const c_void;
            let ptr = receiver.0;
            let res: BOOL = unsafe {
                let msg_send: unsafe extern "C" fn(*mut c_void, *const c_void) -> BOOL =
                    std::mem::transmute(native_bindings::objc_msgSend as *const ());
                msg_send(ptr, sel)
            };
            res == YES
        }
    }
}
