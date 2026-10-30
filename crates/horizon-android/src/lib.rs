//! Horizon Android platform integration and JNI bridge.
//!
//! # Arquitectura
//!
//! Este crate conecta el ciclo de vida de Android Activity / SurfaceView con
//! el motor de Horizon (Rust / wgpu). El flujo es:
//!
//! ```text
//! Kotlin (MainActivity)
//!   │
//!   ├── nativeInit(internalDir, resourcesDir) ──► configura paths, inicia log
//!   │                                             lanza GAME THREAD
//!   ├── nativeSurfaceCreated(surface) ──────────► entrega ANativeWindow al motor
//!   ├── nativeSurfaceChanged(w, h)   ──────────► notifica redimensionado
//!   ├── nativeSurfaceDestroyed()     ──────────► retira la superficie (pausa render)
//!   ├── nativeOnTouch(id, action, x, y)        ► encola evento táctil
//!   ├── nativeOnPause()  ──────────────────────► señal de pausa al motor
//!   ├── nativeOnResume() ──────────────────────► señal de reanudación
//!   └── nativeOnDestroy() ─────────────────────► señal de apagado, join del thread
//!
//! Game Thread (std::thread)
//!   └── horizon_android::engine::run(state_ref)
//!         ├── espera ANativeWindow (nativeSurfaceCreated)
//!         ├── inicializa wgpu sobre ANativeWindow
//!         ├── game loop: renderiza mientras SURFACE_READY && !PAUSED && !SHUTDOWN
//!         └── limpia recursos cuando SHUTDOWN
//! ```
//!
//! # Sobre la integración con horizon-client
//!
//! `game::run()` de `horizon-client` está diseñado para escritorio: lee args CLI
//! y crea un `winit::EventLoop`. En Android ese camino no aplica.
//! Por eso este crate implementa su propio bucle mínimo de arranque que:
//!   1. Configura paths y logging (igual que `main.rs` hace en escritorio).
//!   2. Inicializa `wgpu` directamente sobre el `ANativeWindow` recibido por JNI.
//!   3. Ejecuta un bucle de render stub (splash/carga) hasta que los recursos
//!      estén disponibles; luego conecta el motor completo.
//!
//! Entregas futuras (C+) expandirán el bucle para invocar la lógica de juego.

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;

// ── Señales de ciclo de vida (atómicos para comunicación JNI → game thread) ──

/// El motor ha sido inicializado al menos una vez.
static INITIALIZED: AtomicBool = AtomicBool::new(false);
/// Hay una superficie válida disponible para renderizar.
static SURFACE_READY: AtomicBool = AtomicBool::new(false);
/// La Activity está en pausa (onPause recibido).
static PAUSED: AtomicBool = AtomicBool::new(false);
/// El motor debe detenerse (onDestroy recibido).
static SHUTDOWN: AtomicBool = AtomicBool::new(false);
/// Ancho de superficie actual en píxeles.
static SURFACE_WIDTH: AtomicI32 = AtomicI32::new(0);
/// Alto de superficie actual en píxeles.
static SURFACE_HEIGHT: AtomicI32 = AtomicI32::new(0);

// ── Estado compartido entre JNI calls y game thread ───────────────────────────

/// Estado mutable compartido del motor.
struct EngineState {
    game_home: Option<PathBuf>,
    resources_dir: Option<PathBuf>,
    /// Puntero crudo a `ANativeWindow`. Nulo cuando la superficie no existe.
    native_window: *mut c_void,
    /// Handle del game thread para hacer join en onDestroy.
    game_thread: Option<std::thread::JoinHandle<()>>,
    /// Cola de eventos táctiles: (pointer_id, action, x, y).
    touch_queue: Vec<(i32, i32, f32, f32)>,
}

// SAFETY: `native_window` es un puntero opaco gestionado exclusivamente por
// los callbacks JNI serializados por el Mutex. El game thread sólo lo lee
// cuando SURFACE_READY es true y lo accede también bajo el lock.
unsafe impl Send for EngineState {}
unsafe impl Sync for EngineState {}

