#!/usr/bin/env node
/**
 * Prepara la carpeta de una publicación: los instaladores, el listado de
 * componentes, los avisos de terceros, y la lista de huellas firmada.
 *
 * Cumple la sección 5 de docs/audit/supply-chain.md, en lo que
 * toca a Linux: cada versión sale con sus sumas, su firma y su listado de
 * componentes con licencias, y un tercero puede comprobarlo con `sha256sum` y
 * `openssl` siguiendo docs/developer/deployment/releases.md, sección 6.
 *
 * Uso:
 *   NO_STRIP=1 pnpm empaquetar:linux
 *   pnpm procedencia --clave <par-de-claves.pkcs8> [--salida <carpeta>]
 *
 * Sin `--clave` deja la carpeta montada y **sin firmar**, y lo dice: sirve para
 * mirar lo que saldría, no para publicarlo.
 *
 * # Por qué exige el árbol limpio
 *
 * El listado de componentes dice de qué commit salió la publicación. Con
 * cambios sin confirmar, ese commit no es de donde salió, y un documento de
 * procedencia que nombra un origen falso es peor que no tenerlo.
 *
 * # Por qué las sumas no se calculan aquí
 *
 * Las calcula y las firma `firmar --artefactos`, y las comprueba
 * `comprobar_artefactos`: una sola implementación de qué entra en la lista. Si
 * este guion sumara por su cuenta, el día que una de las dos cambiara qué
 * archivos cuenta se firmaría una lista y se comprobaría otra.
 */

import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(import.meta.dirname, '..');

/** Dónde está la clave pública con la que se comprueba lo firmado. */
export const CLAVE_PUBLICA = join(ROOT, 'docs', 'architecture', 'api', 'clave-de-publicacion.pem');
/** Clave minisign pública con la que NB Launcher verifica el catálogo de NB Books. */
export const CLAVE_DEL_LAUNCHER = join(ROOT, 'docs', 'architecture', 'api', 'clave-del-launcher.pub');

/**
 * El nombre con el que se publica un instalador.
 *
 * El empaquetador los nombra con el nombre del producto, que lleva un espacio.
 * `sha256sum` lo admite, pero quien copia una orden de las instrucciones y la
 * pega sin comillas no, y un nombre que hay que entrecomillar es un tropiezo
 * puesto a propósito delante de quien viene a verificar.
 */
export function nombrePublicado(nombre) {
  return nombre.replace(/^NB Books/, 'nb-books');
}

/** Repositorio y propietario con los que NB Launcher reconoce las publicaciones de NB Books. */
export const DEL_LAUNCHER = { owner: 'nb.books', repository: 'nb-digital-hq/nb-books' };

/**
 * Los cuatro formatos de Linux que el Launcher conoce, con su identificador estable.
 * El `.tar.gz` es el único que el Launcher instala él mismo (`books_linux_v1`).
 */
export function formatosDelLauncher(version) {
  return [
    { id: 'books-linux-x64-tar', archivo: `nb-books_${version}_linux-x86_64.tar.gz`, formato: 'tar-gz', gestionado: true },
    { id: 'books-linux-x64-appimage', archivo: `nb-books_${version}_amd64.AppImage`, formato: 'appimage' },
    { id: 'books-linux-x64-deb', archivo: `nb-books_${version}_amd64.deb`, formato: 'deb' },
    { id: 'books-linux-x64-rpm', archivo: `nb-books-${version}-1.x86_64.rpm`, formato: 'rpm' }
  ];
}

/**
 * El catálogo que NB Launcher descarga y verifica: `launcher-catalog.json`, firmado
 * aparte con la clave minisign de NB Books (contrato de catálogo remoto del
 * Launcher, esquema 1). Cada artefacto lleva su tamaño, su SHA-256 y su propia
 * firma; el catálogo caduca a los catorce días de emitirse.
 */
export function catalogoDelLauncher({ version, emitido, clavePublica, artefactos }) {
  const DIA = 86400;
  return {
    schema: 1,
    owner: DEL_LAUNCHER.owner,
    repository: DEL_LAUNCHER.repository,
    version,
    issued_at: emitido,
    expires_at: emitido + 14 * DIA,
    artifacts: formatosDelLauncher(version).map((f) => {
      const a = artefactos[f.archivo];
      if (!a) throw new Error(`falta ${f.archivo}`);
      return {
        id: `${f.id}-v${version}`,
        app: DEL_LAUNCHER.owner,
        version,
        repository: DEL_LAUNCHER.repository,
        tag: `v${version}`,
        filename: f.archivo,
        platform: 'linux',
        architecture: 'x86_64',
        format: f.formato,
        bytes: a.bytes,
        sha256: a.sha256,
        signature: a.firma,
        public_key: clavePublica,
        ...(f.gestionado ? { managed_layout: 'books_linux_v1' } : {})
      };
    })
  };
}

/**
 * La sección de `CHANGELOG.md` de una versión, sin su encabezado. Vacía si no
 * la hay: el actualizador la trata como "sin notas".
 */
