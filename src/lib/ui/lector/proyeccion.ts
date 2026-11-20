/**
 * Proyección: del sistema de coordenadas del runtime a una posición del DOM.
 *
 * # Qué problema resuelve
 *
 * Una anotación se guarda con un `Locator`, y el runtime la resuelve a una
 * `NativePosition`. Para un EPUB eso es `{href, offset}`, y el desplazamiento
 * está contado **sobre el texto plano que produce `reader/epub.rs`**, no sobre
 * el que compone el navegador. Los dos no coinciden: el del runtime inserta un
 * salto al cerrar los bloques y después colapsa todo el espacio en blanco, y el
 * `textContent` del DOM trae la indentación del archivo tal cual.
 *
 * Este módulo reconstruye ese mismo texto recorriendo el árbol ya compuesto, y
 * se queda con **de dónde salió cada carácter**. Con eso, "carácter N del
 * capítulo" se convierte en "esta posición de este nodo", que es lo que hace
 * falta para pintar.
 *
 * # Por qué esto no es una segunda resolución de anclaje
 *
 * La pregunta G1 avisaba de que emparejar por la cita en la interfaz crearía una
 * segunda verdad compitiendo con la del runtime, y que dos fuentes para el mismo
 * dato acaban discrepando justo donde la primera dudaba. El aviso sigue siendo
 * correcto y esto no lo incumple: **quién decide a qué texto se refiere una
 * anotación sigue siendo el runtime**. Aquí solo se traduce una coordenada que
 * ya viene decidida. Es una proyección determinista, no una decisión.
 *
 * La regla que lo mantiene así: si alguna vez hiciera falta **buscar** la cita
 * para encontrar el sitio, esto se para y el trabajo se lleva al runtime.
 *
 * # Por qué la parte pura va separada de la parte del DOM
 *
 * Por lo mismo que la narración y el panel de anotaciones: las pruebas de esta
 * capa corren sin navegador. `normalizar` recibe una secuencia de eventos y no
 * un árbol, así que se comprueba con una lista escrita a mano; el recorrido del
 * árbol es la parte que necesita un navegador de verdad, y esa la cubren los
 * recorridos de `e2e/`.
 *
 * Contrato: `docs/architecture/reader-contracts.md`.
 */

import type { Locator, OutlineEntry } from '$lib/bindings';
import { desdePayload, type MarcaDeTinta } from './trazos';

/**
 * Elementos que separan al cerrarse.
 *
 * Es la lista de `texto_plano` en `src-tauri/lector/src/reader/epub.rs`, **menos
 * `br`**, y la ausencia es deliberada: el runtime analiza el XHTML con un lector
 * de eventos donde `<br/>` es un elemento vacío y no produce cierre, así que no
 * separa. Como en un EPUB real un salto se escribe siempre `<br/>`, incluirlo
 * aquí desplazaría un carácter todo lo que viniera detrás.
 *
 * Los seis niveles de encabezado y `li` sí van: sus etiquetas se cierran.
 */
