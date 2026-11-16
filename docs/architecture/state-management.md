# Gestión de Estado (State Management)

> **Estado:** Implementado. Preferencias y caché de arranque en `src/lib/core/settings.ts`; relectura por
> anuncios del runtime en `src/lib/core/vivo.ts`.
> Reglas para el estado del frontend Svelte 5 y su relación con la fuente de verdad en Rust.

## 1. Principio rector

**SQLite es la fuente de verdad. El estado del frontend es una proyección.**

Toda información que deba sobrevivir a un reinicio se persiste a través del runtime. Un valor que solo existe en
un rune de Svelte y no tiene respaldo es estado huérfano y se considera un defecto.

## 2. Clasificación del estado

| Clase | Ejemplos | Dónde vive | Persistencia |
|---|---|---|---|
| Estado de dominio | Documentos, colecciones, anotaciones, progreso | Rust + SQLite | Duradera |
| Estado de sesión | Ventanas abiertas, posición y tamaño, pila z | Store del frontend | Se restaura al arrancar |
| Estado efímero de UI | Menú abierto, hover, texto de un campo sin confirmar | Componente | No se persiste |
| Preferencias | Tema, escala tipográfica, proveedor de IA | SQLite | Duradera |
| Secretos | Tokens, claves de API | Almacén del sistema operativo | Fuera de SQLite |
| Caché derivada | Portadas, páginas renderizadas, índices | Directorio de caché | Regenerable |

> **Decidido en [ADR-010](../adr/010-settings-persistence.md).** SQLite es la fuente de
> verdad. El almacenamiento del webview conserva una caché de solo lectura limitada a los cinco valores que
> afectan al primer pintado, para que quien usa alto contraste, movimiento reducido o tipografía aumentada no
> vea un destello con los valores por defecto. Ante discrepancia gana SQLite.
>
> El prototipo archivado guardaba **todas** las preferencias solo en el webview, lo que las hacía frágiles, no
> respaldables y no sincronizables. Ese es el error que la decisión corrige.

## 3. Reglas para el frontend (Svelte 5 Runes)

1. El estado global vive en stores dedicados (`workspace`, `catalog`, `settings`, `theme`), no disperso en
   componentes.
2. Los componentes reciben solo lo que necesitan; se evita pasar objetos grandes que provoquen invalidaciones en
   cascada.
3. Los efectos (`$effect`) se usan para sincronizar con el mundo exterior, no para derivar datos. Los datos
   derivados usan `$derived`.
4. Ningún componente invoca IPC directamente: lo hace a través de la capa `$core/ipc`, que centraliza tipado, manejo
   de errores y registro.
5. Las listas grandes se virtualizan; el store guarda la página actual, no la colección completa.

## 4. Sincronización entre Rust y el frontend

El patrón es unidireccional:

```
UI -> comando IPC -> Rust muta SQLite -> Rust emite evento -> el store del frontend se actualiza -> la UI se re-renderiza
```

El frontend **no** actualiza su store de forma optimista y luego confirma, salvo en interacciones donde la
latencia sea perceptible; en esos casos debe implementar reversión explícita ante error. Nunca debe quedar un
estado visual que no corresponde con la base de datos.

Sin la última flecha, cada sitio que escribe tendría que releer lo que cree que depende de él, y lo que olvidara
se quedaría viejo hasta reiniciar o cerrar el lector. Por eso el runtime anuncia cada escritura -ver "Quien escribe, lo anuncia" en
[events.md](api/events.md)- y `src/lib/core/vivo.ts` traduce esos anuncios en qué releer. Dos reglas que salen
de ahí:

1. **Lo que se ve se relee en el momento; lo que no se ve, al volver a verse.** Inicio no se relee cada cinco
   segundos mientras alguien lee en el lector: se marca como desfasado y se relee al entrar, con la misma semilla,
   para no barajarlo.
2. **Una vista que se desmonta no se lleva el dato.** El lector sigue abierto al ir a otro destino, pero su vista se
   desmonta; la carcasa guarda por dónde va cada documento abierto, y al volver empieza ahí y no donde estaba el
   libro al abrirlo. Al salir del lector se escribe la última posición apuntada (`flush_progress`), para que la
   biblioteca diga la misma cifra que el lector.

## 5. Estado de salud de los componentes

Los estados de la máquina definida en [component-model.md](component-model.md) se propagan al frontend por el
Event Bus y viven en un store propio. La interfaz consulta ese store para decidir qué funciones muestra. Este es
el mecanismo concreto que hace observable la degradación elegante, en lugar de dejarla como una intención.

## 6. Restauración de sesión

Al arrancar, la aplicación restaura la vista activa, los documentos abiertos y la disposición de paneles. No
restaura posiciones ni geometrías de ventana: por [ADR-012](../adr/012-ui-paradigm.md) la
ventana es la del sistema operativo, y su geometría la gestiona el sistema.

La restauración debe ser defensiva: una entrada guardada que apunta a un documento eliminado, a un panel que ya
no existe porque su extensión se desinstaló, o a una clase de ancho que no corresponde a la actual, se descarta
en silencio. **Nada de lo restaurado puede impedir el arranque.**
