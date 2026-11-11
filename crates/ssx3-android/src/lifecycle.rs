use jni::{jni_sig, jni_str, objects::JObject, JavaVM};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

static JAVA: AtomicU32 = AtomicU32::new(0);
static WAKE: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);
thread_local! { static ACTIVITY: std::cell::RefCell<Option<AndroidApp>> = const { std::cell::RefCell::new(None) }; }

#[no_mangle]
pub extern "system" fn Java_org_ssx3_android_AndroidActivity_nativeState(
    _: *mut jni::sys::JNIEnv,
    _: jni::sys::jobject,
    bits: i32,
) {
    JAVA.store(bits as u32, Ordering::Release);
    if let Some(wake) = WAKE.lock().unwrap().as_ref() {
        wake();
    }
}

pub fn install(app: AndroidApp, proxy: EventLoopProxy<()>) {
    *WAKE.lock().unwrap() = Some(Box::new(move || {
        let _ = proxy.send_event(());
    }));
    ACTIVITY.with(|a| *a.borrow_mut() = Some(app));
}
pub fn bits() -> u32 {
    JAVA.load(Ordering::Acquire)
}
pub fn destroyed() -> bool {
    bits() & 8 != 0
}
pub fn request_panel_rate(hz: u32) {
    ACTIVITY.with(|a| {
        let a = a.borrow();
        let Some(app) = a.as_ref() else { return };
        let result = (|| -> jni::errors::Result<()> {
            let vm = JavaVM::singleton()?;
            let ptr: jni::sys::jobject = app.activity_as_ptr().cast();
            vm.attach_current_thread(|env| {
                let object = unsafe { env.as_cast_raw::<JObject>(&ptr)? };
                env.call_method(
                    &*object,
                    jni_str!("requestPanelRate"),
                    jni_sig!("(I)V"),
                    &[jni::objects::JValue::Int(hz as i32)],
                )?;
                Ok(())
            })
        })();
        if let Err(e) = result {
            eprintln!("[mobile] panel request: {e}");
        }
    });
}

pub fn refresh_rate() -> Option<f64> {
    ACTIVITY.with(|a| a.borrow().as_ref().and_then(crate::display::refresh_rate))
}

#[derive(Clone, Copy, Debug)]
pub enum JavaAction {
    EnableAudio,
    Playing,
    Paused,
    Finish,
    Background,
    Fatal,
}

pub fn java(method: JavaAction) {
    ACTIVITY.with(|a| {
        let a = a.borrow();
        let Some(app) = a.as_ref() else { return };
        let result = (|| -> jni::errors::Result<()> {
            let vm = JavaVM::singleton()?;
            let ptr: jni::sys::jobject = app.activity_as_ptr().cast();
            vm.attach_current_thread(|env| {
                let object = unsafe { env.as_cast_raw::<JObject>(&ptr)? };
                let name = match method {
                    JavaAction::EnableAudio => jni_str!("enableAudio"),
                    JavaAction::Playing => jni_str!("playing"),
                    JavaAction::Paused => jni_str!("paused"),
                    JavaAction::Finish => jni_str!("finish"),
                    JavaAction::Background => jni_str!("background"),
                    JavaAction::Fatal => jni_str!("fatal"),
                };
                env.call_method(&*object, name, jni_sig!("()V"), &[])?;
                Ok(())
            })
        })();
        if let Err(e) = result {
            eprintln!("[mobile] Java {method:?}: {e}");
        }
    });
}
