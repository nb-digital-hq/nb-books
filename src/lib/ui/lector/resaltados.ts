/**
 * Geometría de las marcas del capítulo para pintarlas en una capa propia.
 *
 * # Por qué una capa de elementos y no `CSS.highlights`
 *
 * Antes las marcas (`highlight`, `note`, `underline`) se registraban como
 * objetos `Range` en `CSS.highlights`. En WebKitGTK 2.52.6 —el motor del
 * producto en Linux— tener rangos registrados en `CSS.highlights` hace que
 * **cualquier inserción de un nodo o cambio de texto en cualquier parte del
 * documento** cueste en proporción al número de rangos y a la longitud del
 * capítulo: en un capítulo de 1.600 párrafos, actualizar el texto de un aviso o
 * de la píldora de posición pasa de 20 ms sin marcas a 50 ms con una, 140 ms
 * con cinco y 455 ms con veinte (`performance-budgets.md` 4.23).
 *
 * Pintar las marcas en una capa propia de `ReadingSurface.svelte` detrás del
 * texto —igual que la tinta, sin tocar los nodos de texto donde
 * opera la lectura biónica y sin registrar nada en `CSS.highlights`— deja el
 * coste de cualquier cambio del documento en los mismos ~20 ms que sin marcas.
 *
 * Contrato: `docs/architecture/reader-contracts.md` y
 * `docs/architecture/performance-budgets.md` 4.23.
 */

import { bloqueEn, type Marca, rangoDe, type TextoProyectado, textoEntre } from './proyeccion';

/** Un rectángulo ya referido al origen del lienzo de lectura. */
export interface RectanguloDeMarca {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Una marca medida y lista para pintarse en `.nb-capa-marcas`. */
export interface MarcaMedida {
  id: string;
  clase: string;
  /** El texto que cubre la marca, para que las pruebas comprueben qué cita quedó pintada. */
  cita: string;
  /** Índice del bloque (`data-nb-bloque`) donde empieza la marca, para el foco de línea. */
  bloque: number | null;
  /** Un rectángulo por renglón (o fragmento de columna) que ocupa la marca. */
  cajas: RectanguloDeMarca[];
}

/**
 * Si el entorno dispone de `Range` para medir los rectángulos del texto.
 *
 * No se cachea en una constante de módulo porque en las pruebas de nodo no hay
 * `document` al importar.
 */
export function sePuedePintar(): boolean {
  return typeof document !== 'undefined' && typeof document.createRange === 'function';
}

/**
 * Funde los rectángulos contiguos que caen en el mismo renglón.
 *
 * Cuando la lectura biónica está encendida, cada palabra se parte en un `<b
 * data-nb-bionica="si">` y un nodo de texto; `Range.getClientRects()` devuelve
 * entonces dos rectángulos por palabra en vez de uno por línea. Fundir los que
 * comparten renglón y se tocan en horizontal mantiene un único elemento por
 * línea, evita que el subrayado punteado de las notas reinicie su patrón en
 * cada sílaba y no une nunca dos columnas del modo paginado (separadas por
 * `column-gap`).
 */
export function fundirRectangulos(brutos: readonly RectanguloDeMarca[]): RectanguloDeMarca[] {
  const salida: RectanguloDeMarca[] = [];
  for (const r of brutos) {
    if (r.width <= 0 || r.height <= 0) continue;
    const ultimo = salida[salida.length - 1];
    if (
      ultimo &&
      Math.abs(r.top - ultimo.top) <= 4 &&
      Math.abs(r.top + r.height - (ultimo.top + ultimo.height)) <= 6 &&
      r.left >= ultimo.left - 2 &&
      r.left <= ultimo.left + ultimo.width + 4
    ) {
      const izq = Math.min(ultimo.left, r.left);
      const der = Math.max(ultimo.left + ultimo.width, r.left + r.width);
      const arr = Math.min(ultimo.top, r.top);
      const aba = Math.max(ultimo.top + ultimo.height, r.top + r.height);
      ultimo.left = izq;
      ultimo.top = arr;
      ultimo.width = der - izq;
      ultimo.height = aba - arr;
    } else {
      salida.push({ left: r.left, top: r.top, width: r.width, height: r.height });
    }
  }
  return salida;
}

/**
 * Mide dónde cae cada marca sobre el lienzo actual, sin registrar nada en
 * `CSS.highlights`.
 */
export function medirMarcas(
  dentro: HTMLElement,
  proyectado: TextoProyectado,
  nodos: readonly Text[],
  marcas: readonly Marca[]
): MarcaMedida[] {
  const cajaDelLienzo = dentro.getBoundingClientRect();
  const desplazamientoX = dentro.scrollLeft - cajaDelLienzo.left;
  const desplazamientoY = dentro.scrollTop - cajaDelLienzo.top;
  const medidas: MarcaMedida[] = [];

  for (const marca of marcas) {
    const rango = rangoDe(proyectado, nodos, marca);
    if (!rango) continue;

    const crudos = Array.from(rango.getClientRects(), (r) => ({
      left: r.left + desplazamientoX,
      top: r.top + desplazamientoY,
      width: r.width,
      height: r.height
    }));
    let cajas = fundirRectangulos(crudos);
    if (cajas.length === 0) {
      const envolvente = rango.getBoundingClientRect();
      if (envolvente.width > 0 && envolvente.height > 0) {
        cajas = [
          {
            left: envolvente.left + desplazamientoX,
            top: envolvente.top + desplazamientoY,
            width: envolvente.width,
            height: envolvente.height
          }
        ];
      }
    }
    if (cajas.length === 0) continue;

    const elBloque = bloqueEn(proyectado, nodos, marca.inicio);
    const indiceDeBloque = elBloque ? Number(elBloque.dataset.nbBloque) : Number.NaN;
    const cita = textoEntre(proyectado, marca.inicio, marca.fin - marca.inicio) ?? '';

    medidas.push({
      id: marca.id,
      clase: marca.clase,
      cita,
      bloque: Number.isFinite(indiceDeBloque) ? indiceDeBloque : null,
      cajas
    });
  }

  return medidas;
}