static ENGINE: Mutex<EngineState> = Mutex::new(EngineState {
    game_home: None,
    resources_dir: None,
    native_window: std::ptr::null_mut(),
    game_thread: None,
    touch_queue: Vec::new(),
});

// ── Módulo del motor (game loop Android) ─────────────────────────────────────

mod engine {
    use super::*;

    /// Punto de entrada del game thread.
    ///
    /// Por ahora implementa un bucle de espera-activa mínimo que:
    /// 1. Espera hasta que la superficie esté disponible.
    /// 2. Registra la inicialización exitosa del motor en logcat.
    /// 3. Entra en el bucle principal: duerme mientras pausado, sale en SHUTDOWN.
    ///
    /// Entregas D+ sustituirán este stub por la inicialización real de wgpu
    /// y el game loop de Horizon.
    pub fn run() {
        log::info!("Horizon game thread started");

        // Fase 1: esperar la primera superficie (con timeout de 30 s).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if SHUTDOWN.load(Ordering::Acquire) {
                log::info!("Horizon game thread: shutdown before surface — exiting");
                return;
            }
            if SURFACE_READY.load(Ordering::Acquire) {
                break;
            }
            if std::time::Instant::now() > deadline {
                log::error!("Horizon game thread: timed out waiting for ANativeWindow");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        let (w, h) = (
            SURFACE_WIDTH.load(Ordering::Relaxed),
            SURFACE_HEIGHT.load(Ordering::Relaxed),
        );
        log::info!("Horizon game thread: surface ready ({w}×{h}) — engine initialised");

        // Fase 2: bucle principal (stub — Entrega D añadirá wgpu aquí).
        loop {
            if SHUTDOWN.load(Ordering::Acquire) {
                log::info!("Horizon game thread: shutdown signal — exiting");
                break;
            }

            if PAUSED.load(Ordering::Acquire) || !SURFACE_READY.load(Ordering::Acquire) {
                // Pausado o sin superficie: esperar sin consumir CPU.
                std::thread::sleep(std::time::Duration::from_millis(32));
                continue;
            }

            // TODO (Entrega D): llamar al renderer de wgpu aquí.
            // Por ahora simplemente dormimos 16 ms (≈ 60 fps stub).
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        log::info!("Horizon game thread: clean exit");
    }
}

// ── JNI Exports ──────────────────────────────────────────────────────────────

/// **nativeInit** — Primer punto de entrada desde Kotlin.
///
/// Configura el directorio home del juego, inicializa el sistema de logging de
/// Horizon y lanza el game thread. Llamado una sola vez desde `Activity.onCreate`.
///
/// # Parámetros
/// - `internal_data_dir`: ruta de `context.filesDir` (almacenamiento privado).
/// - `resources_dir`: ruta donde se esperan los recursos del juego.
///
/// # Retorno
/// `JNI_TRUE` si la inicialización fue exitosa, `JNI_FALSE` en caso de error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    internal_data_dir: JString,
    resources_dir: JString,
) -> jboolean {
    // Inicializar android_logger una sola vez.
    if !INITIALIZED.swap(true, Ordering::SeqCst) {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_tag("Horizon")
                .with_max_level(log::LevelFilter::Debug),
        );
        log::info!("Horizon Android native library loaded");
    }

    // Leer rutas desde Java.
    let internal_path = match env.get_string(&internal_data_dir) {
        Ok(s) => String::from(s),
        Err(e) => {
            log::error!("nativeInit: failed to read internal_data_dir: {e}");
            return JNI_FALSE;
        }
    };
    let resources_path = match env.get_string(&resources_dir) {
        Ok(s) => String::from(s),
        Err(e) => {
            log::error!("nativeInit: failed to read resources_dir: {e}");
            return JNI_FALSE;
        }
    };

    let home_path = PathBuf::from(&internal_path);
    let res_path = PathBuf::from(&resources_path);

    log::info!("nativeInit: game_home  = {}", home_path.display());
    log::info!("nativeInit: resources  = {}", res_path.display());

    // Configurar el game_home de Horizon (equivale a --home en escritorio).
    horizon_client_render::logging::set_game_home(home_path.clone());

    // Cambiar el directorio de trabajo al home para que las rutas relativas
    // (saves/, options.txt, etc.) funcionen correctamente.
    if let Err(e) = std::env::set_current_dir(&home_path) {
        log::warn!("nativeInit: could not set current_dir: {e}");
    }

    // Inicializar el sistema de log de Horizon en home/logs/.
    horizon_client_render::logging::init(&home_path, "Render thread");
    log::info!("Horizon {}", horizon_client_render::crash::version());

    // Guardar paths en el estado compartido y lanzar el game thread.
    match ENGINE.lock() {
        Ok(mut state) => {
            state.game_home = Some(home_path);
            state.resources_dir = Some(res_path);

            // Lanzar el game thread sólo si aún no está corriendo.
            if state.game_thread.is_none() {
                SHUTDOWN.store(false, Ordering::SeqCst);
                PAUSED.store(false, Ordering::SeqCst);
                match std::thread::Builder::new()
                    .name("Horizon game thread".into())
                    .stack_size(8 * 1024 * 1024) // 8 MiB (mismo que escritorio)
                    .spawn(engine::run)
                {
                    Ok(handle) => {
                        state.game_thread = Some(handle);
                        log::info!("nativeInit: game thread launched");
                    }
                    Err(e) => {
                        log::error!("nativeInit: failed to spawn game thread: {e}");
                        return JNI_FALSE;
                    }
                }
            }
        }
        Err(e) => {
            log::error!("nativeInit: ENGINE lock poisoned: {e}");
            return JNI_FALSE;
        }
    }

    JNI_TRUE
}

