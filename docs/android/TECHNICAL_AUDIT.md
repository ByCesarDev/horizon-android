# Auditoría Técnica: Port de Horizon a Android ARM64

> **Documento:** `docs/android/TECHNICAL_AUDIT.md`
> **Fecha:** 30 de Octubre de 2026
> **Estado:** Fase 0 completada (Auditoría Técnica)
> **Repositorio Principal:** [horizon-android](file:///c:/Users/Usuario/Desktop/Proyectos/horizon)
> **Repositorio de Referencia UX/Táctil:** [PocketMC-Repo](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC)

---

## 1. Resumen Ejecutivo

Este documento presenta la auditoría técnica exhaustiva requerida antes de comenzar la implementación del port de Horizon a Android ARM64 (`aarch64-linux-android`).

El objetivo fundamental es permitir que el cliente de Horizon (reescritura nativa de Minecraft Java Edition 26.3 en Rust con `wgpu`) se ejecute en Android preservando su arquitectura modular, sin bifurcar el motor ni romper la compatibilidad con plataformas de escritorio (Windows, Linux, macOS). Para la experiencia táctil, se audita en profundidad la implementación probada en C++ de PocketMC con el fin de reproducir de manera fidedigna su modelo de gestos, áreas de control, multitouch y opciones de configuración en Rust.

---

## 2. Arquitectura Actual de Horizon

### 2.1 Estructura del Workspace y Crates

El workspace de Horizon se define en [Cargo.toml](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/Cargo.toml) (Rust Edition 2024, `rust-version = 1.85 / 1.90`) y consta de 26 miembros activos:

| Crate                                                                                                                                                                                                   | Responsabilidad                                                                                                                                                                                                                                                                     | Dependencias Críticas                                                          | Compatibilidad Android ARM64                                     |
| :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------ | :--------------------------------------------------------------- |
| [`horizon-core`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-core)                                                                                                               | Primitivas matemáticas (`jmath`), RNG Java, ruido, posiciones, storage paletizado.                                                                                                                                                                                               | Puras de Rust (sin OS)                                                          | **Totalmente Compatible**                                  |
| [`horizon-world`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-world)                                                                                                             | Formato NBT, regiones Anvil, generación de terreno, física de entidades, redstone.                                                                                                                                                                                                | Puras de Rust (`flate2`, `bytemuck`, `glam`)                              | **Totalmente Compatible**                                  |
| [`horizon-protocol`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-protocol)                                                                                                       | Protocolo 777: codecs, encriptación, compresión, paquetes play/config.                                                                                                                                                                                                            | `aes`, `cfb-mode`, `flate2`                                               | **Totalmente Compatible**                                  |
| [`horizon-command`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-command)                                                                                                         | Parser brigadier de comandos vanilla.                                                                                                                                                                                                                                               | Puras de Rust                                                                   | **Totalmente Compatible**                                  |
| [`horizon-items`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-items)                                                                                                             | Definición de bloques, ítems, componentes de datos, recetas.                                                                                                                                                                                                                      | Puras de Rust                                                                   | **Totalmente Compatible**                                  |
| [`horizon-client-core`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-core)                                                                                                 | Textos, tablas de sonido, partículas, idioma, SNBT.                                                                                                                                                                                                                                | Puras de Rust                                                                   | **Totalmente Compatible**                                  |
| [`horizon-client-render`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render)                                                                                             | Renderer (`wgpu`), meshing de chunks, iluminación, pipelines gráficos, HUD, UI, shaders WGSL.                                                                                                                                                                                   | `wgpu 30.0.1`, `winit 0.30`, `cpal 0.16`, `naga`, `png`, `ab_glyph` | **Compatible** (requiere puente Surface Android)           |
| [`horizon-client-input`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-input)                                                                                               | Bindings de teclas, opciones de ventana, mouse, gamepad, portapapeles.                                                                                                                                                                                                              | `winit 0.30`, `arboard 3.6.1`, `horizon-dualsense`                        | **Requiere Ajustes** (`arboard` no compila en Android)   |
| [`horizon-threads`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-threads)                                                                                                         | Prioridades de hilos, afinidad y allocador `mimalloc`.                                                                                                                                                                                                                            | `mimalloc`, `libc`                                                          | **Ya Compatible** (incluye `cfg(target_os = "android")`) |
| [`horizon-assets`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-assets)                                                                                                           | Decodificación de modelos de bloques/ítems, atlas de texturas.                                                                                                                                                                                                                    | Puras de Rust                                                                   | **Totalmente Compatible**                                  |
| [`horizon-shaders`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-shaders) / [`horizon-shader-api`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-shader-api) | Pipeline y parsing de paquetes de shaders.                                                                                                                                                                                                                                          | `naga`, puras de Rust                                                         | **Totalmente Compatible**                                  |
| [`horizon-client`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client)                                                                                                           | Binario del juego ([`main.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/main.rs)), orquestación ([`game.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs)), simulación cliente/servidor integrado. | Agrega todos los crates del cliente                                             | **Requiere punto de entrada JNI (`cdylib`)**             |

### 2.2 Punto de Entrada y Ciclo de Vida Actual

1. **Entrada de escritorio:** [`crates/horizon-client/src/main.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/main.rs) invoca `std::env::args()`, configura el logger con [`logging::init`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/logging.rs) y llama a [`game::run()`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L16578).
2. **Event Loop:** El bucle de eventos se gestiona con `winit 0.30.13` a través de [`run_event_loop`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L16166) y la máquina de estados [`enum App`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L16429):
   - `App::Loading(Loading)`: Muestra la pantalla de carga con rotación de panorama mientras un worker thread carga recursos y modelos en segundo plano.
   - `App::Ready(Game)`: Ejecuta la simulación a 20 ticks/s y el renderizado a la tasa de refresco del monitor.
3. **Mecanismo de Un Solo Proceso / Hilos:** Por defecto desde la issue #1514, Horizon utiliza el modo **Server Thread** ([`crates/horizon-client/src/game/integrated.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game/integrated.rs)), donde un hilo simula el mundo con ticking server-side y el hilo principal actúa como cliente unido por memoria interna vía paquetes `horizon-protocol`.

