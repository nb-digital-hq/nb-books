# Catálogo de Iconos

> **Estado:** Vigente. La fuente es `src/lib/ui/iconos.ts`; una prueba comprueba que este
> documento y el código no divergen.

**En NB Books no hay emojis.** Todo lo que parece un icono es un icono: trazo dibujado que hereda el color del
tema. El motivo está en [ui-conventions.md](ui-conventions.md), sección 1.

## El lenguaje visual

Quien sustituya estos trazos por otros dibujos tiene que respetar seis cosas. Son las que hacen que la barra no
parezca un collage:

| Propiedad | Valor |
|---|---|
| Rejilla | 24 por 24, `viewBox="0 0 24 24"` |
| Área segura | 20 por 20: nada se dibuja fuera de `2..22` |
| Grosor del trazo | 1,5, constante |
| Extremos y uniones | Redondeados |
| Relleno | Ninguno |
| Color | `currentColor`, nunca propio |

Un icono con relleno o con color propio deja de cambiar con el tema, y en alto contraste se queda como estaba.
Uno que se sale del área segura se ve más grande que sus vecinos y nadie sabe por qué; `iconos.test.ts` lo
comprueba sobre cada círculo, cada rectángulo y cada coordenada.

### Las medidas compartidas, que es de donde sale la coherencia

No basta con que cada icono esté bien dibujado: las familias tienen que medir lo mismo. Estas son las que usa el
catálogo, y las que hay que respetar al sustituir un trazo:

| Familia | Medida |
|---|---|
| Círculos de estado y de contenedor | `r = 8.5` centrados en `12,12` |
| Contenedores rectangulares | De `3.5` a `20.5`, con `rx = 2` |
| Renglones de texto | `x` de 7,5 a 16,5, separados 4,5 |
| Chevrones | 6,5 de recorrido |
| Flechas rectas | 15 de fuste |

## La lista completa

Setenta y seis iconos. Ninguno más, y ninguno menos: un icono que no esté aquí obliga a decidir antes qué
significa, y uno que sobra es un dibujo que nadie sabe dónde va. El de la paleta de comandos se retiró con ella.

**Dónde no se repiten, y por qué importa.** Varios conceptos son vecinos y se distinguen a propósito: `lista`
lleva viñetas y `indice`, sangría; `apariencia` parte el círculo por la mitad y `degradado` lo cruza con una
línea; `autor` es una silueta y `cuenta`, la misma silueta dentro de un círculo; `red` son tres arcos de señal y
`sinConexion`, los mismos tachados; `inmersion` es un marco con una sola línea dentro —solo el texto— y
`pantallaCompleta`, las cuatro esquinas de la ventana. Dos iconos que se parecen no son un problema de dibujo:
son dos significados que el usuario va a confundir.

### Destinos de la navegación primaria

| Nombre | Qué representa |
|---|---|
| `inicio` | La pantalla de arranque |
| `biblioteca` | Todo lo que el usuario tiene |
| `descubrir` | Catálogos remotos |
| `lector` | El documento abierto |
| `estudio` | Notas, anotaciones y vocabulario |
| `ajustes` | Preferencias |

### Carcasa

| Nombre | Qué representa |
|---|---|
| `menu` | Plegar y desplegar la barra lateral |
| `buscar` | Búsqueda |
| `cerrar` | Cerrar un panel, un documento o un diálogo |
| `atras` | Volver |
| `adelante` | Avanzar |
| `arriba` | Subir, plegar |
| `abajo` | Bajar, desplegar |
| `izquierda` | Anterior |
| `derecha` | Siguiente |
| `expandir` | Ocupar más |
| `contraer` | Ocupar menos |
| `mas` | Añadir |
| `comprobado` | Hecho, seleccionado |

### Biblioteca

| Nombre | Qué representa |
|---|---|
| `cuadricula` | Vista de portadas |
| `lista` | Vista de lista |
| `mosaico` | Vista de portadas pequeñas |
| `ordenar` | Criterio de orden, y el sentido de mayor a menor |
| `ordenarAscendente` | El sentido de menor a mayor |
| `filtrar` | Filtro activo |
| `estanteria` | Agrupación por regla |
| `autor` | Persona |
| `serie` | Secuencia de obras |
| `materia` | Tema o etiqueta |
| `duplicado` | Varias copias del mismo contenido |
| `noDisponible` | El archivo no está donde debería |
| `portadaAusente` | La edición no tiene portada |
| `progreso` | Cuánto se lleva leído |
| `importar` | Traer documentos |
| `exportar` | Llevarse datos |
| `carpeta` | Ubicación en disco |

