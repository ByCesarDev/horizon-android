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

pub mod touch;

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

// ── Módulo del motor (game loop Android con wgpu) ───────────────────────────

mod engine {
    use super::*;
    use std::ptr::NonNull;

    /// Punto de entrada del game thread.
    ///
    /// Entrega D: Inicializa wgpu (Vulkan / GLES), crea una superficie wgpu::Surface
    /// a partir del ANativeWindow proporcionado por SurfaceHolder, adquiere Adapter/Device/Queue,
    /// y ejecuta el bucle de renderizado presentando los primeros frames gráficos en pantalla.
    pub fn run() {
        log::info!("Horizon game thread started");

        // 1. Inicializar la instancia wgpu
        let backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
        log::info!("Initializing wgpu::Instance with backends {backends:?}");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);

        let mut current_surface: Option<wgpu::Surface<'static>> = None;
        let mut gpu_resources: Option<(
            wgpu::Adapter,
            wgpu::Device,
            wgpu::Queue,
            wgpu::SurfaceConfiguration,
        )> = None;

        let mut touch_manager = touch::TouchManager::new(
            SURFACE_WIDTH.load(Ordering::Relaxed).max(1) as f32,
            SURFACE_HEIGHT.load(Ordering::Relaxed).max(1) as f32,
            touch::TouchOptions::default(),
        );

        let mut frame_count: u64 = 0;

        loop {
            if SHUTDOWN.load(Ordering::Acquire) {
                log::info!("Horizon game thread: shutdown signal — exiting");
                break;
            }

            if PAUSED.load(Ordering::Acquire) || !SURFACE_READY.load(Ordering::Acquire) {
                // Si la superficie se destruyó, soltar el surface de wgpu
                if !SURFACE_READY.load(Ordering::Acquire) && current_surface.is_some() {
                    log::info!("Horizon game thread: surface destroyed, releasing wgpu::Surface");
                    current_surface = None;
                }
                std::thread::sleep(std::time::Duration::from_millis(32));
                continue;
            }

            let native_window_ptr = match ENGINE.lock() {
                Ok(state) => state.native_window,
                Err(_) => std::ptr::null_mut(),
            };

            if native_window_ptr.is_null() {
                std::thread::sleep(std::time::Duration::from_millis(16));
                continue;
            }

            let width = (SURFACE_WIDTH.load(Ordering::Relaxed) as u32).max(1);
            let height = (SURFACE_HEIGHT.load(Ordering::Relaxed) as u32).max(1);

            // Si aún no tenemos superficie creada para este ANativeWindow
            if current_surface.is_none() {
                log::info!(
                    "Creating wgpu::Surface from ANativeWindow {:p} ({width}x{height})",
                    native_window_ptr
                );
                let surface_target = wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Android(
                        raw_window_handle::AndroidDisplayHandle::new(),
                    )),
                    raw_window_handle: raw_window_handle::RawWindowHandle::AndroidNdk(
                        raw_window_handle::AndroidNdkWindowHandle::new(
                            NonNull::new(native_window_ptr).expect("non-null ANativeWindow"),
                        ),
                    ),
                };