---

## 3. Auditoría de Renderizado y Gráficos (wgpu)

### 3.1 Inicialización de GPU y Superficie

El renderizado se implementa en [`crates/horizon-client-render/src/gfx.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/gfx.rs):

- **Estructura Principal:** [`struct Renderer`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/gfx.rs#L367) posee `instance`, `adapter`, `device`, `queue`, `surface` y los pipelines WGSL.
- **Backends Gráficos:** [`backend_order`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/gfx.rs#L850) selecciona los backends según el sistema operativo:

  ```rust
  fn backend_order(api: GraphicsApi, os: &str, wgpu_backend: bool) -> Vec<wgpu::Backends> {
      // En Android (os == "android"), retorna:
      // GraphicsApi::Auto => vec![Backends::VULKAN, Backends::GL]
  }
  ```

  Esto coincide a la perfección con Android: intenta inicializar Vulkan en primer lugar y, si el dispositivo no dispone de Vulkan conforme, recurre al backend OpenGL ES (EGL).
- **Soporte Android en `third_party/wgpu-hal`:** La copia parcheada de `wgpu-hal` en el repositorio ya cuenta con código específico para Android:

  - Detección de versión de SDK mediante `android_system_properties`.
  - Integración con `ndk-sys` para swapchains nativos Vulkan sobre `ANativeWindow`.
- **Creación de Superficie Actual:** Actualmente en [`open_backend`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/gfx.rs#L1030) se realiza:

  ```rust
  instance.create_surface(window.clone())
  ```

  Donde `window` es un `Arc<winit::window::Window>`.
- **Recreación de Superficie:** [`Renderer::recreate_surface(&mut self)`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/gfx.rs#L2909) ya existe y maneja la pérdida o cambio de superficie reutilizando `spare_surface` o recreándola con `instance.create_surface(...)`.

### 3.2 Desafíos y Solución para Android

1. **Ciclo de vida de la superficie en Android (`SurfaceView`):**
   - La superficie gráfica en Android es efímera. Cuando la app pasa a segundo plano o rota, el sistema operativo destruye la `Surface` física (`surfaceDestroyed`).
   - El renderer no debe continuar enviando frames a una superficie destruida, ya que provocaría un cierre forzado inmediato (`SIGSEGV` en el driver de Vulkan/EGL).
   - **Solución:** Cuando Kotlin reciba `surfaceDestroyed`, notifica al puente JNI para que `renderer.release_surface()` ponga la superficie en `None`. Al volver a primer plano (`surfaceCreated`), se pasa el nuevo `ANativeWindow` y se invoca `renderer.recreate_surface()`.
2. **Creación de Surface desde JNI:**
   - La función nativa JNI recibe un `jobject surface` (de Android `android.view.Surface`).
   - Mediante la API NDK:
     ```c
     ANativeWindow* window = ANativeWindow_fromSurface(env, surface);
     ```
   - En Rust, utilizando `raw-window-handle` 0.6:
     ```rust
     let raw_handle = AndroidNdkWindowHandle::new(NonNull::new(window as *mut _).unwrap());
     let surface = instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
         raw_display_handle: RawDisplayHandle::Android(AndroidDisplayHandle::new()),
         raw_window_handle: RawWindowHandle::AndroidNdk(raw_handle),
     })?;
     ```

---

## 4. Auditoría del Sistema de Entrada (Input)

### 4.1 Arquitectura Actual en Horizon

- **Canales de Entrada:**
  - Teclado y Mouse: procesados en [`game.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L13588) mediante `WindowEvent::KeyboardInput`, `WindowEvent::CursorMoved`, `WindowEvent::MouseInput`, `WindowEvent::MouseWheel` y `DeviceEvent::MouseMotion`.
  - Gamepad / Controlador: [`crates/horizon-client/src/game/pad.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game/pad.rs) gestiona el estado de mandos reales (vía `gilrs`) y emula mandos virtuales (`horizon-ctl pad`).
- **Cómo llega la entrada al Jugador:**
  - En [`crates/horizon-client/src/game/remote.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game/remote.rs#L4260-L4277), las teclas se traducen al struct `Keys`:

    ```rust
    let keys = Keys {
        forward: held(KeyCode::KeyW),
        backward: held(KeyCode::KeyS),
        left: held(KeyCode::KeyA),
        right: held(KeyCode::KeyD),
        jump: held(KeyCode::Space),
        shift: held(KeyCode::ShiftLeft),
        sprint: held(KeyCode::ControlLeft),
        analog: None,
    }.with_stick(stick.filter(|_| playing));
    ```

    Y luego se empaquetan en bits para el paquete vanilla `ServerboundPlayerInputPacket`:- Bit 0: Adelante (`forward`)

    - Bit 1: Atrás (`backward`)
    - Bit 2: Izquierda (`left`)
    - Bit 3: Derecha (`right`)
    - Bit 4: Salto (`jump`)
    - Bit 5: Agacharse / Sneak (`shift`)
    - Bit 6: Carrera / Sprint (`sprint`)
  - **Rotación de Cámara:** [`mouse.on_move(...)`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L14569) aplica incrementos de `yaw` y `pitch` derivados del desplazamiento del cursor.

---

## 5. Referencia Exhaustiva de Controles Táctiles (PocketMC)

Al inspeccionar directamente el código fuente de PocketMC en [`c:\Users\Usuario\Desktop\Proyectos\PocketMC`](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC), se verifican las siguientes especificaciones exactas:

### 5.1 Distribución y Coordenadas ([`TouchscreenInput.cpp`](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC/src/client/player/input/touchscreen/TouchscreenInput.cpp))

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                  │
│                        [Chat] [F5] [Pausa]                       │  <-- Botones superiores centrados
│                                                                  │
│                                                                  │
│                                           ZONA DE CÁMARA         │
│                                           (UnifiedTurnBuild)     │
│                                           - Deslizar: Girar      │
│   [  N  ]                                 - Tap <250ms: Usar     │
│ [W][Sneak][E]                             - Hold >400ms: Romper  │
│   [  S  ]                                                        │
│                                                 [Vuelo / Salto]  │  <-- Pila derecha dinámica
│  D-pad Izquierdo                                [Toggle Vuelo]   │      (Cambia según si vuela
│  (BaseX, BaseY)                                 [Descenso Vuelo] │       o camina)
└──────────────────────────────────────────────────────────────────┘
```

1. **Dimensiones base del D-pad:**
   - Botón individual: `Bw = w * 0.08f` (8% del ancho de pantalla), `Bh = Bw` (cuadrado).
   - Posición base vertical: `BaseY = -8.0f + h - 3.0f * Bh`.
   - Posición base horizontal (modo diestro): `BaseX = 8.0f`.
   - Posición base horizontal (modo zurdo `OPTIONS_IS_LEFT_HANDED`): `BaseX = -8.0f + w - 3.0f * Bw`.
2. **Botones del D-pad:**
   - Arriba / Norte (`AREA_DPAD_N`): `(BaseX + Bw, BaseY)`.
   - Abajo / Sur (`AREA_DPAD_S`): `(BaseX + Bw, BaseY + 2 * Bh)`.
   - Izquierda / Oeste (`AREA_DPAD_W`): `(BaseX, BaseY + Bh)`.
   - Derecha / Este (`AREA_DPAD_E`): `(BaseX + 2 * Bw, BaseY + Bh)`.
   - **Centro del D-pad (`AREA_SNEAK`):** `(BaseX + Bw, BaseY + Bh)`. Alterna el estado de agacharse (`sneaking = !sneaking`). Reduce la velocidad a `0.3x`.
   - **Diagonales:** Áreas virtuales `aUpLeft` y `aUpRight` que permiten presionar simultáneamente `N+W` o `N+E` con arrastre continuo del dedo.
3. **Pila de Botones Derecha (Salto y Vuelo):**
   - Posición base horizontal: `BaseX_Right = w - 8.0f - 2.0f * Bw` (en modo zurdo: `8.0f + Bw`).
   - **Estado Caminando:**
     - Un único botón de salto (`AREA_DPAD_C` / `aJump`) ubicado en la fila central (`BaseY + Bh`), alineado simétricamente con el botón Sneak del D-pad.
     - Textura: `gui/jump_button.png` / `jump_press_button.png`.
   - **Estado Volando (`player->abilities.flying`):**
     - Botón superior (`AREA_DPAD_C`): Vuelo ascendente (`wantUp = true`). Textura: `gui/fly_up_button.png`.
     - Botón intermedio (`AREA_FLIGHT_TOGGLE`): Conmuta activar/desactivar vuelo. Textura: `gui/flight_button.png`.
     - Botón inferior (`AREA_FLY_DOWN`): Vuelo descendente (`wantDown = true`). Textura: `gui/fly_down_button.png`.
4. **Botones Superiores (Chat, F5, Pausa):**
   - Tamaño: `btnSize = GuiScale * 18` píxeles de GUI.
   - Espaciado: `spacing = 4.0f`.
   - Alineación: Centrados horizontalmente en la parte superior: `left = (w - (btnSize * 3 + spacing * 2)) / 2.0f; top = 4.0f;`.
   - Disparo: Se activan al soltar el dedo (`Multitouch::isReleased(p)`).

### 5.2 Discriminación de Gestos: Girar vs Romper vs Colocar ([`TouchInputHolder.h`](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC/src/client/player/input/touchscreen/TouchInputHolder.h))

La clase [`UnifiedTurnBuild`](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC/src/client/player/input/touchscreen/TouchInputHolder.h#L50) implementa la lógica que distingue un arrastre de cámara de una pulsación para minar o colocar bloques:

- **Área activa de cámara (`AREA_TURN`):** Toda la pantalla (`screenArea`) excluyendo el área del D-pad (`moveArea` con margen expandido del 5% + 10px), el área del inventario/hotbar (`inventoryArea`) y el área de pausa (`pauseArea`).
- **Estados de entrada:**
  ```cpp
  static const int State_None = 0;
  static const int State_Deciding = 1;
  static const int State_Turn = 2;
  static const int State_Destroy = 3;
  static const int State_Build = 4;
  ```
- **Parámetros temporales y de desplazamiento:**
  - `MaxBuildMovement = 20` píxeles de desplazamiento acumulado.
  - `RemovalMilliseconds = 400` milisegundos de pulsación continua.
- **Lógica de decisión:**
  1. Al apoyar el dedo (`Touch Down`): Se inicia en `State_Deciding`, `_startTurnTime = now` y `_totalMoveDelta = 0`.
  2. Si el dedo se mueve y `_totalMoveDelta >= 20` píxeles: El estado cambia a `State_Turn`. El movimiento se transforma linealmente en delta angular (`dx, dy`) multiplicador de sensibilidad para girar la cámara.
  3. Si el dedo se mantiene presionado durante `>= 400 ms` con desplazamiento `< 20` píxeles: El estado pasa a `State_Destroy`. Se emite la acción de minar continua (`BAI_FIRSTREMOVE` / `BAI_REMOVE | BAI_INTERACT`) y se calcula la opacidad alfa del círculo de progreso de rotura (`_holder->alpha`).
  4. Si el dedo se suelta antes de `250 ms` (`since < 0.25f`) con `_totalMoveDelta <= 20` píxeles: Se interpreta como un toque rápido y se emite la intención de colocar/usar o atacar (`BAI_BUILD | BAI_ATTACK`).
  5. **Filtro de "efecto ketchup":** Se descarta el delta del primer movimiento de puntero (`wasFirstMovement`) para evitar saltos bruscos provocados por la latencia del digitalizador táctil.

### 5.3 Opciones de Configuración Identificadas ([`ControlsScreen.cpp`](file:///c:/Users/Usuario/Desktop/Proyectos/PocketMC/src/client/gui/screens/ControlsScreen.cpp#L25-L29))

PocketMC expone 5 opciones clave en la pantalla de controles que debemos reproducir en Horizon:

1. `OPTIONS_USE_TOUCHSCREEN`: Activa o desactiva la capa de controles táctiles en pantalla.
2. `OPTIONS_AUTOJUMP`: Activa el salto automático al aproximarse a un bloque de desnivel.
3. `OPTIONS_ALLOW_SPRINT`: Permite correr rápidamente mediante doble pulsación hacia adelante.
4. `OPTIONS_IS_LEFT_HANDED`: Invierte el diseño en pantalla (D-pad a la derecha, botones de acción a la izquierda).
5. `OPTIONS_IS_JOY_TOUCH_AREA`: Define una zona delimitada y sombreada para el giro táctil con mayor sensibilidad (`1.8x` vs `1.0x`).

---

## 6. Auditoría de Dependencias del Workspace

A continuación se detalla la matriz de compatibilidad de dependencias de terceros para el target `aarch64-linux-android`:

| Dependencia / Archivo                                                                                          | Crate Afectado            | Diagnóstico en Android ARM64                                                                                                                   | Solución Propuesta                                                                                                                                                                                                                  |                  ¿Bloquea Primera Compilación?                  |
| :------------------------------------------------------------------------------------------------------------- | :------------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------: |
| [`arboard`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-input/Cargo.toml#L15)    | `horizon-client-input`  | **Incompatible.** Solo soporta Wayland, X11, Win32 y Cocoa. Produce error de compilación inmediato en Android.                           | Condicionar en `Cargo.toml`: `[target.'cfg(not(target_os = "android"))'.dependencies] arboard = ...`. En Android, `SystemClipboard` almacena el texto en memoria interna o delega al `ClipboardManager` de Android vía JNI. |                    **SÍ (Bloqueante)**                    |
| [`gilrs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/Cargo.toml#L59)            | `horizon-client`        | **Problema potencial de enlazado.** En Linux usa `libudev-dev`. En Android NDK no existe `libudev`.                                   | En la primera entrega compilar con `--no-default-features --features audio` o convertir `controllers` en un feature desacoplado para Android.                                                                                    | **SÍ (Bloqueante si no se desactiva en el build inicial)** |
| [`horizon-dualsense`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-dualsense)            | `horizon-client-input`  | Requiere acceso directo HID raw. No disponible en Android sin permisos de root o USB host.                                                      | Desactivar para Android bajo el feature `controllers`.                                                                                                                                                                             |                    No (controlado por feature)                    |
| [`wgpu 30.0.1`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/third_party/wgpu-hal)                      | `horizon-client-render` | **Compatible.** `third_party/wgpu-hal` ya contiene las ramas `target_os = "android"` con `ndk-sys` y `android_system_properties`. | Utilizar Vulkan NDK nativo (`libvulkan.so`) provisto por Android NDK r26+.                                                                                                                                                         |                            No bloquea                            |
| [`cpal 0.16`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/Cargo.toml#L28) | `horizon-client-render` | **Compatible.** Soporta AAudio y OpenSL ES en Android.                                                                                    | Configurar permisos de audio si se requiere; se puede arrancar con `Audio::silent` en pruebas iniciales.                                                                                                                           |                            No bloquea                            |
| [`horizon-threads`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-threads)                | `horizon-threads`       | **Compatible.** Contiene branches `cfg(any(target_os = "linux", target_os = "android"))`. Usa `libc` y `mimalloc`.                  | `mimalloc` compila en Android NDK r25+.                                                                                                                                                                                            |                            No bloquea                            |
| [`open`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/Cargo.toml#L72)             | `horizon-client`        | Exclusivo de Windows (`cfg(windows)`).                                                                                                        | Ya está aislado condicionalmente.                                                                                                                                                                                                   |                            No bloquea                            |
| [`winresource`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/Cargo.toml#L85)      | `horizon-client`        | Exclusivo de Windows build script.                                                                                                              | Ya está aislado condicionalmente.                                                                                                                                                                                                   |                            No bloquea                            |

---

## 7. Auditoría de Almacenamiento, Recursos y Sistema de Archivos

### 7.1 Acceso al Directorio del Juego (`game_home`)

En [`crates/horizon-client-render/src/logging.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/logging.rs#L58-L65):

```rust
pub fn game_home() -> PathBuf {
    if let Some(home) = HOME.get() {
        return home.clone();
    }
    match std::env::var_os("HORIZON_HOME").filter(|h| !h.is_empty()) {
        Some(h) => PathBuf::from(h),
        None => data_dir().join("horizon"),
    }
}
```

- **Ventaja de la arquitectura:** Ya existe [`set_game_home(home: PathBuf)`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client-render/src/logging.rs#L52).
- **Entorno Android:** En Android no existe un home tradicional de Linux. El puente JNI debe invocar `set_game_home` apuntando a:
  - Almacenamiento interno privado: `context.filesDir.absolutePath` (ej. `/data/user/0/net.bycesardev.horizon/files/horizon`), o
  - Almacenamiento externo de la app (sin requerir permisos especiales en Android 11+): `context.getExternalFilesDir(null)?.absolutePath`.

### 7.2 Rutas Relativas y Directorio Actual (`current_dir`)

En [`crates/horizon-client/src/game.rs`](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/crates/horizon-client/src/game.rs#L16618-L16629):

- Mundos guardados: `std::env::current_dir()?.join("saves")`.
- Opciones del cliente: `std::env::current_dir()?.join("options.txt")`.
- Caché de estructuras y biomas: `std::env::current_dir()?.join(".horizon-cache")`.
- Servidores guardados: `std::env::current_dir()?.join("servers.dat")`.
- **Estrategia en Android:** Durante la inicialización JNI en Rust, ejecutar:

  ```rust
  std::env::set_current_dir(&app_game_dir)?;
  ```

  Esto garantiza que el 100% del código existente que lee `saves/` o `options.txt` funcione de manera automática sin alterar una sola línea de lógica interna del juego.

### 7.3 Recursos Vanilla Preparados (`horizon-resources.json`)

- Horizon requiere los recursos procesados por `horizon-prepare` (texturas, atlas de fuentes, modelos horneados, tablas de idioma).
- En Android, los recursos no se pueden leer directamente de carpetas del sistema raíz.
- **Estrategia de despliegue:**
  - Los recursos mínimos necesarios se empaquetan en los `assets/` del APK de Android.
  - Al primer inicio de la app, Kotlin extrae o descomprime estos recursos hacia `getExternalFilesDir(null)/resources` y notifica a Horizon la ruta absoluta mediante `options.resources`.

---

## 8. Propuesta de Arquitectura de Integración Android

Para respetar el principio de **no bifurcar el cliente** y mantener Rust como el núcleo de simulación, la arquitectura se divide en dos componentes claramente delimitados:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        ANDROID APPLICATION (Kotlin)                    │
│                                                                        │
│  MainActivity.kt                                                       │
│   ├── Ciclo de vida (onCreate, onResume, onPause, onDestroy)          │
│   ├── SurfaceView / SurfaceHolder.Callback                             │
│   │    ├── surfaceCreated(Surface) ──> JNI nativeSurfaceCreated        │
│   │    ├── surfaceChanged(w, h)   ──> JNI nativeSurfaceChanged         │
│   │    └── surfaceDestroyed()     ──> JNI nativeSurfaceDestroyed       │
│   ├── onTouchEvent(MotionEvent)   ──> JNI nativeTouch(id, action, x, y)│
│   └── Extracción de Assets / Permisos de Almacenamiento                │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ JNI Bridge (C-ABI)
┌───────────────────────────────────▼────────────────────────────────────┐
│                    HORIZON ANDROID NATIVE BRIDGE (Rust)                │
│                                                                        │
│  crates/horizon-android (cdylib: libhorizon_android.so)                │
│   ├── jni_bridge.rs (Exports JNI: init, surface, touch, render)        │
│   ├── android_surface.rs (ANativeWindow_fromSurface -> wgpu::Surface)  │
│   ├── touch_bridge.rs (Recibe eventos táctiles de Android)             │
│   └── platform_storage.rs (set_current_dir, set_game_home)             │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Direct Rust API Call
┌───────────────────────────────────▼────────────────────────────────────┐
│                       HORIZON CORE GAME ENGINE                         │
│                                                                        │
│  crates/horizon-client & horizon-client-render                         │
│   ├── Renderer (wgpu: Vulkan / GLES sobre ANativeWindow)               │
│   ├── TouchInput / Gesture Recognition (Port de PocketMC)              │
│   │    ├── D-pad, Sneak, Salto, Vuelo dinámico                         │
│   │    └── UnifiedTurnBuild (Girar vs Romper vs Colocar)               │
│   ├── Simulación y Server Thread (Integrated Server / Remote)          │
│   └── Audio & Streaming Chunks                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### 8.1 Ventajas de esta Arquitectura

1. **Zero Impact on Desktop:** La creación de un crate específico `crates/horizon-android` (con `crate-type = ["cdylib"]`) permite generar `libhorizon_android.so` sin modificar el crate `horizon-client` de escritorio ni sus flags de compilación.
2. **Ciclo de Vida Preciso:** Kotlin maneja el ciclo de vida de la actividad Android y la recreación de la superficie sin necesidad de trucos frágiles en C.
3. **Paso Directo de Punteros:** La superficie gráfica `android.view.Surface` se entrega directamente a `ANativeWindow_fromSurface`, permitiendo a `wgpu` presentar los fotogramas sin capas intermedias de copia ni pérdida de rendimiento.

---

## 9. Toolchain y Entorno Recomendado

Para la compilación y prueba de Horizon en Android ARM64, se establece la siguiente configuración estándar:

- **Target Rust:** `aarch64-linux-android`
  - Comando: `rustup target add aarch64-linux-android`
- **Android NDK:** NDK r26b o r27 (soporta C++20, headers de Vulkan actualizados y toolchains Clang compatibles con Rust 1.85+).
- **Android SDK:**
  - `minSdkVersion`: 24 (Android 7.0 Nougat - garantiza disponibilidad de soporte Vulkan 1.0/1.1 y pthread QoS).
  - `targetSdkVersion`: 34 (Android 14).
- **Gradle:** 8.5+ con Gradle Wrapper y Kotlin 1.9+.
- **Herramienta de enlace Cargo-NDK:** `cargo-ndk` v3.5+ para automatizar el pasaje de variables de entorno del NDK (`CC`, `AR`, `CFLAGS`, sysroot) durante `cargo build --target aarch64-linux-android`.

---

## 10. Hoja de Ruta de Implementación Ordenada

|        Fase / Entrega        | Objetivo Principal                             | Tareas Clave                                                                                                                                 | Criterio de Aceptación                                                                                                                  |
| :---------------------------: | :--------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------- |
|   **A (Completada)**   | **Auditoría Técnica**                  | Inspección de crates, dependencias, wgpu y PocketMC.                                                                                        | Documento [docs/android/TECHNICAL_AUDIT.md](file:///c:/Users/Usuario/Desktop/Proyectos/horizon/docs/android/TECHNICAL_AUDIT.md) aprobado. |
| **B<br />(Completada)** | **Proyecto Android & Compilación Rust** | Crear estructura `android/`, configurar Gradle, aislar `arboard` y compilar `libhorizon_android.so` para `aarch64-linux-android`.    | Compilación exitosa y generación de APK instalable en dispositivo real.                                                                |
| **C<br />(Completada)** | **Inicialización JNI & Ciclo de Vida**  | Puente JNI, arranque del motor, gestión de paths (`game_home`, `current_dir`), manejo de pausa y suspensión.                           | La aplicación abre, ejecuta el log de Horizon en `logcat` e inicializa sin crashear.                                                  |
| **D<br />(Completada)** | **Superficie Android & Renderizado**     | Conectar `ANativeWindow` con `wgpu::Surface`, gestionar `surfaceCreated`/`surfaceDestroyed`, renderizar pantalla inicial de Horizon. | **Hito 1:** Primer fotograma de Horizon renderizado correctamente en la pantalla del móvil.                                       |
| **E<br />(Completada)** | **Puente de Entrada & Lógica Táctil**   | Port de `TouchAreaModel`, `GestureRecognizer`, `TouchTurnInput` y `TouchscreenInput` de PocketMC en Rust.                                   | Detección de toques, minado vs rotación, saltos y D-pad procesados y verificados en tiempo real.                                       |
| **F<br />(Completada)** | **HUD Táctil PocketMC en Pantalla**     | Pipeline 2D wgpu, carga de 30 sprites de PocketMC vía `AAssetManager`, renderizado interactivo de botones, cruceta y hotbar.         | Overlay de controles visible e interactivo sobre la pantalla en Redmi Note 11S.                                                       |
|          **G**          | **Conexión con la Simulación del Juego**| Conectar `TouchActionState` con los bindings del jugador de Horizon (`keyboard_direction`, `turn_player`, `attack`, `use_item`).     | El jugador se mueve, mira, salta, ataca e interactúa con el mundo mediante los controles táctiles.                                      |
|          **H**          | **Almacenamiento y Recursos**            | Despliegue de recursos de juego (`horizon-resources.json`), guardado y carga de mundos.                                                    | Crear un mundo nuevo en Android, guardarlo y reabrirlo sin corrupción.                                                                  |
|          **I**          | **Optimización y Pruebas Finales**      | Medir FPS, estabilidad térmica y memoria en Redmi Note 11S.                                                                                 | **Hito 2:** Sesión de juego fluida, estable y reproducible.                                                                       |

---

## 11. Resumen de Estado y Próxima Tarea

### Estado Actual (30-Oct-2026):

- **Entregas A, B, C, D, E y F completadas exitosamente.**
- **Entrega E (Lógica táctil):** Port de todo el subsistema táctil de PocketMC a Rust (`crates/horizon-android/src/touch/`).
- **Entrega F (HUD visual PocketMC):**
  - Copiados los 31 assets de PocketMC (`touchgui/`) a `android/app/src/main/assets/touchgui/`.
  - Implementado `crates/horizon-android/src/hud/` con pipeline 2D wgpu (`hud.wgsl`).
  - Carga automática de texturas PNG mediante `AAssetManager` de Android NDK (`AAssetManager_fromJava`).
  - Renderizado optimizado: 1 único búfer de vértices dinámico, 1 búfer de índices estático y 1 solo render pass por fotograma para rendimiento máximo en GPUs móviles (Mali-G57).
  - Verificado en el dispositivo físico Redmi Note 11S (`2307x1080` a 60 FPS):
    - `HudRenderer: loaded 30/30 sprites`
    - Controles responden a toques individuales, toques mantenidos, arrastre de cámara y botones del HUD (`pause`, `3rd`, `jump`, etc.).

### Próxima Tarea Concreta (Entrega G):

**Conectar `TouchActionState` con la simulación del jugador en Horizon:**
- Vincular `TouchActionState.move_direction` con la dirección de movimiento del jugador.
- Vincular `TouchActionState.look_delta` con la rotación de cámara (`turn_player`).
- Vincular `TouchActionState.attack` y `use_item` con romper/colocar bloques.
- Vincular los toggles de pantalla (pausa, chat, tercera persona).