### Lector

| Nombre | Qué representa |
|---|---|
| `indice` | Tabla de contenidos |
| `anotaciones` | Anotaciones del documento |
| `nota` | Escribir una nota |
| `resaltar` | Resaltar la selección |
| `marcador` | Marcar la posición |
| `subrayar` | Subrayar palabras sueltas |
| `trazoLibre` | Dibujar a mano alzada sobre la página |
| `tipografia` | Ajustes del plano del libro |
| `modoLectura` | Día, sepia, nocturno y los demás |
| `inmersion` | Retirar la carcasa |
| `pantallaCompleta` | Pantalla completa del sistema |
| `vistaDividida` | Dos documentos a la vez |
| `cambiarMitad` | Pasar el foco a la otra mitad |
| `extenderArriba` | Extender la selección hacia arriba |
| `extenderAbajo` | Extender la selección hacia abajo |
| `diccionario` | Consulta sobre la selección |
| `voz` | Lectura en voz alta |
| `reproducir` | Empezar |
| `pausa` | Detener |
| `fondoSonoro` | Ruido de fondo para concentrarse |

### Estudio

| Nombre | Qué representa |
|---|---|
| `vocabulario` | Palabras consultadas |
| `metas` | Objetivos del usuario |
| `plan` | Plan de lectura |
| `tablero` | Tablero de notas por columnas |
| `enlace` | Enlace entre notas |
| `retroenlace` | Quién apunta aquí |

### Estado

| Nombre | Qué representa |
|---|---|
| `correcto` | Todo bien |
| `advertencia` | Algo que conviene saber |
| `error` | Algo que ha fallado |
| `informacion` | Contexto adicional |
| `sinConexion` | Sin red, o red desactivada |
| `degradado` | Un componente funciona a medias |
| `cargando` | En curso |

### Acciones

| Nombre | Qué representa |
|---|---|
| `editar` | Modificar |
| `borrar` | Dar de baja |
| `copiar` | Duplicar al portapapeles |
| `reintentar` | Volver a intentarlo |

### Ajustes

| Nombre | Qué representa |
|---|---|
| `apariencia` | Tema y densidad |
| `accesibilidad` | Contraste, movimiento, escala |
| `idioma` | Idioma de la interfaz |
| `almacenamiento` | Dónde vive la biblioteca |
| `red` | Salida a internet |
| `privacidad` | Consentimiento y registro |
| `extensiones` | Complementos |
| `ia` | Modelos y proveedores |
| `cuenta` | Identidad y vinculación |

## Cómo se usan

```svelte
<Icon nombre="biblioteca" /> <!-- decorativo, junto a un texto -->
<Icon nombre="cerrar" etiqueta="Cerrar el Panel" /> <!-- solo, con nombre accesible -->
```

Un icono que acompaña a un texto **no se anuncia**: el lector de pantalla ya lee el texto, y oírlo dos veces es
ruido. Solo el icono que va solo lleva `etiqueta`.

---

## La marca

La marca de la aplicación **no** es un icono del catálogo. Es una silueta —dos hojas que se abren desde un eje
central— y vive en `static/logo.png`, montada por `src/lib/ui/Logo.svelte`.

**La silueta es la marca: no acompaña a un nombre escrito, lo sustituye.** No va con el nombre al lado: una marca tiene que verse para ser una marca, y a tamaño de icono junto a un texto no se
distinguía de una mancha.

| Dónde | Diámetro del disco | Por qué |
|---|---|---|
| Barra lateral desplegada | 10 rem | La barra da sitio, y ahí la marca se lee como marca y no como icono |
| Barra lateral plegada | 2,5 rem | Es lo que cabe en una barra de 56 px, y ahí es un icono, no una ilustración |

### Por qué la silueta no tiene color propio

La silueta es **una forma plana sobre nada**: fuera del trazo no hay fondo, es transparente. Puesta tal cual
sobre un fondo de su mismo tono, desaparece. Y no hay un tono que sirva para todos los acentos: entre el
`#00402f` del alto contraste claro y el `#7fe3c0` del oscuro, el que se lee sobre uno se pierde sobre el otro.

