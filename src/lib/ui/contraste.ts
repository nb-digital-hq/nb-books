/**
 * Relación de contraste según WCAG 2.1.
 *
 * Existe para que el contraste de la paleta sea una cifra medida y no una
 * afirmación: `tema.test.ts` la calcula sobre `src/app.css` y falla si algún
 * par baja de su umbral. Sin esto, "cumple WCAG 2.1 AA" sería justo la clase
 * de frase que este proyecto no se permite escribir sin mecanismo detrás.
 *
 * Fórmula: WCAG 2.1, definiciones de luminancia relativa y de relación de
 * contraste.
 */

/** Umbral de texto normal, WCAG 2.1 criterio 1.4.3 (AA). */
export const UMBRAL_TEXTO = 4.5;

/** Umbral de texto grande y de componentes de interfaz, criterios 1.4.3 y 1.4.11. */
export const UMBRAL_INTERFAZ = 3;

/** Canales de 0 a 255. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/**
 * Interpreta un color hexadecimal de tres o seis dígitos.
 *
 * Devuelve `null` en lugar de lanzar porque quien llama, al comprobar una hoja
 * de estilos, necesita distinguir "no es un color" de "es un color malo".
 */
export function interpretarHex(valor: string): Rgb | null {
  const limpio = valor.trim().replace(/^#/, '');

  if (/^[0-9a-fA-F]{3}$/.test(limpio)) {
    const duplicado = limpio.replace(/./g, (c) => c + c);
    const n = Number.parseInt(duplicado, 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }

  if (/^[0-9a-fA-F]{6}$/.test(limpio)) {
    const n = Number.parseInt(limpio, 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }

  return null;
}

function canalLineal(canal: number): number {
  const c = canal / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Luminancia relativa, entre 0 (negro) y 1 (blanco). */
export function luminanciaRelativa({ r, g, b }: Rgb): number {
  return 0.2126 * canalLineal(r) + 0.7152 * canalLineal(g) + 0.0722 * canalLineal(b);
}

/** Relación de contraste entre dos colores, entre 1 y 21. El orden no importa. */
export function relacionDeContraste(a: Rgb, b: Rgb): number {
  const la = luminanciaRelativa(a);
  const lb = luminanciaRelativa(b);
  const claro = Math.max(la, lb);
  const oscuro = Math.min(la, lb);
  return (claro + 0.05) / (oscuro + 0.05);
}

/** Igual que `relacionDeContraste`, sobre colores hexadecimales. */
export function contrasteHex(a: string, b: string): number {
  const ca = interpretarHex(a);
  const cb = interpretarHex(b);
  if (!ca || !cb) throw new Error(`Color hexadecimal no válido: ${!ca ? a : b}`);
  return relacionDeContraste(ca, cb);
}