const SEPARAN = new Set(['P', 'DIV', 'LI', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6']);

/** Espacio en blanco según Unicode, que es el criterio de `char::is_whitespace`. */
const ESPACIO = /\p{White_Space}/u;

/** Lo que la parte pura recibe: texto que aparece, o un bloque que se cierra. */
export type Evento = { tipo: 'texto'; texto: string; ref: number } | { tipo: 'finDeBloque' };

/**
 * El texto normalizado con el origen de cada uno de sus caracteres.
 *
 * Los índices son **puntos de código**, no unidades de UTF-16, porque el runtime
 * cuenta `char` de Rust. Un libro con emoji o con caracteres fuera del plano
 * básico desplazaría todas las marcas si se contara de la otra forma.
 */
export interface TextoProyectado {
  /** El mismo texto que el runtime usa como sistema de coordenadas. */
  texto: string;
  /** Cuántos puntos de código tiene. */
  largo: number;
  /** Por cada punto de código, de qué origen salió. */
  refs: number[];
  /** Y en qué desplazamiento de ese origen empieza, en unidades de UTF-16. */
  desde: number[];
  /** Cuántas unidades de UTF-16 ocupa ahí. Cero para un espacio de relleno. */
  ancho: number[];
}

/**
 * Reconstruye el texto del runtime a partir de los eventos del árbol.
 *
 * Reglas, y son las de `texto_plano`: cerrar un bloque separa, y después todo
 * el espacio en blanco se colapsa a uno solo, sin dejar ninguno al principio ni
 * al final. Aquí se hace en una sola pasada en lugar de en dos, porque hay que
 * llevar la cuenta del origen: el espacio de separación no se emite cuando se
 * ve, sino cuando llega el carácter que lo necesita, y por eso nunca sobra uno
 * al final.
 */
export function normalizar(eventos: Iterable<Evento>): TextoProyectado {
  let texto = '';
  const refs: number[] = [];
  const desde: number[] = [];
  const ancho: number[] = [];
  let pendiente = false;

  for (const evento of eventos) {
    if (evento.tipo === 'finDeBloque') {
      pendiente = true;
      continue;
    }

    let posicion = 0;
    for (const caracter of evento.texto) {
      if (ESPACIO.test(caracter)) {
        pendiente = true;
      } else {
        // El espacio de relleno se apunta al carácter que lo sigue y con ancho
        // cero: no es de nadie, así que no puede prestar sitio a nadie.
        if (pendiente && texto.length > 0) {
          texto += ' ';
          refs.push(evento.ref);
          desde.push(posicion);
          ancho.push(0);
        }
        pendiente = false;
        texto += caracter;
        refs.push(evento.ref);
        desde.push(posicion);
        ancho.push(caracter.length);
      }
      posicion += caracter.length;
    }
  }

  return { texto, largo: refs.length, refs, desde, ancho };
}

/** Los puntos de código de `desde` a `desde + largo`, o `null` si no caben. */
export function textoEntre(
  proyectado: TextoProyectado,
  inicio: number,
  largo: number
): string | null {
  if (inicio < 0 || largo < 0 || inicio + largo > proyectado.largo) return null;
  // Se recorta por puntos de código y no con `slice`, que corta por unidades de
  // UTF-16 y partiría un par sustituto por la mitad.
  return [...proyectado.texto].slice(inicio, inicio + largo).join('');
}

/**
 * Una marca proyectada sobre el texto: qué es, de dónde a dónde, y de quién.
 */
export interface Marca {
  /** Identificador de la anotación, para poder abrirla al señalarla. */
  id: string;
  /** Qué clase de marca, que decide cómo se pinta. */
  clase: string;
  /** En puntos de código del texto normalizado. */
  inicio: number;
  fin: number;
}

/** Lo que hace falta saber de una anotación para colocarla. */
export interface Anotacion {
  id: string;
  clase: string;
  /** Desplazamiento que resolvió el runtime, en puntos de código. */
  inicio: number;
  /** La cita, que es con lo que se comprueba que el sitio es el suyo. */
  cita: string;
}

/** Cuáles se pudieron colocar y cuáles no, que no es lo mismo que perderlas. */
export interface Colocacion {
  marcas: Marca[];
  /** Identificadores que no se pudieron colocar en el texto compuesto. */
  sinColocar: string[];
}

/**
 * Coloca las anotaciones sobre el texto proyectado, comprobando cada una.
 *
 * **Nada se pinta sin comprobarlo.** La reconstrucción del texto es exacta salvo
 * en un caso conocido —el saneado descarta el contenido de `iframe`, `object`,
 * `embed` y `template`, y el texto plano del runtime no lo descarta—, y en un
 * capítulo así todos los desplazamientos posteriores irían corridos. Comparar el
 * tramo con la cita que la propia anotación lleva cuesta una comparación de
 * cadenas y convierte un fallo silencioso —una marca sobre el párrafo
 * equivocado, que es peor que ninguna marca— en una que se declara.
 *
 * Lo que no se puede colocar sale aparte y no se descarta, que es lo que
 * el reanclaje ya hace con las anotaciones.
 */
export function colocar(
  proyectado: TextoProyectado,
  anotaciones: readonly Anotacion[]
): Colocacion {
  const marcas: Marca[] = [];
  const sinColocar: string[] = [];

  for (const anotacion of anotaciones) {
    const largo = [...anotacion.cita].length;
    // Una anotación sin cita no se puede comprobar, así que no se pinta: un
    // resaltado sin cita ya se rechaza al crearlo, y un marcador no
    // tiene tramo que pintar.
    if (largo === 0 || textoEntre(proyectado, anotacion.inicio, largo) !== anotacion.cita) {
      sinColocar.push(anotacion.id);
      continue;
    }
    marcas.push({
      id: anotacion.id,
      clase: anotacion.clase,
      inicio: anotacion.inicio,
      fin: anotacion.inicio + largo
    });
  }

  return { marcas, sinColocar };
}

/**
 * Una marca que es un **punto** y no un tramo.
 *
 * Un resaltado cubre palabras; un marcador y el separador señalan un sitio. En
 * papel la diferencia es la misma: el subrayado está sobre el texto y la cinta
 * asoma por el canto. Aquí se resuelven distinto por eso: un tramo se pinta como
 * rango, y un punto marca **el bloque** que lo contiene, que es lo que se puede
 * ver de lejos sin leer.
 */
export interface Punto {
  id: string;
  /** `marcador` es del usuario; `separador`, por dónde iba al abrir. */
  clase: 'marcador' | 'separador';
  /** En puntos de código del texto normalizado. */
  inicio: number;
}

/**
 * Los marcadores de **este** capítulo.
 *
 * Son la otra mitad de lo que `anotacionesDelCapitulo` deja fuera a propósito:
 * allí se descartan porque no tienen tramo que pintar, y aquí se recogen porque
 * sí tienen sitio que señalar. Poner un marcador y no ver nada era el ejemplo
 * más claro de que la aplicación guardaba sin enseñar.
 */
export function marcadoresDelCapitulo(entradas: readonly OutlineEntry[], href: string): Punto[] {
  const salida: Punto[] = [];
  for (const entrada of entradas) {
    const posicion = entrada.position;
    if (entrada.lost || !posicion || posicion.kind !== 'fragment') continue;
    if (posicion.value.href !== href) continue;
    if (entrada.annotation.kind !== 'bookmark') continue;
    salida.push({ id: entrada.annotation.id, clase: 'marcador', inicio: posicion.value.offset });
  }
  return salida;
}

/**
 * Los trazos libres de un capítulo, con su geometría ya interpretada.
 *
 * Se anclan como los marcadores —por el bloque que los contiene— porque la tinta
 * no rodea caracteres: rodea una zona, y a veces el margen, que no tiene ninguno.
 *
 * Una geometría que no se reconoce **se descarta en lugar de aproximarse**, que
 * es lo mismo que se hace con una marca cuyo tramo no cuadra con su cita: pintar
 * una forma parecida pondría tinta donde nadie la puso.
 */
export function tintaDelCapitulo(entradas: readonly OutlineEntry[], href: string): MarcaDeTinta[] {
  const salida: MarcaDeTinta[] = [];
  for (const entrada of entradas) {
    const posicion = entrada.position;
    if (entrada.lost || !posicion || posicion.kind !== 'fragment') continue;
    if (posicion.value.href !== href) continue;
    if (entrada.annotation.kind !== 'drawing') continue;

    const crudo = entrada.annotation.payload;
    if (!crudo) continue;
    let interpretado: unknown;
    try {
      interpretado = JSON.parse(crudo);
    } catch {
      continue;
    }
    const tinta = desdePayload(interpretado);
    if (tinta) salida.push({ id: entrada.annotation.id, inicio: posicion.value.offset, tinta });
  }
  return salida;
}

/**
 * El desplazamiento que un `Locator` señala en un capítulo, si es el suyo.
 *
 * El selector de carácter viaja en los fragmentos, con la forma `#char=N` que
 * fija ADR-004 y que produce el proveedor de EPUB. Se lee el fragmento y no se
 * deduce de la progresión: una fracción del documento entero no dice en qué
 * carácter de este capítulo cae, y redondearla pondría la cinta en otro párrafo.
 *
 * Devuelve `null` cuando el locator es de otro capítulo o cuando no ancla por
 * carácter —un PDF ancla por página—, que son los dos casos en los que no hay
 * nada que señalar aquí.
 */
export function desplazamientoDe(locator: Locator, href: string): number | null {
  if (locator.href !== href) return null;
  for (const fragmento of locator.locations.fragments ?? []) {
    const encontrado = /^#char=(\d+)$/.exec(fragmento);
    if (encontrado) return Number(encontrado[1]);
  }
  return null;
}

/**
 * El bloque que contiene un punto del texto, si es que hay alguno.
 *
 * Se sube del nodo de texto al elemento anclable, que es la unidad que la
 * superficie ya numera. Marcar el bloque y no el carácter no es una
 * aproximación: una cinta de papel señala la página, no la letra.
 */
export function bloqueEn(
  proyectado: TextoProyectado,
  nodos: readonly Text[],
  indice: number
): HTMLElement | null {
  if (indice < 0 || indice >= proyectado.largo) return null;
  const nodo = nodos[proyectado.refs[indice] as number];
  return nodo?.parentElement?.closest<HTMLElement>('[data-nb-bloque]') ?? null;
}

/** Qué se pudo pintar sobre el texto y qué no. */
export interface InformeDeMarcas {
  /** Si el motor sabe pintar rangos. Si no sabe, no se ven y hay que decirlo. */
  soportado: boolean;
  /** Cuántas se pintaron en este capítulo. */
  pintadas: number;
  /** Las que no cayeron donde su propia cita dice. Nunca se pintan a medias. */
  sinColocar: string[];
}

/** Cuál de las marcas cubre un punto del texto, si alguna. La última gana. */
export function marcaEn(marcas: readonly Marca[], indice: number): Marca | null {
  let encontrada: Marca | null = null;
  for (const marca of marcas) {
    if (indice >= marca.inicio && indice < marca.fin) encontrada = marca;
  }
  return encontrada;
}

/**
 * Las anotaciones de **este** capítulo, en la forma que `colocar` entiende.
 *
 * Se quedan fuera tres clases, y cada una por un motivo distinto:
 *
 * * Las que el runtime no supo colocar en el documento. Ya salen marcadas en el
 *   panel; pintarlas en algún sitio sería inventarles uno.
 * * Las de otro capítulo, y las que no anclan por fragmento. Un `page` o un
 *   `time` no son coordenadas de este formato.
 * * Los **marcadores**, que no marcan un tramo sino un punto. Enseñar por dónde
 * vas es otra cosa y tiene su propio módulo: `trazos.ts`.
 */
export function anotacionesDelCapitulo(
  entradas: readonly OutlineEntry[],
  href: string
): Anotacion[] {
  const salida: Anotacion[] = [];
  for (const entrada of entradas) {
    const posicion = entrada.position;
    if (entrada.lost || !posicion || posicion.kind !== 'fragment') continue;
    if (posicion.value.href !== href) continue;
    if (entrada.annotation.kind === 'bookmark') continue;
    const cita = entrada.annotation.target.text?.highlight ?? '';
    if (cita.length === 0) continue;
    salida.push({
      id: entrada.annotation.id,
      clase: entrada.annotation.kind,
      inicio: posicion.value.offset,
      cita
    });
  }
  return salida;
}

/* -------------------------------------------------------------------------- */
/* La parte que necesita un árbol de verdad.                                   */
/*                                                                             */
/* Vive aquí y no en el componente porque es la otra mitad del mismo contrato,  */
/* pero no la cubre ninguna prueba de esta capa: las de aquí corren sin         */
/* navegador. La cubren los recorridos de `e2e/`, que corren sobre el marcado   */
/* que sale de verdad.                                                          */
/* -------------------------------------------------------------------------- */

/** Un árbol recorrido: los eventos que produce y los nodos a los que apuntan. */
export interface ArbolRecorrido {
  eventos: Evento[];
  nodos: Text[];
}

/**
 * Recorre el marcado compuesto y produce los eventos que `normalizar` espera.
 *
 * El orden del recorrido es el del documento, que es el mismo en el que el
 * runtime lee el XHTML. La separación se emite **al salir** de un bloque, igual
 * que allí se emite al cerrar la etiqueta.
 */
export function recorrer(raiz: Element): ArbolRecorrido {
  const eventos: Evento[] = [];
  const nodos: Text[] = [];

  const recorrido = (nodo: Node): void => {
    for (const hijo of nodo.childNodes) {
      if (hijo.nodeType === Node.TEXT_NODE) {
        const texto = hijo as Text;
        eventos.push({ tipo: 'texto', texto: texto.data, ref: nodos.length });
        nodos.push(texto);
      } else if (hijo.nodeType === Node.ELEMENT_NODE) {
        const elemento = hijo as Element;
        if (elemento.getAttribute('aria-hidden') === 'true') continue;
        recorrido(hijo);
        /*
         * Separa por etiqueta **o por ser un bloque declarado**, que es lo que
         * hace que esto valga también para el PDF.
         *
         * El texto de un PDF no es marcado sino fragmentos colocados, y van en
         * `<span>`, que no puede entrar en `SEPARAN`: en un EPUB un `<span>` está
         * dentro de un párrafo y separar ahí partiría palabras. Lo que distingue a
         * un fragmento de PDF es llevar `data-nb-bloque`, que es justo lo que la
         * superficie les pone.
         *
         * Y con esto los dos formatos producen **el mismo texto**: la proyección
         * convierte cada separación en un solo espacio y colapsa el resto, que es
         * literalmente lo que hace `normalizar` en el proveedor de PDF. Sin la
         * separación, dos fragmentos contiguos se pegaban —"palabraotra"— y
         * ninguna cita coincidía.
         *
         * En un EPUB no cambia nada: sus bloques ya separaban por etiqueta, y
         * marcar la separación dos veces produce un espacio igual.
         */
        if (SEPARAN.has(elemento.tagName) || elemento.hasAttribute('data-nb-bloque')) {
          eventos.push({ tipo: 'finDeBloque' });
        }
      }
    }
  };

  recorrido(raiz);
  return { eventos, nodos };
}

/**
 * El rango del DOM que cubre una marca, o `null` si sus extremos no existen.
 *
 * El final se toma como el fin del último punto de código y no como el inicio
 * del siguiente: si la marca termina donde termina un nodo, el siguiente estaría
 * en otro y el rango se comería el espacio de en medio.
 */
export function rangoDe(
  proyectado: TextoProyectado,
  nodos: readonly Text[],
  marca: Marca
): Range | null {
  const ultimo = marca.fin - 1;
  if (marca.inicio < 0 || ultimo >= proyectado.largo || ultimo < marca.inicio) return null;

  const nodoInicial = nodos[proyectado.refs[marca.inicio] as number];
  const nodoFinal = nodos[proyectado.refs[ultimo] as number];
  if (!nodoInicial || !nodoFinal) return null;

  const rango = document.createRange();
  rango.setStart(nodoInicial, proyectado.desde[marca.inicio] as number);
  rango.setEnd(
    nodoFinal,
    (proyectado.desde[ultimo] as number) + (proyectado.ancho[ultimo] as number)
  );
  return rango;
}

/**
 * El camino de vuelta: de una posición del DOM al índice del texto.
 *
 * Es lo que permite señalar una marca o anclar un trazo con el puntero sin que
 * la capa visual de marcas intercepte la selección de texto (`pointer-events:
 * none`): se pregunta qué carácter hay bajo el punto y se traduce al índice.
 */
export function indiceDe(
  proyectado: TextoProyectado,
  nodos: readonly Text[],
  nodo: Text,
  desplazamiento: number
): number | null {
  const ref = nodos.indexOf(nodo);
  if (ref < 0) return null;

  /*
   * No se busca una coincidencia exacta, sino **el carácter más cercano por la
   * izquierda**, y esto no es una tolerancia: es que muchos desplazamientos del
   * documento no tienen carácter propio en el texto proyectado.
   *
   * El espacio en blanco se colapsa, así que ninguno de los que se descartan
   * está aquí. Y el punto donde se pulsa cae en un espacio con toda naturalidad:
   * el centro de "una aldea de veinte casas" es justo el hueco antes de
   * "veinte", que fue lo que encontró la primera prueba del gesto. Exigir
   * coincidencia exacta hacía que señalar en medio de una marca no señalara
   * nada, que desde fuera se ve como que el resaltado no se puede pulsar.
   *
   * Un espacio pertenece a la marca que lo contiene, así que devolver el
   * carácter anterior es la respuesta correcta y no una aproximación.
   */
  let anterior: number | null = null;
  let siguiente: number | null = null;
  for (let i = 0; i < proyectado.largo; i += 1) {
    if (proyectado.refs[i] !== ref) continue;
    const inicio = proyectado.desde[i] as number;
    const ancho = proyectado.ancho[i] as number;
    if (desplazamiento >= inicio && desplazamiento < inicio + ancho) return i;
    // El relleno de separación tiene ancho cero y el sitio del que le sigue: no
    // es de nadie, así que tampoco puede ser la respuesta.
    if (ancho === 0) continue;
    if (inicio < desplazamiento) anterior = i;
    else if (siguiente === null) siguiente = i;
  }
  return anterior ?? siguiente;
}