Por eso el archivo **no se monta como imagen sino como máscara de CSS**: se recorta el fondo del elemento con el
canal alfa del PNG, y el color lo pone `var(--nb-on-accent)` —el mismo par que ya usan `Button` y `Switch` para
escribir sobre el acento—. Eso resuelve tres cosas a la vez:

* **La marca no lleva color encima.** Hereda el que el tema garantiza que contrasta con el disco de debajo, en
  lugar de traer el suyo dentro de un binario, que es un color literal fuera de `app.css` y nadie puede revisar.
* **Se lee en los nueve pares.** Comprobado componiendo la silueta sobre los siete temas y los dos modos de alto
  contraste que define `app.css`, a 40, 64 y 160 px.
* **Cambia con el usuario.** El disco toma el acento vigente, así que la marca participa de la personalización
  en lugar de ser una estampa fija.

**Se descartó pintar la silueta siempre clara sobre el disco**, que es como venía el dibujo: se lee sobre los
cuatro acentos oscuros y desaparece sobre los cinco claros.

La silueta va al **62 %** del disco. Por debajo se queda pequeña dentro de su propio aro; por encima del 70 las
puntas alcanzan el borde y el disco deja de leerse como disco.

### Por qué un archivo y no un SVG en línea

Al revés que con los iconos. La silueta son curvas largas que en trazado pesarían más que el PNG a los tamaños
en que se usa, y cargarían el marcado en cada pintado. Y no necesita heredar `currentColor` desde dentro: como
máscara, el color lo pone la regla de CSS que la usa.

### Qué se le hizo al archivo, y por qué

La fuente es un vector, `docs/assets/branding/NB_Books.svg`: no hay halo que quitar ni alfa que escalar aparte,
porque no hay píxeles de partida. `static/logo.png` sale así:

1. **Se retira el disco** del SVG y se traza solo la silueta, a 2048 px.
2. **Se recorta al contenido y se cuadra.** La caja de la silueta es más ancha que alta (1253 por 1237 a esa
   escala), así que se recorta un cuadrado del lado mayor centrado en ella: sin esto queda descentrada dentro de
   su propio disco y el aro sale más grueso por un lado.
3. **Gris con alfa** en lugar de color: como se monta de máscara, del archivo solo se usa el alfa, y dos canales
   bastan donde habría cuatro.
4. **512x512**, que cubre su tamaño real —160 px— en una pantalla de tres veces la densidad.

Resultado: 18 KB. No se monta el SVG directamente como máscara porque el archivo lleva el disco dentro, y una
segunda copia del trazado sin él sería una fuente más que mantener a mano.

### El icono del sistema operativo

Es el mismo dibujo, y no el mismo archivo: el que ve el gestor de ventanas no puede ser una máscara ni heredar
nada, porque fuera de la aplicación no hay tema. Lleva **los colores del propio SVG** —disco
`#FEB61A`, silueta `#FEF9F2`— y no los de `gabinete`: el icono es uno de un juego común a las aplicaciones de
NB Digital, y fuera de la ventana manda ese juego. De ahí salen los tamaños más el `.ico` y el `.icns`:

```bash
rsvg-convert -w 1024 -h 1024 docs/assets/branding/NB_Books.svg -o docs/assets/branding/app-icon.png
pnpm tauri icon docs/assets/branding/app-icon.png
```

El maestro vive en `docs/assets/branding/app-icon.png` y es el SVG trazado sin retocar. Dentro de la ventana no cambia
nada: el disco sigue tomando el acento del tema, por lo dicho arriba.

### El icono de la pestaña

Es la misma composición, a 128 px, en `static/favicon.png`. No es un glifo del catálogo: el sitio donde va un
logotipo lleva el logotipo, no un icono parecido: la marca es una sola en todas partes. Y como la silueta suelta
es casi blanca, aquí tampoco puede ir sin su disco, o desaparece en una pestaña clara.

Sin este archivo, todo navegador pide `/favicon.ico` y recibe un 404 en cada carga, que es exactamente el tipo de
error de consola que `e2e/flujo.spec.ts` prohíbe en su `afterEach`.
