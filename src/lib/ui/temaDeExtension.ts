/**
 * Temas aportados por extensiones.
 *
 * # Qué comprueba esto y qué comprueba el runtime
 *
 * El runtime comprueba la **forma**: que los colores estén en la lista cerrada
 * de tokens y sean hexadecimales de seis dígitos, y que un tema no traiga nada
 * ejecutable. Eso se hace al validar el manifiesto, antes de instalar.
 *
 * Aquí se comprueba el **contraste**, y se hace aquí a propósito: la aritmética
 * y los dos umbrales viven en `contraste.ts`, que es de donde los toma el
 * sistema de diseño. Reimplementarlos en Rust habría creado una segunda fuente
 * para un número que tiene que ser uno solo, y la única forma de que dos
 * fuentes no se separen es que no haya dos.
 *
 * # Por qué un tema no puede apagar el foco
 *
 * Porque no existe un token para apagarlo. Lo más que puede hacer es darle un
 * color, y ese color pasa por el umbral de interfaz como cualquier otro. Es la
 * misma idea que `check-ui.mjs` aplica al sistema de diseño, aplicada a lo que
 * viene de fuera.
 */

import { contrasteHex, UMBRAL_INTERFAZ, UMBRAL_TEXTO } from './contraste';

/**
 * Los pares que tienen que legibles, con su umbral.
 *
 * El primero de cada par se pinta **sobre** el segundo.
 */
const PARES: ReadonlyArray<readonly [string, string, number]> = [
  ['--nb-text', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-text-muted', '--nb-surface', UMBRAL_TEXTO],
  ['--nb-on-accent', '--nb-accent', UMBRAL_TEXTO],
  ['--nb-on-danger', '--nb-danger', UMBRAL_TEXTO],
  // El foco y el acento son interfaz, no texto: se ven, no se leen.
  ['--nb-focus', '--nb-surface', UMBRAL_INTERFAZ],
  ['--nb-accent', '--nb-surface', UMBRAL_INTERFAZ]
];

/** Un par que no llega a su umbral. */
export interface FalloDeContraste {
  encima: string;
  debajo: string;
  medido: number;
  umbral: number;
}

/**
 * Los pares del tema que no llegan a su umbral.
 *
 * Un par cuyos dos colores no vengan **los dos** en el tema no se comprueba: el
 * que falte lo pone `app.css`, y mezclar uno de cada sitio mediría algo que no
 * es lo que se va a ver.
 */
export function fallosDeContraste(tokens: Readonly<Record<string, string>>): FalloDeContraste[] {
  const fallos: FalloDeContraste[] = [];
  for (const [encima, debajo, umbral] of PARES) {
    const a = tokens[encima];
    const b = tokens[debajo];
    if (!a || !b) continue;
    const medido = contrasteHex(a, b);
    if (medido < umbral) fallos.push({ encima, debajo, medido, umbral });
  }
  return fallos;
}

/*
 * **Aplicarlo ya no se hace aquí, y eso fue el arreglo del 2026-09-12.**
 *
 * Hasta entonces este módulo escribía los tokens del tema en línea sobre la raíz
 * y los quitaba al cambiar. Con el rediseño, media interfaz pasó a
 * leer las variables de Material -que un tema no declara-, así que aquello dejaba
 * la pantalla mitad con el tema nuevo y mitad con el anterior: medido en
 * WebKitGTK, un tema oscuro sobre una paleta clara dejaba texto a 1,13 de
 * contraste. Ahora el tema es parte de la apariencia y lo pinta
 * `aplicarApariencia`, que es quien ya sabía derivar la paleta entera y llevar la
 * cuenta de lo que escribe en línea. Aquí solo queda el contraste, que es lo
 * único que hay que decidir **antes** de dejar que se ponga.
 */