export function notasDeLaVersion({ changelog, version }) {
  const lineas = changelog.split('\n');
  const inicio = lineas.findIndex((l) => l.startsWith(`## ${version}`));
  if (inicio === -1) return '';
  const resto = lineas.slice(inicio + 1);
  const fin = resto.findIndex((l) => l.startsWith('## '));
  return `${(fin === -1 ? resto : resto.slice(0, fin)).join('\n').trim()}\n`;
}

/**
 * Una licencia como la quiere CycloneDX: expresión SPDX si lo es, nombre si no.
 *
 * `cargo metadata` todavía trae la forma antigua con barra -`MIT/Apache-2.0`-,
 * que significa "o" y no es SPDX válido.
 */
export function licenciaCdx(licencia) {
  // Lo que no es una licencia sino la confesión de que no se declaró una va
  // como nombre: presentarlo como expresión SPDX sería inventarla.
  if (/^(ver |SIN )/.test(licencia)) return [{ license: { name: licencia } }];
  return [{ expression: licencia.replaceAll('/', ' OR ') }];
}

/**
 * El listado de componentes, en CycloneDX 1.5.
 *
 * Sin marca de tiempo propia ni número de serie aleatorio: lleva la fecha del
 * commit, así que dos ejecuciones sobre el mismo commit dan el mismo archivo y
 * la misma huella. Un listado que cambia solo cada vez que se genera no se
 * puede comparar con nada.
 */
export function listado({ version, commit, fecha, rust, node, vendorizados, pdfium }) {
  const componente = (tipo, p) => ({
    type: 'library',
    name: p.nombre,
    version: p.version,
    purl: `pkg:${tipo}/${p.nombre.replace(/^@/, '%40')}@${p.version}`,
    licenses: licenciaCdx(p.licencia)
  });
  const componentes = [
    ...rust.map((p) => componente('cargo', p)),
    // pnpm junta en una fila las versiones de un mismo paquete; aquí cada una
    // es un componente, que es lo que son.
    ...node.flatMap((p) =>
      p.version.split(', ').map((v) => componente('npm', { ...p, version: v }))
    ),
    ...vendorizados.map((v) => ({
      type: 'library',
      name: v.nombre,
      version: v.version,
      licenses: licenciaCdx(v.licencia),
      properties: [{ name: 'nb-books:copiado-en', value: v.carpeta }]
    }))
  ];
  if (pdfium) {
    componentes.push({
      type: 'library',
      name: 'pdfium',
      version: pdfium.version,
      licenses: [{ expression: 'Apache-2.0 AND BSD-3-Clause' }],
      hashes: [{ alg: 'SHA-256', content: pdfium.sha256 }],
      properties: [{ name: 'nb-books:origen', value: 'binario descargado con huella fijada' }]
    });
  }
  return {
    bomFormat: 'CycloneDX',
    specVersion: '1.5',
    version: 1,
    metadata: {
      timestamp: fecha,
      component: {
        type: 'application',
        name: 'nb-books',
        version,
        licenses: [{ expression: 'Apache-2.0' }]
      },
      properties: [{ name: 'nb-books:commit', value: commit }]
    },
    components: componentes
  };
}

function git(...args) {
  return execFileSync('git', args, { cwd: ROOT, encoding: 'utf8' }).trim();
}

function herramienta(nombre, args) {
  const r = spawnSync(
    'cargo',
    ['run', '--quiet', '--features', 'herramientas', '--bin', nombre, '--', ...args],
    { cwd: join(ROOT, 'src-tauri'), stdio: 'inherit' }
  );
  return r.status === 0;
}

function fallar(mensaje) {
  console.error(mensaje);
  process.exit(1);
}

