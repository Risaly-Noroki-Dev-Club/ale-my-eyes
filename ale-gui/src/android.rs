use crate::AppWindow;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();

    let window = AppWindow::new().unwrap();
    crate::setup_app(&window);
    window.run().unwrap();
}
