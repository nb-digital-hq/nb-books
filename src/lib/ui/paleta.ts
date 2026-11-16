/**
 * La paleta de Material que corresponde a una apariencia.
 *
 * Contrato: `docs/architecture/ui-components.md`, sección 5.1.
 *
 * Vive aparte de `tema.ts` por el peso y no por el orden: la utilería de color de
 * Material ocupa 94 KB, y sus catorce paletas de fábrica ya viajan calculadas en
 * `src/paletas.css`. Solo hace falta calcular cuando el usuario elige un acento
 * propio, así que `tema.ts` carga este módulo a demanda y el arranque no la paga.
 * Las cifras están en `performance-budgets.md`, sección 4.11.
 *
 * **Nada del paquete de arranque puede importar este archivo de forma estática.**
 * No fallaría nada: la utilería volvería al arranque sin que se viera, que es
 * como el paquete llegó al doble de su presupuesto. `scripts/check-paquete.mjs`
 * lo comprueba sobre la compilación.
 */

import { generarVariablesM3 } from './generadorM3.js';
import {
  type Apariencia,
  esquemaDe,
  NEUTROS_GRISES,
  SEMILLAS_POR_TEMA,
  type TokensDeTema,
  temaBase
} from './tema.js';

/**
 * Qué variable de Material pone cada color que un tema de extensión declara.
 *
 * **Existe porque un tema declara catorce colores y la interfaz usa más de
 * doscientos.** Generar la paleta desde su acento deja todo en su esquema, pero
 * las superficies y los textos saldrían los que Material derive, no los que el
 * autor eligió: el tema se vería "parecido" y no como es. Así que lo que declara
 * se pone encima de la paleta generada, y lo que no declara lo pone Material.
 *
 * `--nb-focus` y `--nb-text-subtle` no están: no tienen equivalente en Material y
 * se usan tal cual desde los tokens del propio tema.
 */
const EQUIVALENCIAS: Readonly<Record<string, readonly string[]>> = {
  '--nb-surface': ['--m3c-surface', '--m3c-surface-dim', '--m3c-background'],
  '--nb-surface-raised': [
    '--m3c-surface-container',
    '--m3c-surface-container-high',
    '--m3c-surface-bright'
  ],
  '--nb-surface-sunken': ['--m3c-surface-container-lowest', '--m3c-surface-container-low'],
  '--nb-text': ['--m3c-on-surface', '--m3c-on-surface-container'],
  '--nb-text-muted': ['--m3c-on-surface-variant'],
  '--nb-accent': ['--m3c-primary'],
  '--nb-on-accent': ['--m3c-on-primary'],
  '--nb-accent-soft': ['--m3c-primary-container'],
  '--nb-border': ['--m3c-outline-variant'],
  '--nb-border-strong': ['--m3c-outline'],
  '--nb-danger': ['--m3c-error'],
  '--nb-on-danger': ['--m3c-on-error']
};

function conLosColoresDelTema(
  variables: Record<string, string>,
  tokens: TokensDeTema
): Record<string, string> {
  for (const [token, equivalentes] of Object.entries(EQUIVALENCIAS)) {
    const valor = tokens[token];
    if (!valor) continue;
    for (const nombre of equivalentes) variables[nombre] = valor;
  }
  return variables;
}

export function paletaDe(
  apariencia: Apariencia,
  elSistemaPrefiereOscuro: boolean
): Record<string, string> {
  const resuelto = temaBase(apariencia, elSistemaPrefiereOscuro);
  const tema = apariencia.temaDeExtension;

  /*
   * **El alto contraste es un nivel, no otra semilla.**
   *
   * Material deriva cada color contra su fondo con una curva de contraste, y el
   * nivel es un parámetro de esa curva: a 1 recalcula la paleta entera para
   * separar más. Sustituir la semilla por blanco o negro puro -que es lo que se
   * hacía antes- no separa nada: medido sobre Gabinete, el
   * contraste de texto sobre fondo era 14,36 con alto contraste y 14,36 sin él.
   * Y además le cambiaba el tono al tema, porque el blanco no tiene matiz del
   * que partir.
   *
   * Como el nivel es independiente de la semilla, el acento del usuario deja de
   * sacrificarse: ya no hay motivo para elegir entre su color y ver.
   */
  /*
   * Con un tema de extensión puesto, su acento es la semilla y el del usuario se
   * queda esperando: la paleta tiene que salir del tema que se eligió, no de una
   * mezcla de los dos. Y los neutros van sin matiz, porque las superficies que se
   * ven son las que el tema declara y teñirlas con otro tono las separaría de él.
   */
  const variables = generarVariablesM3(
    tema?.['--nb-accent'] || apariencia.acento || SEMILLAS_POR_TEMA[resuelto],
    esquemaDe(resuelto) === 'dark',
    apariencia.altoContraste ? 1 : 0,
    tema ? true : NEUTROS_GRISES.includes(resuelto)
  );

  return tema ? conLosColoresDelTema(variables, tema) : variables;
}
