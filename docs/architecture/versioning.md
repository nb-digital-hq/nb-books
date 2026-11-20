# Versionado y Compatibilidad

> **Estado:** Vigente.
> NB Books declara que se diseña para durar diez años. Esa promesa se cumple o se incumple aquí.

El proyecto mantiene **cinco contratos versionados de forma independiente**. Confundirlos es la causa habitual de
roturas silenciosas.

| Contrato | Consumidor | Regla |
|---|---|---|
| Versión de la aplicación | Usuario final | SemVer |
| Esquema de base de datos | La instalación del propio usuario | Entero incremental, solo hacia adelante |
| Contrato IPC | El frontend de la misma versión | Puede cambiar entre versiones; se distribuye acoplado |
| API de extensiones | Terceros | SemVer con ventana de soporte declarada |
| Formato de exportación e intercambio | Usuario y otras herramientas | SemVer; compatibilidad hacia atrás prioritaria. Versión 1, especificado en [api/export-format.md](api/export-format.md) |

---

## 1. Esquema de base de datos

* Versión entera incremental. Cada migración sube exactamente un número.
* Solo hacia adelante. No hay scripts de reversión (ver
  [migrations.md](migrations.md)).
* Una versión antigua de la aplicación que encuentra una base de datos más nueva **debe negarse a operar** y
  explicarlo, en lugar de intentar leerla.
* Antes de una migración no trivial se crea una copia de seguridad automática.

### Reglas de evolución de tablas
* Añadir columnas con valor por defecto: permitido.
* Renombrar columnas en sitio: prohibido. Se añade la nueva, se migran los datos y la antigua se marca obsoleta.
* Eliminar columnas: solo tras al menos una versión menor de deprecación registrada.

---

## 2. API de extensiones

> **Este es el compromiso de estabilidad, y este documento es el único sitio donde viven sus números.**
> [api/plugins.md](api/plugins.md) es la cara del contrato para quien escribe una extensión y enlaza aquí;
> no repite las cifras, porque dos copias de un número son una contradicción esperando a que alguien cambie una
> sola.

Cada módulo declara la versión de API contra la que fue compilado; el host la lee del manifiesto y de la sección
personalizada del módulo WebAssembly, y rechaza lo que esté fuera del rango soportado.

### 2.1 La API se versiona aparte de la aplicación

La versión de la aplicación y la versión de la API de extensiones son **dos números distintos** y no se mueven
juntos. Una versión menor de la aplicación puede no tocar la API, y una mayor de la API puede llegar sin que la
aplicación cambie de mayor.

Se hace así porque atarlas obliga a elegir entre dos daños: o la aplicación no puede romper nunca nada suyo, o
las extensiones se rompen cada vez que la aplicación evoluciona por motivos que no tienen que ver con ellas.

### 2.2 Estabilidad de la API 1.0.0

La API de extensiones es **1.0.0 y estable**. Cada clase de punto de contribución tiene una extensión de primera
parte en `extensiones/propias/` (metadatos, metadatos por lotes, citas, temas, modos de lectura, proveedor de
formato, órdenes y vistas), el paquete del mundo WIT se declara en `nb:extension@1.0.0`, el anfitrión fija su
versión mayor en `MAYOR_ACTUAL`, Ajustes lo lee del anfitrión con `extensions_api` y lo muestra como estable, y los
componentes de WebAssembly declaran su versión de API en el binario para que la revisión de R11 la contraste
automáticamente.

La regla que la sostiene: **no se promete estabilidad sobre una superficie que no se ha usado**. Por eso una clase
de punto de contribución nueva no entra en el contrato estable sin una extensión propia que la use.

### 2.3 Cuántas versiones mayores se soportan a la vez

**Dos: la actual y la anterior.** El anfitrión acepta un módulo cuya mayor declarada sea `N` o `N-1`, y rechaza
todo lo demás.

Una sola significaría que el día que la API sube de mayor se rompen a la vez todas las extensiones instaladas.
Tres significaría mantener tres implementaciones del anfitrión vivas, y sobre una superficie tan corta como la
de la sección 8 de [ui-components.md](ui-components.md) eso no es prudencia, es una promesa que no se podría
cumplir con el tamaño real del proyecto.

### 2.4 Cuánto dura el soporte tras una deprecación

**Doce meses desde el anuncio, y al menos dos versiones menores de la aplicación.** Se cumplen las dos
condiciones, y manda la que termine más tarde.

Son dos y no una porque cada una tapa un agujero de la otra. Si el proyecto publica deprisa, "dos versiones
menores" pueden ser tres semanas. Si publica despacio, los doce meses pueden agotarse sin que el autor haya
visto **ninguna** versión que lleve el aviso. Juntas garantizan las dos cosas que el autor necesita: tiempo de
calendario, y versiones publicadas que le avisen.

### 2.5 Qué cuenta como cambio mayor

Una ventana de soporte no significa nada si no está escrito qué la dispara.

| Cambio | Clase |
|---|---|
| Quitar o renombrar una función del mundo WIT, un campo obligatorio del manifiesto o un punto de contribución | Mayor |
| Cambiar el tipo de un parámetro o del valor devuelto | Mayor |
| **Cambiar lo que una función hace, sin tocar su firma** | **Mayor** |
| **Endurecer una restricción: rechazar lo que antes se aceptaba** | **Mayor** |
| Añadir una función, un campo opcional o un punto de contribución nuevo | Menor |
| Relajar una restricción: aceptar lo que antes se rechazaba | Menor |
| Corregir un fallo por el que una función no hacía lo que su documentación decía | Parche |

