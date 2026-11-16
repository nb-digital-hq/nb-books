/**
 * Modos de lectura que aporta un paquete de personalización.
 *
 * # Qué comprueba esto y qué comprueba el runtime
 *
 * El runtime comprueba la **forma** al validar el manifiesto: que los colores
 * sean hexadecimales de seis dígitos, que el brillo y la temperatura estén en
 * su rango, y que el paquete que los trae no declare permisos ni código.
 *
 * Aquí se comprueba el **contraste**, y se hace aquí a propósito: la aritmética
 * y el umbral viven en `contraste.ts`, que es de donde los toma el sistema de
 * diseño. Es la misma decisión que con los temas, y por el mismo
 * motivo: dos fuentes para un número que tiene que ser uno acaban separándose.
 *
 * # Por qué el umbral de texto y no el de interfaz
 *
 * Porque lo que un modo pinta es **el libro**: fondo y texto de la superficie
 * de lectura, que es lo que alguien mira durante horas. Los seis modos del
 * núcleo cumplen 4,5 y hay una prueba que lo calcula; uno que llegue de fuera
 * pasa por el mismo listón.
 */

import type { ExtensionReadingMode } from '$lib/bindings';

import { contrasteHex, UMBRAL_TEXTO } from '../contraste';
import type { Modo } from './modos';

/** Un modo de extensión que no llega al umbral. */
export interface ModoIlegible {
  modo: string;
  medido: number;
  umbral: number;
}

/**
 * Convierte lo que declara el manifiesto en un modo del lector.
 *
 * El brillo y la temperatura llegan en porcentaje entero —un manifiesto se
 * escribe a mano y `95` no admite las dos lecturas de `0.95`— y aquí se pasan a
 * la fracción con la que compone la superficie.
 */
export function aModo(declarado: ExtensionReadingMode): Modo {
  return {
    nombre: declarado.id,
    etiqueta: declarado.name,
    fondo: declarado.background,
    texto: declarado.text,
    brillo: declarado.brightness / 100,
    temperatura: declarado.warmth / 100,
    monocromo: declarado.monochrome
  };
}

/** Si el texto de un modo se lee sobre su fondo. */
export function contrasteDe(modo: Modo): number {
  return contrasteHex(modo.texto, modo.fondo);
}

/**
 * Los modos que no llegan al umbral, para no ofrecerlos.
 *
 * Se filtran **antes de enseñarlos** y no al elegirlos: un modo ilegible en la
 * lista es una trampa, porque quien lo elige descubre el problema justo cuando
 * ya no puede leer para arreglarlo.
 */
export function legibles(declarados: readonly ExtensionReadingMode[]): {
  modos: Modo[];
  descartados: ModoIlegible[];
} {
  const modos: Modo[] = [];
  const descartados: ModoIlegible[] = [];

  for (const declarado of declarados) {
    const modo = aModo(declarado);
    const medido = contrasteDe(modo);
    if (medido >= UMBRAL_TEXTO) {
      modos.push(modo);
    } else {
      descartados.push({ modo: modo.nombre, medido, umbral: UMBRAL_TEXTO });
    }
  }
  return { modos, descartados };
}