/// **nativeSurfaceCreated** — La `SurfaceView` tiene una superficie válida.
///
/// Obtiene el `ANativeWindow` del objeto `android.view.Surface` recibido y lo
/// almacena para que el game thread lo use al inicializar wgpu.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceCreated(
    env: JNIEnv,
    _class: JClass,
    surface: JObject,
) {
    log::info!("nativeSurfaceCreated");

    #[cfg(target_os = "android")]
    {
        use ndk_sys::ANativeWindow_fromSurface;
        let env_raw = env.get_raw();
        let surface_raw = surface.as_raw();
        let window =
            unsafe { ANativeWindow_fromSurface(env_raw as *mut _, surface_raw as *mut _) };

        if window.is_null() {
            log::error!("nativeSurfaceCreated: ANativeWindow_fromSurface returned null");
            return;
        }

        log::info!("nativeSurfaceCreated: ANativeWindow = {:p}", window);
        if let Ok(mut state) = ENGINE.lock() {
            state.native_window = window as *mut c_void;
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        // Stub para compilación de escritorio (tests, CI sin NDK).
        let _ = (env, surface);
        log::debug!("nativeSurfaceCreated: stub (non-android build)");
    }

    SURFACE_READY.store(true, Ordering::SeqCst);
}

/// **nativeSurfaceChanged** — La superficie cambió de tamaño o formato.
///
/// Actualiza el viewport del renderer. El game thread lo leerá en el próximo
/// frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceChanged(
    _env: JNIEnv,
    _class: JClass,
    width: jint,
    height: jint,
) {
    let w = width.max(1);
    let h = height.max(1);
    log::info!("nativeSurfaceChanged: {w}×{h}");
    SURFACE_WIDTH.store(w, Ordering::Relaxed);
    SURFACE_HEIGHT.store(h, Ordering::Relaxed);

    if let Ok(mut state) = ENGINE.lock() {
        // Notificar al renderer (Entrega D: llamar renderer.recreate_surface()).
        let _ = &mut state.native_window; // será usado en Entrega D
    }
}

