# Plan Maestro: Horizon Android (Port táctil desde PocketMC)

¡Ahora sí! Ya revisé los dos repositorios y creo que tenemos una dirección bastante clara. Y hay algo importante: no vamos a inventar controles nuevos; vamos a portar la experiencia táctil de PocketMC a Horizon.

- [horizon-android](https://github.com/ByCesarDev/horizon-android?utm_source=chatgpt.com)
- [PocketMC-Repo](https://github.com/ByCesarDev/PocketMC-Repo?utm_source=chatgpt.com)

---

## 1. Qué tenemos actualmente

### Horizon Android

- **Sin implementación Android todavía**
- Es un fork de Horizon.
- Conserva su workspace Rust y sus crates.
- No encontré un proyecto Android, `AndroidManifest.xml`, `Activity` ni integración JNI propia.
- Ya tiene sistemas de entrada para teclado, ratón y gamepad.

### PocketMC

- **Controles táctiles existentes**
- Tiene `TouchscreenInput`.
- Tiene `TouchTurnInput`.
- Tiene `TouchAreaModel`.
- Tiene soporte multitáctil.
- Tiene configuración para zurdos, salto automático y área táctil de movimiento.

---

## 2. Los controles que vamos a portar

Esta es la distribución que encontré en PocketMC:

- [Minecraft: Pocket Edition — Everything you need to know! | iMore](https://images.openai.com/static-rsc-4/sR9ItK6wi7ZnRoMfQykV96wB2065tLgJ3Q5dz9xOlu2Rrt1TC3D7rWXxfGwusbymsfCl5rjFzlIxkWZoF1PK0YoONkYWM10aGfRwpY2J4kjd05fFFUOCaSt-0a1_AplDJofVKa_yIqTog5mZkFrRZob2elTjuQIGAEvMkj39occ?purpose=inline)
- [Simak Cara Mudah Atasi Minecraft Pocket Edition (PE) Crash atau Freeze - Denpasar Update](https://images.openai.com/static-rsc-4/biE1ue3BlSUWHVj_FYem1KH-bAiXQL1qD1AEAvQqtFFRy2evm96a5M7Ufy4ctiGkvS2TLEaCfWMWbvQH90hTDacUJbTg0i2OT5_wYXxmzbzllG7-37cxOpgCYt6npnylQnKog6pBgJEXGekwIo2-XWLpwR9gNuXAOXArEeDMxKY?purpose=inline)
- [Minecraft Controls | Minecraft](https://images.openai.com/static-rsc-4/ZmhM8gCAFnJEw8gWrU3f7TN9oerg4KZ48WF1-RMdaRtfgkV49-OH2sxdNUOO43Z5aY41IG4zsZkyVcxQyVfaiFk7lQ2eeSH7jqISwP3Dp12ZRyTV2887lTitrBE_Xu5e_3na8zOX5DHv3LF2p83P-4pXVAEy__BRPNHtPOaZ5ac?purpose=inline)

| Control                           | Comportamiento                    |
| :-------------------------------- | :-------------------------------- |
| **D-pad izquierdo**         | Movimiento en cuatro direcciones  |
| **Centro del D-pad**        | Agacharse                         |
| **Botón derecho**          | Saltar                            |
| **Botón inferior derecho** | Descender volando                 |
| **Botón de vuelo**         | Activar/desactivar vuelo          |
| **Parte superior**          | Chat, tercera persona y pausa     |
| **Zona táctil de cámara** | Girar la vista                    |
| **Toque prolongado**        | Romper bloques según el contexto |

También conservaría las opciones existentes:

- Controles para zurdos.
- Salto automático.
- Permitir sprint.
- Área táctil de movimiento.

No quiero que perdamos esos detalles. Por ejemplo, PocketMC distingue entre mantener pulsado para romper y deslizar para girar la cámara. Eso hay que reproducirlo correctamente, no convertir todos los toques en simples clics.

---

## 3. Cómo lo integraría en Horizon

No copiaría las clases C++ directamente. Portaría su comportamiento a Rust y lo conectaría con el sistema de entrada de Horizon.

### La arquitectura propuesta:

```text
Android App
│
├── Android Activity
│   ├── Surface lifecycle
│   ├── Touch events
│   └── Application lifecycle
│
└── Horizon Native
    ├── Android Platform
    │   ├── Window / Surface
    │   ├── Graphics initialization
    │   └── Touch event bridge
    ├── Touch Controls
    │   ├── TouchscreenInput
    │   ├── TouchAreaModel
    │   ├── TouchTurnInput
    │   └── Gesture recognition
    └── Horizon Client
        ├── Player input
        ├── Camera
        └── Renderer
```

La idea es que los controles táctiles produzcan las mismas acciones que el teclado y el ratón. Así no modificamos la lógica del jugador para Android.

---

## 4. Orden de implementación

Yo dividiría el trabajo en estos hitos:

1. **Preparar el proyecto Android:** Crear el proyecto Gradle, Activity, AndroidManifest y configuración del NDK.
2. **Compilar Horizon para ARM64:** Generar la biblioteca Rust nativa para `aarch64-linux-android` e integrarla con Android.
3. **Conseguir renderizado en pantalla:** Conectar la superficie Android con wgpu y conseguir que Horizon renderice correctamente.
4. **Portar los controles de PocketMC:** D-pad, salto, agacharse, cámara, vuelo, chat, tercera persona y pausa.
5. **Adaptar la interfaz táctil:** Escalado, posiciones, orientación de pantalla, tamaños y opciones de configuración.
6. **Pruebas en dispositivos reales:** Multitouch, cambios de orientación, suspensión, reanudación y rendimiento.

---

## 5. Una decisión que quiero conservar

No haría que Android se convierta en una versión independiente de Horizon.

La aplicación Android debe ser una plataforma más del mismo cliente. Si después añadimos soporte para tablets, controles físicos o incluso otras plataformas móviles, no tendremos que duplicar el motor.

El primer objetivo concreto será conseguir que Horizon abra en Android y renderice su pantalla inicial. Después incorporamos los controles de PocketMC, respetando su distribución y comportamiento originales.

Y mantendría ambos repositorios separados: **horizon-android** como proyecto principal y **PocketMC-Repo** como referencia para portar la experiencia móvil.

---

---

# ACA EL PLAN:

Vamos a plantearlo como un proyecto de verdad, no como una simple adaptación de pantalla. La idea es convertir Horizon en un cliente de Minecraft Java Edition que pueda ejecutarse en Android, usando PocketMC como referencia de experiencia móvil, pero sin arrastrar innecesariamente su arquitectura C++.

La prioridad será conseguir primero una versión Android funcional y estable. Después construiremos los controles táctiles y adaptaremos la interfaz. El soporte de shaderpacks, las propiedades dinámicas de bloques y otras características avanzadas quedan fuera de esta primera etapa: no quiero que nos distraigan antes de tener el juego funcionando en el teléfono.

---

## Plan maestro: Horizon Android

### Objetivo principal

Portar Horizon a Android ARM64 manteniendo Rust como lenguaje principal y aprovechando su arquitectura actual.

---

### 1. Arquitectura objetivo

La aplicación tendrá dos partes bien separadas:

#### Android Application — Kotlin

- Responsable de integrarse con el sistema operativo.
- Activity y ciclo de vida.
- Ventana y superficie de renderizado.
- Permisos y almacenamiento.
- Eventos táctiles y teclado.
- Orientación y configuración del dispositivo.
- JNI / Native bridge.

#### Horizon Native — Rust

- El juego y su lógica principal.
- Inicialización y cierre del motor.
- Renderer y backend gráfico.
- Mundo, chunks y entidades.
- Red y almacenamiento.
- Entrada del jugador.
- Adaptación de controles táctiles.

> **Principio importante:** Kotlin no debe convertirse en una segunda implementación del juego. Su trabajo será conectar Horizon con Android. La lógica del juego, el procesamiento de entrada y el renderizado deben permanecer en Rust siempre que sea viable.

---

### 2. Fases de desarrollo

#### Fase 0 — Auditoría técnica y preparación

**Primero:** Antes de escribir código, hay que conocer exactamente qué necesita Horizon para ejecutarse fuera de escritorio.

**Tareas:**

- Revisar el workspace, crates y puntos de entrada de Horizon.
- Identificar dependencias que no compilan para Android.
- Localizar la inicialización del renderer, ventana, input y audio.
- Revisar cómo se administra el ciclo de vida del cliente.
- Identificar las dependencias de sistema y APIs exclusivas de escritorio.
- Confirmar las versiones de Rust, Android NDK, SDK y Gradle necesarias.
- Revisar licencias del código y dependencias que se incorporarán.

**Entregable:** un documento de compatibilidad Android con bloqueos concretos, soluciones y decisiones técnicas. No se modifica la arquitectura a ciegas.

#### Fase 1 — Infraestructura Android

Crear el proyecto Android dentro del repositorio `horizon-android`.

**Estructura propuesta:**

```text
horizon-android/
├── android/
│   ├── app/
│   │   ├── src/main/
│   │   │   ├── java/.../
│   │   │   │   └── MainActivity.kt
│   │   │   ├── jniLibs/
│   │   │   ├── res/
│   │   │   └── AndroidManifest.xml
│   │   └── build.gradle.kts
│   ├── build.gradle.kts
│   └── settings.gradle.kts
├── crates/
│   └── ... (Horizon)
├── Cargo.toml
└── ...
```

**Tareas:**

- Configurar Gradle, Kotlin y Android SDK.
- Integrar Android NDK y Cargo.
- Compilar para `aarch64-linux-android`.
- Generar e integrar la biblioteca nativa Rust.
- Crear la Activity y el puente JNI.
- Implementar inicialización, pausa, reanudación y cierre del motor.
- Preparar el manejo de errores y logs nativos.

**Criterio de aceptación:** la aplicación se instala, abre y puede inicializar la biblioteca Rust en un dispositivo ARM64 real.

#### Fase 2 — Ventana y renderizado

Esta es una de las etapas más delicadas. Que Rust compile para Android no significa que el renderer pueda dibujar en la pantalla.

**Tareas:**

- Integrar el ciclo de vida de Surface de Android.
- Conectar la superficie al sistema de ventanas que utilice Horizon.
- Revisar la compatibilidad de wgpu con Android.
- Configurar el backend gráfico disponible.
- Verificar Vulkan y contemplar un backend alternativo cuando sea necesario.
- Adaptar el tamaño del viewport y el escalado de pantalla.
- Gestionar la pérdida y recreación de la superficie.
- Evitar que el renderer continúe utilizando una superficie destruida.

**Decisión técnica:** no reemplazar wgpu por Vulkan directo de entrada. Primero comprobaremos si el backend Vulkan de wgpu funciona correctamente en Android. Reescribir el renderer ahora aumentaría mucho el alcance sin demostrar que sea necesario.

> **Hito 1:** Horizon arranca en Android y muestra una imagen renderizada correctamente.

#### Fase 3 — Sistema de entrada Android

Antes de diseñar los botones táctiles, necesitamos una capa de entrada independiente de la plataforma.

La intención es que el motor reciba acciones abstractas, no eventos específicos de Android.

Por ejemplo:

```rust
enum GameInput {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Sneak,
    Sprint,
    Attack,
    Use,
    OpenInventory,
}
```

*La implementación definitiva deberá respetar el sistema de input existente en Horizon; este ejemplo representa el concepto, no una instrucción para reemplazarlo sin auditoría.*

**Tareas:**

- Crear el puente de eventos Android hacia Rust.
- Manejar multitouch mediante identificadores de puntero.
- Diferenciar pulsaciones, mantenimientos y desplazamientos.
- Evitar que un dedo controle simultáneamente dos zonas incompatibles.
- Mantener estados de botones mientras estén presionados.
- Resolver correctamente la cancelación de gestos y la pérdida de foco.
- Preparar la integración con teclado, mouse y mandos Bluetooth.

**Criterio de aceptación:** el motor puede recibir y procesar entradas simultáneas sin depender de la interfaz visual.

#### Fase 4 — Portar los controles táctiles de PocketMC

Aquí sí utilizaremos PocketMC como referencia directa. No quiero que Antigravity invente un diseño nuevo ni que simplifique los controles sin autorización.

- [Minecraft Controls | Minecraft](https://images.openai.com/static-rsc-4/ZmhM8gCAFnJEw8gWrU3f7TN9oerg4KZ48WF1-RMdaRtfgkV49-OH2sxdNUOO43Z5aY41IG4zsZkyVcxQyVfaiFk7lQ2eeSH7jqISwP3Dp12ZRyTV2887lTitrBE_Xu5e_3na8zOX5DHv3LF2p83P-4pXVAEy__BRPNHtPOaZ5ac?purpose=inline)
- [5 Tips for New Minecraft Pocket Edition Players](https://images.openai.com/static-rsc-4/YSBzvgeYbhV32W7UKfHjpuiNC_QaCyoRghpICJ-MZNvyahP51t5ehTH1rtT80zEuRM5KK9WA09k-VP852g0lBWPzT99Edj6p39gYBy90zD6A9PoIpf7fCOAPUoSp-bOuZbplie-1utD9uIIRHPVVr2dDEIxzI7ffDu4xhe6xUp4?purpose=inline)
- [Block Clutch Practice Map For MCPE 1.18 (New Controls) - MCPE Central](https://images.openai.com/static-rsc-4/CMpU5kimaz1mkuDlMyfP9kq6uu6JT9Dtm6z0I2y4RwNH-hU0oA7okjiJeE8hX-uWOh1mBP26F_x3xINZkb8WXq1xawSQN0ksP648T_bK8qtbX0MvVDrXdn2qDFQzVCGZ86X346v9ON5kG6alylj5qWG1VY6impYWyFOjRfx6d8A?purpose=inline)

**Elementos que debemos reproducir:**

| Control                    | Comportamiento                          |
| :------------------------- | :-------------------------------------- |
| **D-pad**            | Movimiento en cuatro direcciones        |
| **Centro del D-pad** | Agacharse                               |
| **Salto**            | Saltar                                  |
| **Cámara**          | Girar la vista arrastrando              |
| **Ataque**           | Romper bloques y atacar                 |
| **Interacción**     | Usar bloques y objetos                  |
| **Vuelo**            | Activar y descender cuando corresponda  |
| **Chat**             | Abrir el chat                           |
| **Perspectiva**      | Cambiar entre primera y tercera persona |
| **Pausa**            | Abrir el menú de pausa                 |

**También debemos conservar las opciones que ya tiene PocketMC:**

- Controles para zurdos.
- Salto automático.
- Permitir sprint.
- Área de movimiento táctil.
- Configuración y ajuste de los controles.

No debemos asumir que todos los botones se muestran permanentemente: hay que revisar el comportamiento real de las clases originales y reproducirlo.

**Criterio de aceptación:** una persona que ya conoce los controles móviles de PocketMC debe poder jugar sin tener que aprender un esquema nuevo.

#### Fase 5 — Adaptación de interfaz y HUD

El HUD de escritorio no debe simplemente reducirse para caber en una pantalla pequeña.

**Tareas:**

- Adaptar inventario y barra rápida.
- Ajustar los indicadores de salud, hambre y experiencia.
- Revisar menús y pantallas de configuración.
- Adaptar chat y cuadros de diálogo.
- Mejorar escalado según resolución y densidad.
- Permitir que la interfaz se adapte a distintas proporciones de pantalla.
- Evitar que los botones táctiles tapen información importante.

**Criterio de aceptación:** se puede jugar y navegar por los menús cómodamente en una pantalla móvil.

#### Fase 6 — Almacenamiento y recursos

Android tiene restricciones diferentes a las de escritorio.

**Tareas:**

- Definir dónde se guardan mundos, configuraciones y logs.
- Implementar el acceso a los recursos del juego.
- Gestionar permisos y selección de archivos mediante mecanismos compatibles con Android.
- Evitar depender de rutas absolutas de Windows o Linux.
- Preparar importación y exportación de mundos.
- Gestionar correctamente el espacio disponible.

No debemos asumir que podremos escribir libremente en cualquier carpeta del almacenamiento compartido. La solución debe respetar el modelo de almacenamiento de Android.

#### Fase 7 — Optimización y estabilidad

No basta con que el juego abra: debe ser razonablemente jugable.

**Tareas:**

- Medir FPS y tiempos de frame.
- Medir uso de CPU y memoria.
- Evaluar carga y reconstrucción de chunks.
- Revisar consumo energético y temperatura.
- Optimizar la gestión de recursos.
- Comprobar pausas, reanudaciones y cambios de orientación.
- Detectar cierres por presión de memoria.
- Revisar estabilidad de red y audio.

El Redmi Note 11S será un dispositivo de referencia útil para detectar limitaciones reales de hardware, pero no asumiremos compatibilidad gráfica sin probarla.

> **Hito 2:** Una sesión de juego estable en un dispositivo Android real, con controles táctiles completos.

---

### 3. Organización del desarrollo

Para evitar que Antigravity intente modificar todo el proyecto de una vez, dividiremos el trabajo en entregas pequeñas.

|   Entrega   | Resultado                                      |
| :---------: | :--------------------------------------------- |
| **A** | Auditoría de Horizon y compatibilidad Android |
| **B** | Proyecto Android y compilación Rust           |
| **C** | Inicialización JNI y ciclo de vida            |
| **D** | Superficie Android y renderizado               |
| **E** | Sistema de entrada multiplataforma             |
| **F** | Controles táctiles de PocketMC                |
| **G** | HUD y menús adaptados                         |
| **H** | Almacenamiento y recursos                      |
| **I** | Optimización y pruebas finales                |

**Cada entrega deberá incluir:**

- Código funcional.
- Pruebas de compilación.
- Documentación de las decisiones técnicas.
- Registro de problemas conocidos.
- Confirmación de que no se rompió la versión de escritorio.

No avanzaremos a una fase que dependa de otra hasta que su criterio de aceptación esté cumplido.

---

### 4. Pruebas que debemos contemplar

No quiero que las pruebas se limiten a comprobar si la aplicación abre.

- **Compatibilidad:**
  - ARM64.
  - Diferentes resoluciones y densidades.
  - Diferentes versiones de Android compatibles.
  - Vulkan y backend alternativo, si aplica.
- **Controles:**
  - Multitouch.
  - Movimiento y cámara simultáneos.
  - Mantener para romper bloques.
  - Interacciones rápidas.
  - Controles para zurdos.
  - Teclado y mouse externos.
- **Ciclo de vida:**
  - Minimizar y volver al juego.
  - Bloquear y desbloquear el teléfono.
  - Cambiar de aplicación.
  - Recrear la superficie gráfica.
  - Cerrar el juego correctamente.
- **Experiencia de juego:**
  - Crear y cargar mundos.
  - Moverse y saltar.
  - Romper y colocar bloques.
  - Abrir inventario y menús.
  - Conectarse a servidores.

---

### 5. Riesgos técnicos y cómo los vamos a tratar

| Riesgo                                                          | Tratamiento                                                  |
| :-------------------------------------------------------------- | :----------------------------------------------------------- |
| **Dependencias incompatibles con Android**                | Aislarlas detrás de abstracciones de plataforma             |
| **wgpu no inicializa correctamente**                      | Investigar surface y backend antes de cambiar el renderer    |
| **Problemas con JNI**                                     | Mantener un puente pequeño y explícito                     |
| **Eventos multitouch incorrectos**                        | Diseñar y probar la gestión de punteros desde el principio |
| **Diferencias entre almacenamiento Android y escritorio** | Implementar una capa de almacenamiento multiplataforma       |
| **Memoria y rendimiento limitados**                       | Medir antes de realizar optimizaciones                       |
| **Cambios que rompen escritorio**                         | Compilación y pruebas de regresión en cada entrega         |

> **Una regla importante:** no crearemos una bifurcación independiente del motor para Android. Queremos que Horizon tenga soporte Android dentro de su arquitectura multiplataforma, no mantener dos juegos distintos.

---

### 6. Qué NO vamos a implementar todavía

Para mantener el alcance controlado, estas características quedan para después:

- Compatibilidad con shaderpacks de Iris.
- Sistema de propiedades dinámicas para bloques.
- Nuevos tipos de `BlockEntities`.
- Modloader y compatibilidad avanzada con mods.
- Reemplazo de wgpu por Vulkan directo.
- Rediseño profundo del renderer.
- Funciones exclusivas de Android que no sean necesarias para jugar.

*No significa que estén descartadas. Significa que primero necesitamos una base Android sólida.*

---

### 7. Estructura de trabajo y repositorios

- **Repositorio principal:** [horizon-android](https://github.com/ByCesarDev/horizon-android?utm_source=chatgpt.com)
  - Desarrollo del motor y soporte Android.
- **Repositorio de referencia:** [PocketMC](https://github.com/ByCesarDev/PocketMC-Repo?utm_source=chatgpt.com)
  - Referencia de comportamiento, controles táctiles y experiencia móvil. No será una dependencia del nuevo motor.

Recomiendo organizar las ramas así:

- `main`: código estable.
- `android/platform`: integración Android.
- `android/input`: sistema de entrada.
- `android/touch-controls`: controles táctiles.
- `android/ui`: interfaz móvil.

Las ramas pueden integrarse progresivamente mediante PR. No conviene que Antigravity haga una reescritura masiva directamente sobre `main`.

---

### 8. Primer trabajo para Antigravity

No empezaría pidiéndole que implemente Android todavía. Primero debe entender Horizon y producir un diagnóstico técnico.

Este sería el primer encargo:

```text
Task 001 — Horizon Android Port: Technical Audit
We are preparing to port Horizon to Android.
Repository: https://github.com/ByCesarDev/horizon-android
Reference repository: https://github.com/ByCesarDev/PocketMC-Repo

Project objective
Turn Horizon into a functional Minecraft Java Edition client on Android ARM64.
The game engine must remain primarily written in Rust. Android-specific integration may use Kotlin/Java, JNI and the Android NDK.
PocketMC is a behavioral and UX reference, especially for its existing touchscreen controls. We are NOT porting PocketMC's C++ engine into Horizon.

IMPORTANT: Audit only
Do not implement the Android port yet.
Do not rewrite existing architecture, replace the renderer, introduce unnecessary dependencies, or modify unrelated game systems.
Your task is to thoroughly inspect the current repository and produce a technical audit.

Areas to investigate
1. Project architecture
- Workspace structure and crates.
- Main client entry point.
- Game initialization and shutdown.
- Platform abstractions.
- Existing desktop-specific implementations.

2. Rendering
- Current renderer architecture.
- How windows and surfaces are created.
- How wgpu is initialized.
- Which graphics backends are supported.
- What must change to support Android surfaces.
- Whether the current architecture can support Android without replacing the renderer.

3. Input
- Existing keyboard, mouse and controller input systems.
- How input reaches the player and camera.
- Whether input can be extended with touch events without breaking desktop input.
- Required changes for multitouch.

4. Dependencies
- Identify dependencies that may not compile on Android.
- Identify dependencies requiring platform-specific alternatives.
- Identify native libraries and system APIs.
- Check Android ARM64 compatibility.

5. Audio, storage and resources
- Identify platform-specific assumptions.
- Determine how worlds, settings, assets and logs are accessed.
- Identify potential Android restrictions.

6. Android integration proposal
- Determine the minimum viable architecture for:
  - Android Activity.
  - JNI bridge.
  - Rust native library.
  - Android Surface lifecycle.
  - wgpu surface initialization.
  - Android input forwarding.

7. PocketMC touchscreen reference
- Inspect the actual source code in PocketMC, especially:
  - TouchscreenInput.h/.cpp
  - TouchAreaModel.h
  - TouchTurnInput.h
  - TouchInputHolder.h
  - ControlsScreen.cpp
  - Multitouch.cpp
- Document the actual behavior and configuration options. Do not assume behavior based only on class names.

Required deliverables
Create docs/android/TECHNICAL_AUDIT.md.
The document must include:
- Current architecture overview.
- Relevant source files and their responsibilities.
- Android compatibility assessment.
- Blocking issues.
- Proposed architecture.
- Recommended Android toolchain.
- wgpu and graphics integration strategy.
- Input and touchscreen integration strategy.
- PocketMC behavior reference.
- Ordered implementation roadmap.
- Risks and testing requirements.

For each identified issue, include:
- Relevant file or dependency.
- Why it is a problem.
- Proposed solution.
- Whether it blocks the first Android build or can wait.

Constraints
- Preserve desktop functionality.
- Keep Rust as the primary game implementation.
- Prefer existing abstractions over unnecessary rewrites.
- Do not assume Vulkan is automatically faster.
- Do not implement shaderpacks or mod support as part of this task.
- Do not copy PocketMC C++ code into Horizon.
- Do not claim Android compatibility without verifying it.
- At the end, provide a concise summary of the current state and the exact next implementation task.
- Only create the audit document and any minimal documentation directories required. Do not modify game code.
```

---

### 9. Cuándo consideraremos terminado el proyecto

El objetivo final no es simplemente tener un APK.

Será una versión de Horizon que:

1. Se instale y ejecute en Android ARM64.
2. Renderice el juego correctamente.
3. Pueda crear y cargar mundos.
4. Permita jugar con controles táctiles equivalentes a PocketMC.
5. Tenga HUD e interfaz adaptados.
6. Soporte teclado, mouse y controles externos cuando sea posible.
7. Mantenga la arquitectura de escritorio.
8. Sea estable durante sesiones reales de juego.

**Mi recomendación:** que el primer objetivo sea conseguir el primer frame de Horizon en Android. No los controles, no el HUD y mucho menos los shaderpacks. Si resolvemos bien la integración gráfica y el ciclo de vida desde el principio, tendremos una base mucho más segura para todo lo demás.

Después de que Antigravity termine la auditoría, revisamos sus hallazgos y convertimos la siguiente fase en tareas concretas, sin dejar que modifique cosas que todavía no entendemos.