async function principal() {
  const args = process.argv.slice(2);
  const opcion = (nombre) => {
    const i = args.indexOf(nombre);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const clave = opcion('--clave');

  if (git('status', '--porcelain') !== '') {
    fallar(
      'Hay cambios sin confirmar. La procedencia nombra el commit del que sale la\n' +
        'publicación, y con el árbol así ese commit no es de donde sale.'
    );
  }
  const commit = git('rev-parse', 'HEAD');
  const fecha = git('log', '-1', '--format=%cI');
  const { version } = JSON.parse(readFileSync(join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));

  const paquetes = join(ROOT, 'src-tauri', 'target', 'release', 'bundle');
  const instaladores = ['appimage', 'deb', 'rpm', 'targz'].flatMap((tipo) => {
    const carpeta = join(paquetes, tipo);
    if (!existsSync(carpeta)) return [];
    return readdirSync(carpeta)
      .filter((n) => /\.(AppImage|deb|rpm|tar\.gz)$/.test(n))
      .map((n) => ({ origen: join(carpeta, n), nombre: nombrePublicado(n) }));
  });
  if (instaladores.length === 0) {
    fallar('No hay instaladores: ejecuta antes NO_STRIP=1 pnpm empaquetar:linux');
  }
  // Un instalador de otra versión, que quedó de una compilación anterior, se
  // firmaría como parte de esta sin que nada lo delatara.
  const ajenos = instaladores.filter((i) => !i.nombre.includes(version));
  if (ajenos.length > 0) {
    fallar(
      `Estos instaladores no son de la versión ${version}:\n` +
        ajenos.map((i) => `  ${i.nombre}`).join('\n')
    );
  }

  const salida = resolve(opcion('--salida') ?? join(ROOT, 'src-tauri', 'target', 'publicacion', version));
  rmSync(salida, { recursive: true, force: true });
  mkdirSync(salida, { recursive: true });
  for (const { origen, nombre } of instaladores) copyFileSync(origen, join(salida, nombre));
  copyFileSync(join(ROOT, 'THIRD-PARTY-NOTICES.md'), join(salida, 'THIRD-PARTY-NOTICES.md'));
  // Las notas de esta versión, sacadas de CHANGELOG.md. Van firmadas con lo demás:
  // el actualizador las enseña y las comprueba contra la misma lista.
  writeFileSync(join(salida, 'NOTAS.md'), notasDeLaVersion({ changelog: readFileSync(join(ROOT, 'CHANGELOG.md'), 'utf8'), version }));

  const { dependenciasNode, dependenciasRust, VENDORIZADOS } = await import('./dependencias.mjs');
  const sello = join(ROOT, 'src-tauri', 'vendor', 'pdfium', 'version.txt');
  const [versionPdfium, sha256] = existsSync(sello)
    ? readFileSync(sello, 'utf8').trim().split(' ')
    : [];
  if (!sha256) fallar('Falta el motor de PDF: ejecuta pnpm pdfium. El paquete no pudo salir con él.');
  writeFileSync(
    join(salida, `nb-books_${version}.cdx.json`),
    `${JSON.stringify(
      listado({
        version,
        commit,
        fecha,
        rust: dependenciasRust(),
        node: dependenciasNode(),
        vendorizados: VENDORIZADOS,
        pdfium: { version: versionPdfium, sha256 }
      }),
      null,
      2
    )}\n`
  );

  // El catálogo para NB Launcher, si se da su clave. Va antes de `SHA256SUMS`, que
  // así lo cubre también.
  const claveLauncher = opcion('--clave-launcher');
  if (claveLauncher) {
    const firmaMinisign = (archivo) => {
      const r = spawnSync(
        process.execPath,
        [join(ROOT, 'scripts', 'tauri.mjs'), 'signer', 'sign', '-f', resolve(claveLauncher), '-p', '', archivo],
        { cwd: ROOT, encoding: 'utf8' }
      );
      if (r.status !== 0) fallar(`No se pudo firmar para el Launcher: ${basename(archivo)}`);
      const sig = readFileSync(`${archivo}.sig`, 'utf8').trim();
      rmSync(`${archivo}.sig`);
      return sig;
    };
    const artefactos = {};
    for (const f of formatosDelLauncher(version)) {
      const ruta = join(salida, f.archivo);
      if (!existsSync(ruta)) fallar(`Falta ${f.archivo}: el Launcher lo espera.`);
      const bytes = readFileSync(ruta);
      artefactos[f.archivo] = {
        bytes: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
        firma: firmaMinisign(ruta)
      };
    }
    const clavePublica = readFileSync(`${resolve(claveLauncher)}.pub`, 'utf8').trim();
    if (clavePublica !== readFileSync(CLAVE_DEL_LAUNCHER, 'utf8').trim()) {
      fallar('La clave del Launcher no es la que está publicada en el repositorio.');
    }
    const catalogo = join(salida, 'launcher-catalog.json');
    writeFileSync(
      catalogo,
      `${JSON.stringify(catalogoDelLauncher({ version, emitido: Math.floor(Date.now() / 1000), clavePublica, artefactos }), null, 2)}\n`
    );
    writeFileSync(`${catalogo}.sig`, `${firmaMinisign(catalogo)}\n`);
  }

  if (!clave) {
    console.log(
      `Carpeta montada en ${salida}, SIN FIRMAR.\n` +
        'No es una publicación: falta --clave <par-de-claves.pkcs8>.'
    );
    return;
  }
  if (!herramienta('firmar', ['--artefactos', salida, '--clave', resolve(clave)])) {
    fallar('No se pudo firmar.');
  }
  // Se comprueba contra la clave del repositorio y no contra la que acaba de
  // firmar: es la que va a usar quien verifique, y si no son la misma es ahora
  // cuando hay que enterarse.
  if (!existsSync(CLAVE_PUBLICA)) {
    fallar(
      `Firmado, pero no hay clave pública en ${CLAVE_PUBLICA}: nadie podría\n` +
        'comprobarlo. Se obtiene con firmar --clave-publica-pem y se confirma en el repositorio.'
    );
  }
  if (!herramienta('comprobar_artefactos', [salida, '--clave', CLAVE_PUBLICA])) {
    fallar('Lo firmado no pasa la comprobación con la clave publicada. No se publica.');
  }
  console.log(`Publicación de ${version} (${commit.slice(0, 7)}) lista en ${salida}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await principal();