                let surface: wgpu::Surface<'static> =
                    match unsafe { instance.create_surface_unsafe(surface_target) } {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("Failed to create wgpu::Surface: {e}");
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        }
                    };

                // Inicializar Adapter, Device y Queue si no se ha hecho
                if gpu_resources.is_none() {
                    log::info!("Requesting GPU adapter compatible with surface...");
                    let adapter = match pollster::block_on(instance.request_adapter(
                        &wgpu::RequestAdapterOptions {
                            power_preference: wgpu::PowerPreference::HighPerformance,
                            compatible_surface: Some(&surface),
                            force_fallback_adapter: false,
                            ..Default::default()
                        },
                    )) {
                        Ok(a) => a,
                        Err(e) => {
                            log::error!("No compatible GPU adapter found: {e}");
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            continue;
                        }
                    };

                    let info = adapter.get_info();
                    log::info!(
                        "GPU Adapter: {} ({:?}, driver: {}, backend: {:?})",
                        info.name,
                        info.device_type,
                        info.driver,
                        info.backend
                    );

                    let limits = adapter.limits();
                    let features = adapter.features()
                        & (wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                            | wgpu::Features::RG11B10UFLOAT_RENDERABLE);

                    log::info!("Requesting device with adapter limits...");
                    let (device, queue) = match pollster::block_on(adapter.request_device(
                        &wgpu::DeviceDescriptor {
                            label: Some("Horizon Android Device"),
                            required_features: features,
                            required_limits: limits,
                            ..Default::default()
                        },
                    )) {
                        Ok(dq) => dq,
                        Err(e) => {
                            log::error!("Failed to request device: {e}");
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            continue;
                        }
                    };

                    let caps = surface.get_capabilities(&adapter);
                    let format = caps
                        .formats
                        .iter()
                        .copied()
                        .find(|f| f.is_srgb())
                        .unwrap_or(caps.formats[0]);
                    let present_mode = caps
                        .present_modes
                        .iter()
                        .copied()
                        .find(|&m| m == wgpu::PresentMode::Fifo)
                        .unwrap_or(caps.present_modes[0]);
                    let alpha_mode = caps.alpha_modes[0];

                    let config = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format,
                        width,
                        height,
                        present_mode,
                        desired_maximum_frame_latency: 2,
                        alpha_mode,
                        view_formats: vec![format],
                        color_space: wgpu::SurfaceColorSpace::default(),
                    };

                    log::info!(
                        "Configuring surface: {width}x{height}, format: {format:?}, present_mode: {present_mode:?}"
                    );
                    surface.configure(&device, &config);

                    gpu_resources = Some((adapter, device, queue, config));
                } else if let Some((_, ref device, _, ref mut config)) = gpu_resources {
                    config.width = width;
                    config.height = height;
                    log::info!("Re-configuring existing device surface: {width}x{height}");
                    surface.configure(device, config);
                }

                current_surface = Some(surface);
            }

            // Drenar y procesar cola táctil de Android (Entrega E)
            let mut pending_touches = Vec::new();
            if let Ok(mut state) = ENGINE.lock() {
                std::mem::swap(&mut state.touch_queue, &mut pending_touches);
            }
            for (pointer_id, action, x, y) in pending_touches {
                touch_manager.on_touch(pointer_id, action, x, y);
            }
            let touch_actions = touch_manager.update();

            // Registrar en logcat cuando el usuario interactúa
            if touch_actions.move_direction != [0.0, 0.0]
                || touch_actions.jump
                || touch_actions.sneak
                || touch_actions.attack
                || touch_actions.use_item
                || touch_actions.look_delta != (0.0, 0.0)
                || touch_actions.toggle_flight
                || touch_actions.toggle_pause
                || touch_actions.toggle_chat
                || touch_actions.toggle_perspective
            {
                log::info!(
                    "TouchAction: move={:?}, jump={}, sneak={}, attack={}, use={}, look=({:.1},{:.1}), pause={}, chat={}, 3rd={}",
                    touch_actions.move_direction,
                    touch_actions.jump,
                    touch_actions.sneak,
                    touch_actions.attack,
                    touch_actions.use_item,
                    touch_actions.look_delta.0,
                    touch_actions.look_delta.1,
                    touch_actions.toggle_pause,
                    touch_actions.toggle_chat,
                    touch_actions.toggle_perspective,
                );
            }

            // Renderizar un frame
            if let Some(surface) = &current_surface {
                if let Some((_, device, queue, config)) = &mut gpu_resources {
                    // Verificar si cambió el tamaño del viewport
                    if config.width != width || config.height != height {
                        config.width = width;
                        config.height = height;
                        surface.configure(device, config);
                        touch_manager.resize(width as f32, height as f32);
                    }

                    let (surface_texture, reconfigure) = match surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(f) => (Some(f), false),
                        wgpu::CurrentSurfaceTexture::Suboptimal(f) => (Some(f), true),
                        wgpu::CurrentSurfaceTexture::Outdated => {
                            surface.configure(device, config);
                            (None, false)
                        }
                        wgpu::CurrentSurfaceTexture::Lost => {
                            log::warn!("Surface lost, reconfiguring...");
                            surface.configure(device, config);
                            (None, false)
                        }
                        wgpu::CurrentSurfaceTexture::Timeout
                        | wgpu::CurrentSurfaceTexture::Occluded => (None, false),
                        wgpu::CurrentSurfaceTexture::Validation => {
                            log::error!("Surface validation error, reconfiguring...");
                            surface.configure(device, config);
                            (None, false)
                        }
                    };

                    if let Some(output) = surface_texture {
                        let view = output
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Horizon Clear Encoder"),
                            });

                        // Color temático Horizon con respuesta visual interactiva a toques
                        let phase = (frame_count as f64 * 0.03).sin() * 0.03;
                        let mut r = 0.12 + phase;
                        let mut g = 0.25 + phase;
                        let mut b = 0.45 + phase;

                        // Modulación de color para validar en pantalla cada acción táctil
                        if touch_actions.move_direction != [0.0, 0.0] {
                            // D-pad activo: tono esmeralda
                            g += 0.22;
                        }
                        if touch_actions.jump {
                            // Salto activo: iluminación brillante
                            r += 0.2;
                            g += 0.2;
                            b += 0.2;
                        }
                        if touch_actions.sneak {
                            // Agachado: tono sigiloso oscuro
                            r *= 0.5;
                            g *= 0.5;
                            b *= 0.5;
                        }
                        if touch_actions.attack {
                            // Toque prolongado (minar/atacar): tono rubí
                            r += 0.35;
                        }
                        if touch_actions.use_item {
                            // Tap simple (usar/colocar): destello dorado
                            r += 0.25;
                            g += 0.25;
                        }
                        if touch_actions.look_delta != (0.0, 0.0) {
                            // Deslizamiento de cámara: tono zafiro
                            b += 0.25;
                        }

                        {
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Horizon Clear Pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color {
                                            r: r.clamp(0.0, 1.0),
                                            g: g.clamp(0.0, 1.0),
                                            b: b.clamp(0.0, 1.0),
                                            a: 1.0,
                                        }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                        }

                        queue.submit(std::iter::once(encoder.finish()));
                        queue.present(output);

                        if reconfigure {
                            surface.configure(device, config);
                        }

                        frame_count += 1;
                        if frame_count == 1 || frame_count % 300 == 0 {
                            log::info!(
                                "Rendered frame #{frame_count} on Android ({width}x{height})"
                            );
                        }
                    }
                }
            }

            // Cadencia de refresco ≈ 60 fps
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
