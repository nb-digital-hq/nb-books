#!/usr/bin/env node
/**
 * Escribe `src/paletas.css` con las catorce paletas de Material que la interfaz
 * puede necesitar: siete temas por dos niveles de contraste.
 *
 * # Por qué existe
 *
 * Porque desde que los colores se derivan de una semilla, **el primer fotograma
 * se pinta sin ellos**. El generador corre en JavaScript y escribe las variables
 * sobre el elemento raíz, así que entre que el navegador pinta el marcado y que
 * termina de ejecutarse el paquete no hay un solo `--m3c-*` definido: los fondos,
 * el texto y los trazos se quedan en su valor inicial y saltan al tema de golpe.
 * Comprobado el 2026-09-10 sobre el paquete construido: ninguna hoja de estilos
 * declaraba ni una de esas variables.
 *
 * Con las paletas en CSS, el primer fotograma ya sale con su tema. Y desde el
 * 2026-09-11 el generador ya no corre al arrancar: sin acento propio la paleta es
 * la de este archivo y nada más, y la utilería de color solo se carga para
 * calcular un acento (`src/lib/ui/tema.ts`). Así el arranque no la paga.
 *
 * # Y por qué se genera en vez de escribirse
 *
 * Por lo mismo que los tipos del puente: una copia escrita a mano de algo que
 * una función calcula se desincroniza el día que la función cambie, y la
 * diferencia solo se vería como un color raro en la máquina de alguien. Este
 * archivo se compara en `pnpm verify` con `--check`.
 *
 * Uso:
 *   node scripts/generar-paletas.mjs           escribe el archivo
 *   node scripts/generar-paletas.mjs --check   falla si no está al día
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { createServer } from 'vite';

const DESTINO = resolve('src/paletas.css');

const servidor = await createServer({
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'silent'
});

const { generarVariablesM3 } = await servidor.ssrLoadModule('/src/lib/ui/generadorM3.ts');
const { TEMAS, SEMILLAS_POR_TEMA, NEUTROS_GRISES, esquemaDe } =
  await servidor.ssrLoadModule('/src/lib/ui/tema.ts');
await servidor.close();

function bloque(selector, semilla, oscuro, contraste, grises) {
  const variables = generarVariablesM3(semilla, oscuro, contraste, grises);
  const cuerpo = Object.entries(variables)
    .map(([nombre, valor]) => `  ${nombre}: ${valor};`)
    .join('\n');
  return `${selector} {\n${cuerpo}\n}`;
}

const bloques = [];
for (const tema of TEMAS) {
  const oscuro = esquemaDe(tema) === 'dark';
  const semilla = SEMILLAS_POR_TEMA[tema];
  const grises = NEUTROS_GRISES.includes(tema);
  bloques.push(bloque(`:root[data-theme='${tema}']`, semilla, oscuro, 0, grises));
  bloques.push(
    bloque(`:root[data-theme='${tema}'][data-contrast='high']`, semilla, oscuro, 1, grises)
  );
}

/*
 * `system` es el estado del documento antes de que nadie resuelva nada: es lo
 * que trae `app.html`. Sin estas dos reglas, el primer fotograma de una carga en
 * frío seguiría sin colores, que es justo lo que este archivo viene a arreglar.
 */
bloques.unshift(
  bloque(":root, :root[data-theme='system']", SEMILLAS_POR_TEMA.papel, false, 0, false),
  `@media (prefers-color-scheme: dark) {\n${bloque(
    "  :root, :root[data-theme='system']",
    SEMILLAS_POR_TEMA.grafito,
    true,
    0,
    true
  )
    .split('\n')
    .map((l) => `  ${l}`)
    .join('\n')}\n}`
);

const contenido = `/*
 * GENERADO POR scripts/generar-paletas.mjs. NO SE EDITA A MANO.
 *
 * Las catorce paletas de Material -siete temas por dos niveles de contraste- más
 * las dos del estado inicial del documento. Existen para que el primer fotograma
 * salga con su tema en lugar de sin colores: el motivo largo está en el script.
 */

${bloques.join('\n\n')}
`;

if (process.argv.includes('--check')) {
  let actual = '';
  try {
    actual = readFileSync(DESTINO, 'utf8');
  } catch {
    console.error('Paletas: falta src/paletas.css. Ejecuta node scripts/generar-paletas.mjs');
    process.exit(1);
  }
  if (actual !== contenido) {
    console.error(
      'Paletas: src/paletas.css no es el que genera el generador de color.\n' +
        'Ejecuta node scripts/generar-paletas.mjs y revisa el resultado.'
    );
    process.exit(1);
  }
  console.log('Paletas: las catorce están al día.');
} else {
  writeFileSync(DESTINO, contenido);
  console.log(`Paletas: escritas ${bloques.length} en ${DESTINO}`);
}