/// **nativeSurfaceDestroyed** — La `SurfaceView` ha destruido su superficie.
///
/// El renderer debe dejar de presentar frames inmediatamente. El `ANativeWindow`
/// se libera (su refcount baja a 0) y el puntero se pone a nulo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeSurfaceDestroyed(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeSurfaceDestroyed");

    // Primero señalizamos que no hay superficie para que el game thread
    // no intente presentar un frame mientras liberamos el window.
    SURFACE_READY.store(false, Ordering::SeqCst);

    #[cfg(target_os = "android")]
    {
        use ndk_sys::ANativeWindow_release;
        if let Ok(mut state) = ENGINE.lock() {
            if !state.native_window.is_null() {
                unsafe { ANativeWindow_release(state.native_window as *mut _) };
                log::debug!("nativeSurfaceDestroyed: ANativeWindow released");
                state.native_window = std::ptr::null_mut();
            }
        }
    }
}

/// **nativeOnTouch** — Evento táctil de Android.
///
/// `action`: 0=DOWN, 1=UP, 2=MOVE, 3=CANCEL, 5=POINTER_DOWN, 6=POINTER_UP.
/// Los eventos se encolan para consumo desde el game thread (Entrega E/F).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnTouch(
    _env: JNIEnv,
    _class: JClass,
    pointer_id: jint,
    action: jint,
    x: jfloat,
    y: jfloat,
) {
    log::trace!(
        "nativeOnTouch: ptr={pointer_id} action={action} ({x:.1},{y:.1})"
    );

    // Encolar para el game thread. Entrega E implementará la lectura.
    if let Ok(mut state) = ENGINE.lock() {
        // Limitar la cola a 256 eventos para evitar acumulación si el juego está pausado.
        if state.touch_queue.len() < 256 {
            state.touch_queue.push((pointer_id, action, x, y));
        }
    }
}

/// **nativeOnPause** — `Activity.onPause` recibido.
///
/// Señaliza al game thread que debe reducir o detener su actividad.
/// El renderer deja de presentar frames (PAUSED=true).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnPause(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnPause: pausing game thread");
    PAUSED.store(true, Ordering::SeqCst);
}

/// **nativeOnResume** — `Activity.onResume` recibido.
///
/// El game thread puede volver a renderizar cuando la superficie esté lista.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnResume(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnResume: resuming game thread");
    PAUSED.store(false, Ordering::SeqCst);
}

/// **nativeOnDestroy** — `Activity.onDestroy` recibido.
///
/// Señaliza el apagado al game thread y espera a que termine limpiamente.
/// Máximo 5 segundos de espera antes de continuar de todas formas.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_bycesardev_horizon_MainActivity_nativeOnDestroy(
    _env: JNIEnv,
    _class: JClass,
) {
    log::info!("nativeOnDestroy: requesting shutdown");

    SURFACE_READY.store(false, Ordering::SeqCst);
    SHUTDOWN.store(true, Ordering::SeqCst);

    // Esperar al game thread con timeout.
    let handle = ENGINE.lock().ok().and_then(|mut s| s.game_thread.take());
    if let Some(handle) = handle {
        log::info!("nativeOnDestroy: waiting for game thread...");
        // `join` no tiene timeout; usamos un thread auxiliar para no bloquear JNI indefinidamente.
        let join_thread = std::thread::spawn(move || {
            let _ = handle.join();
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !join_thread.is_finished() {
            if std::time::Instant::now() > deadline {
                log::warn!("nativeOnDestroy: game thread join timed out (5 s) — continuing");
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        log::info!("nativeOnDestroy: game thread finished");
    }

    // Flush de los logs antes de que el proceso muera.
    horizon_client_render::logging::flush();
    log::info!("nativeOnDestroy: clean shutdown complete");
}

/// Punto de entrada requerido por `android-activity` (dependencia indirecta de `winit`).
/// Evita que el linker dinámico falle con: `cannot locate symbol "android_main"`.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(_app: android_activity::AndroidApp) {
    log::info!("android_main entry point called by android-activity");
}
