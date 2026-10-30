//! Horizon Android platform integration and JNI bridge.
//! Connects Android Activity lifecycle, SurfaceView, and touch events to Horizon.

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, JNI_TRUE};
use jni::JNIEnv;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static SURFACE_READY: AtomicBool = AtomicBool::new(false);

/// Internal state holder for Android engine integration.
struct AndroidEngineState {
    game_home: Option<PathBuf>,
    resources_dir: Option<PathBuf>,
    native_window: *mut c_void,
    width: u32,
    height: u32,
}

unsafe impl Send for AndroidEngineState {}
unsafe impl Sync for AndroidEngineState {}

static ENGINE_STATE: Mutex<AndroidEngineState> = Mutex::new(AndroidEngineState {
    game_home: None,
    resources_dir: None,
    native_window: std::ptr::null_mut(),
    width: 0,
    height: 0,
});

/// Initialize logging and directories for Horizon Android.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    internal_data_dir: JString,
    resources_dir: JString,
) -> jboolean {
    if !INITIALIZED.swap(true, Ordering::SeqCst) {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_tag("Horizon")
                .with_max_level(log::LevelFilter::Info),
        );

        log::info!("Horizon Android native library initialized");
    }

    let internal_path: String = match env.get_string(&internal_data_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to read internal_data_dir string: {e}");
            return 0;
        }
    };

    let resources_path: String = match env.get_string(&resources_dir) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to read resources_dir string: {e}");
            return 0;
        }
    };

    let home_path = PathBuf::from(&internal_path);
    let res_path = PathBuf::from(&resources_path);

    log::info!("Setting game home: {}", home_path.display());
    log::info!("Setting resources dir: {}", res_path.display());

    // Point Horizon's logging and game_home to internal storage
    horizon_client_render::logging::set_game_home(home_path.clone());

    // Switch current working directory to the app home so relative paths (saves/, options.txt) work
    if let Err(e) = std::env::set_current_dir(&home_path) {
        log::warn!("Failed to set current_dir to {}: {e}", home_path.display());
    }

    if let Ok(mut state) = ENGINE_STATE.lock() {
        state.game_home = Some(home_path);
        state.resources_dir = Some(res_path);
    }

    JNI_TRUE
}

/// Called when SurfaceView surface is created in Kotlin.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceCreated(
    env: JNIEnv,
    _class: JClass,
    surface: JObject,
) {
    log::info!("nativeSurfaceCreated called");

    #[cfg(target_os = "android")]
    {
        use ndk_sys::ANativeWindow_fromSurface;
        let env_raw = env.get_raw();
        let surface_raw = surface.as_raw();

        let window = unsafe { ANativeWindow_fromSurface(env_raw as *mut _, surface_raw as *mut _) };
        if window.is_null() {
            log::error!("ANativeWindow_fromSurface returned null!");
            return;
        }

        log::info!("Acquired ANativeWindow: {:p}", window);
        if let Ok(mut state) = ENGINE_STATE.lock() {
            state.native_window = window as *mut c_void;
        }
    }

    SURFACE_READY.store(true, Ordering::SeqCst);
}

/// Called when SurfaceView surface dimensions change.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceChanged(
    _env: JNIEnv,
    _class: JClass,
    width: jint,
    height: jint,
) {
    log::info!("nativeSurfaceChanged: width={width}, height={height}");
    if let Ok(mut state) = ENGINE_STATE.lock() {
        state.width = width.max(1) as u32;
        state.height = height.max(1) as u32;
    }
}

/// Called when SurfaceView surface is destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceDestroyed(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeSurfaceDestroyed called");
    SURFACE_READY.store(false, Ordering::SeqCst);

    #[cfg(target_os = "android")]
    {
        use ndk_sys::ANativeWindow_release;
        if let Ok(mut state) = ENGINE_STATE.lock() {
            if !state.native_window.is_null() {
                unsafe { ANativeWindow_release(state.native_window as *mut _) };
                state.native_window = std::ptr::null_mut();
            }
        }
    }
}

/// Called on Android touch events (ACTION_DOWN, ACTION_UP, ACTION_MOVE, ACTION_POINTER_DOWN/UP).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnTouch(
    _env: JNIEnv,
    _class: JClass,
    pointer_id: jint,
    action: jint,
    x: jfloat,
    y: jfloat,
) {
    // Action codes:
    // 0: DOWN, 1: UP, 2: MOVE, 3: CANCEL, 5: POINTER_DOWN, 6: POINTER_UP
    log::debug!("nativeOnTouch: ptr={pointer_id}, action={action}, x={x:.1}, y={y:.1}");
}

/// Called on Activity onPause.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnPause(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnPause called");
}

/// Called on Activity onResume.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnResume(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnResume called");
}

/// Called on Activity onDestroy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnDestroy(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnDestroy called");
    SURFACE_READY.store(false, Ordering::SeqCst);
}