La fila de endurecer existe porque es la mitad que rompe extensiones. El primer caso es la
firma del autor (R13 de la [política del registro](../legal/governance/extension-policy.md)): un paquete cuya firma
no corresponde deja de instalarse, y una actualización firmada por otra clave tampoco entra. **No afecta a las
extensiones ya construidas** —el mundo WIT no cambia y una extensión instalada sigue ejecutándose igual—, así
que no consume ventana de soporte; lo que cambia es qué se distribuye y qué se acepta como actualización, y eso
se anuncia por la política, no por la versión de la API.

La tercera fila es la que se incumple con más facilidad y la que más daño hace: el autor **no puede verla
compilando**. Su extensión sigue construyéndose, se instala, y hace otra cosa. Un cambio de conducta sin cambio
de firma es un cambio mayor aunque no lo parezca.

### 2.6 Cómo se avisa de una deprecación

En tres sitios, y el tercero es el que casi nadie hace:

1. **En las notas de la versión**, con el motivo y la alternativa, como manda la sección 5.
2. **En el registro**, marcando las extensiones que usan lo deprecado. La revisión automatizada lo
   comprueba en cada publicación, así que el autor se entera al publicar y no al romperse.
3. **En la aplicación, a quien tenga instalada una extensión afectada.** Por el mismo canal por el que llega el
   aviso de retirada.

El tercero existe porque el autor puede haber abandonado la extensión, y en ese caso el único al que le importa
—y el único que puede hacer algo, aunque sea buscar otra— es quien la tiene instalada.

### 2.7 Qué ve el autor de un módulo que el anfitrión no admite

No "extensión incompatible". El mensaje dice **las tres cosas** que hacen falta para arreglarlo: qué versión de
API declara el módulo, qué rango admite este anfitrión, y contra qué versión hay que recompilarlo.

Quien lo ve en la aplicación es el usuario, y a ese se le dice otra cosa: qué extensión es, que no depende de
él, y si hay una versión suya que sí funcione.

### 2.8 Qué pasa con una extensión cuya API ya no se soporta

**No se carga, y no se toca nada más.** No se desinstala sola, no se borran sus datos ni su configuración, y
sigue apareciendo en la lista con su estado y su motivo.

Es la regla del producto aplicada aquí: **ninguna pérdida**. Que una ventana de soporte se agote es un motivo
para dejar de ejecutar código, nunca para destruir lo que el usuario tenía. Si vuelve a instalar una versión
anterior de la aplicación, o si el autor publica una actualización, todo sigue donde estaba.

### 2.9 Qué no se promete

Tan importante como lo anterior, y más corto:

* **El contrato IPC interno no es API de extensiones.** Cambia entre versiones y se distribuye acoplado al
  frontend de la misma versión, como dice la tabla de arriba.
* **El esquema de la base de datos tampoco.** Es de la instalación del usuario, no de terceros.
* **El aspecto y el orden de las ranuras de interfaz no se prometen.** Lo que se promete es que la ranura
  existe y que lo que se monta en ella se monta; no dónde queda ni cómo se ve, que dependen de la clase de
  ancho por [ADR-012](../adr/012-ui-paradigm.md).

### 2.10 Qué hace cumplir todo esto

**Mecanismo.** El anfitrión lee `nb:api-version` de los bytes del
módulo antes de compilar nada, admite la mayor actual y la anterior, y rechaza el resto devolviendo los tres
números con los que su autor lo arregla. La regla de las dos mayores **está escrita en el código**, con una
prueba que falla si alguien la ensancha o la estrecha, así que cambiarla obliga a pasar por aquí.

**El resto sigue dependiendo de la disciplina de quien programa**, y decirlo es parte del compromiso: la ventana
de doce meses, la tabla de qué cuenta como cambio mayor y los tres avisos los sostiene la revisión de cada cambio.
Cambiar la conducta de una función sin tocar su firma es precisamente lo que ninguna herramienta puede ver.
`scripts/check-capacidades.mjs` comprueba además que ninguna función del mundo WIT se ofrezca sin un permiso detrás.

---

## 3. Eventos del Event Bus

Los eventos son un contrato entre servicios y con el frontend.

* Los payloads solo pueden crecer con campos opcionales.
* Renombrar un evento o cambiar el tipo de un campo requiere un evento nuevo y un periodo de convivencia.
* Cada evento declara su versión de esquema; los consumidores deben ignorar campos desconocidos.

---

## 4. Formato de exportación

Es el contrato más importante para la longevidad, porque es lo que permite al usuario irse.

* Formato abierto, documentado y legible sin NB Books.
* Debe incluir documentos referenciados o sus hashes, metadatos, colecciones, anotaciones con sus locators,
  progreso y preferencias.
* La importación debe aceptar todas las versiones anteriores del formato. Romper esto equivale a destruir los
  respaldos de los usuarios.

---

## 5. Política de deprecación

1. Se anuncia en las notas de release, con motivo y alternativa.
2. Se mantiene funcionando durante al menos una versión menor, emitiendo advertencia.
3. Se elimina en una versión mayor.

Ninguna función que el usuario esté usando desaparece sin aviso previo ni sin ruta de migración. Esto es la
aplicación directa del principio "evolucionar sin romper".
