# Modelo de Componentes

> **Estado:** Implementado en `src-tauri/src/monitor.rs`, con propagación al webview por el bus de eventos.
>
> **Primer caso real de degradación**: el componente `reader` arranca en `Ready` si el motor nativo de
> PDF está, y en `Degraded` con motivo `PDF_ENGINE_MISSING` si no. Degradado y no caído, porque sin ese motor se
> siguen leyendo EPUB: lo que se pierde es una capacidad, no el componente. La interfaz oculta el PDF y ofrece el
> resto.

En NB Books, ninguna dependencia crítica se asume como siempre disponible. El diseño se basa en **capacidades** y
**contratos estables**.

## 0. La metáfora que gobierna: la aplicación es un ordenador

Un ordenador no es un bloque: son piezas que trabajan solas y en equipo. Si quitas un módulo de memoria, no se
apaga: arranca con menos. Si quitas **toda** la memoria, no da vídeo. Si le falta el procesador, no hay
ordenador. Si le fallan los ventiladores, funciona y se degrada hasta pararse. **Todas las piezas importan, y no
todas importan igual**, y confundir esas dos frases produce los dos errores contrarios: tratar lo accesorio como
crítico —y no arrancar por falta de un diccionario— o tratar lo crítico como accesorio, y perder datos por creer
que algo podía fallar en silencio.

De ahí sale la clasificación que este documento necesitaba y no tenía. **La regla 1 de la Constitución y el
innegociable de que el fallo de un componente no tumbe a otro siguen valiendo enteros**: lo que esta sección
añade es que "no tumbar a otro" no significa "todos son opcionales".

| Clase | Qué significa | Componentes | Qué se hace si falta |
|---|---|---|---|
| **Vital** | Sin él **no hay aplicación**. Su ausencia no se degrada: se declara y se para | `database`; y los archivos de la biblioteca, que no son un componente pero son el sustrato | Modo de recuperación: la base dañada se aparta con marca de tiempo, se arranca vacío, y se dice qué pasó y dónde quedó lo suyo. **Nunca un `expect` que mate el proceso antes de abrir ventana** |
| **Estructural** | La aplicación arranca sin él, pero **pierde su razón de ser** | `reader` | Arranca `Degraded` con motivo. Sin motor de PDF se leen EPUB; sin ningún proveedor, no hay producto que ofrecer y hay que decirlo |
| **De servicio** | Amplía lo que se puede hacer. Su fallo **quita una capacidad, nunca un dato** | `search`, `metadata`, voz, consulta, red | Se oculta lo que depende de él (estados degradados). El índice de búsqueda es **caché**: se puede borrar entero y reconstruir, y por eso un índice corrupto no impide arrancar |
| **Ajeno** | Opcional, y puede no instalarse nunca | `ai` | Se publica `STOPPED`: `seOfrece` lo da por no utilizable, así que la interfaz oculta sus funciones, y `avisos` lo excluye, así que no pide atención. No se anuncia hasta que hay algo detrás. Lo que no está se oculta; lo que no tiene arreglo ni se anuncia |

**La prueba de que la clasificación es real y no una tabla bonita** es que cada fila tiene un escenario en
[chaos.md](../developer/testing/chaos.md): la vital, los escenarios 3 y 10; la estructural, el 1 y el 7; las de servicio,
el 6 y el 9; la ajena, el 8, junto con el 8b y el 8c.

El estado de un componente **se calcula**, no se escribe a mano: el de `ai` lo calcula
`ai::AiService::component_state`, y el simulador de los recorridos de interfaz sirve todos los componentes que
publica el runtime, porque un simulador que omite la dimensión medida no puede ver el defecto.

**Y en pantalla se dice el motivo, no solo el estado.** Acerca de muestra el motivo canónico cuando hay palabras
para él: `AI_DISABLED` es "Sin activar", y `AI_MEMORY_LIMIT` es un modelo instalado que no cabe en el tope de memoria que eligió el usuario. Es
`STOPPED` y no pide atención, porque es una decisión suya, y se ve en Ajustes junto al tope que lo produce.

**Consecuencia operativa, y es la que se olvida:** una capacidad de clase "de servicio" que falla **tiene que
verse**. Un componente que se degrada en silencio es indistinguible de uno que funciona mal, y quien lo sufre
concluye que la aplicación está rota. Para eso existe el avisador de la barra lateral.

---

## 1. Abstracción por contratos

Los componentes dependen de interfaces (traits en Rust), no de implementaciones concretas. Esto protege a la
aplicación frente a cambios en bibliotecas externas, ausencia de binarios y caídas de red.

**Antipatrón prohibido:**
`Reader Component -> instancia concreta del motor de inferencia -> fallo si el motor no existe`

**Forma correcta:**
`Reader Component -> AIServiceContract`

El contrato resuelve internamente si usa inferencia local, un adaptador externo o devuelve un estado de no
disponibilidad.

## 2. Máquina de estados de salud

Cada componente administrado por el runtime reporta su estado al monitor del sistema:

| Estado | Significado |
|---|---|
| `NOT_INSTALLED` | Requiere binarios o configuración ausentes |
| `NOT_SUPPORTED` | El hardware o el sistema no permiten ejecutarlo |
| `STARTING` | Inicializando |
| `READY` | Funcionamiento óptimo |
| `DEGRADED` | Funciona con capacidades limitadas |
| `FAILED` | Fallo crítico aislado; el resto del sistema continúa |
| `STOPPED` | Detenido deliberadamente |

La distinción entre `NOT_INSTALLED` y `NOT_SUPPORTED` importa para la interfaz: el primero invita a instalar, el
segundo debe ocultar la función sin generar frustración.

**Qué es `NOT_SUPPORTED`.** Solo lo que **de verdad no se puede ejecutar**: el motor no
arranca, la plataforma no trae la pieza. **No** lo que se ejecutaría despacio, con peor calidad o con riesgo.
Recomendar no es obligar: cuando la aplicación mide el equipo y de ahí saca qué funciona mejor, **ofrece todas las
opciones**, marca una como recomendada con el motivo y dice en las demás lo que es probable que pase con los datos
medidos. Así un equipo sin AVX2 compilado, uno justo de memoria o uno lento para narrar en vivo siguen pudiendo
elegir. Lo que probablemente cierre la aplicación -un modelo que pide más memoria
de la que tiene el equipo- también se ofrece, con una confirmación que lo dice claro antes de hacerlo.

## 3. Degradación elegante

La interfaz consulta estos estados en tiempo real. Ante `FAILED` en el servicio de IA, la aplicación debe:

1. Ocultar las acciones que dependen de él.
2. Mostrar un indicador discreto en configuración señalando la indisponibilidad.
3. Permitir seguir leyendo sin interrupción.

Lo que **no** debe ocurrir bajo ninguna circunstancia: una pantalla en blanco, un error sin explicación, o un
botón que falla al pulsarlo.

## 4. Verificación

La degradación elegante no es una intención de diseño, es una propiedad verificable. Cada componente con estado
debe tener un escenario correspondiente en [testing/chaos.md](../developer/testing/chaos.md). Un componente sin escenario
de caos asociado no puede declararse resiliente.

## 5. Propagación a la interfaz

Los estados viajan por el bus de eventos hasta un store dedicado del frontend, del que dependen las decisiones de
visibilidad de la interfaz. Ver [state-management.md](state-management.md).
