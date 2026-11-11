use crate::{bootstrap, lifecycle, needs_data, App};
use jni::{jni_sig, jni_str, objects::JObject, JavaVM};
use winit::event_loop::EventLoop;
use winit::platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid};

#[no_mangle]
fn android_main(app: AndroidApp) {
    match owns_session(&app) {
        Ok(true) => {}
        result => {
            eprintln!("[mobile] rejected Activity session: {result:?}");
            return;
        }
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[mobile] panic: {info}");
        lifecycle::java(lifecycle::JavaAction::Fatal);
        previous(info);
        std::process::abort();
    }));
    let roots = bootstrap::initialize(&app);
    let mut builder = EventLoop::builder();
    builder.with_android_app(app.clone());
    let event_loop = builder.build().expect("Android event loop");
    lifecycle::install(app, event_loop.create_proxy());
    match roots {
        Ok((data, saves)) => {
            lifecycle::java(lifecycle::JavaAction::EnableAudio);
            let mut frontend = App::new(data, saves);
            if let Err(e) = event_loop.run_app(&mut frontend) {
                eprintln!("[mobile] event loop: {e}");
            }
        }
        Err(message) => needs_data::run(event_loop, &message),
    }
    extern "C" {
        fn _exit(status: std::ffi::c_int) -> !;
    }
    unsafe { _exit(0) }
}

fn owns_session(app: &AndroidApp) -> jni::errors::Result<bool> {
    let ptr: jni::sys::jobject = app.activity_as_ptr().cast();
    JavaVM::singleton()?.attach_current_thread(|env| {
        let object = unsafe { env.as_cast_raw::<JObject>(&ptr)? };
        env.call_method(&*object, jni_str!("ownsSession"), jni_sig!("()Z"), &[])?
            .z()
    })
}
