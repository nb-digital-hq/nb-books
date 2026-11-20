/**
 * El listado de componentes de una publicación.
 *
 * Lo que se fija aquí es lo que hace que el listado sirva para algo: que dos
 * ejecuciones sobre el mismo commit den el mismo archivo, que una licencia sin
 * declarar no se presente como una expresión SPDX, y que el motor de PDF -que
 * no viene de ningún gestor de paquetes- figure con su huella.
 */
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const GUION = fileURLToPath(new URL('../scripts/procedencia.mjs', import.meta.url));

/**
 * Llama a una función del guion en un proceso de Node, como lo ejecuta quien
 * publica. No se importa: es un guion sin tipos, e importarlo lo mete en la
 * comprobación de tipos de la aplicación, que no es su sitio.
 */
function llamar<T>(funcion: string, argumento: unknown): T {
  const salida = execFileSync(
    'node',
    [
      '--input-type=module',
      '-e',
      `const m = await import(${JSON.stringify(GUION)});
       console.log(JSON.stringify(m[process.argv[1]](...JSON.parse(process.argv[2]))));`,
      funcion,
      JSON.stringify(
        Array.isArray(argumento) && ['nombrePublicado', 'nombreDeEdicion'].includes(funcion)
          ? argumento
          : [argumento]
      )
    ],
    { encoding: 'utf8' }
  );
  return JSON.parse(salida) as T;
}
interface Componente {
  name: string;
  purl?: string;
  hashes?: { alg: string; content: string }[];
}
interface Listado {
  serialNumber?: string;
  metadata: {
    timestamp: string;
    component: { version: string };
    properties: { name: string; value: string }[];
  };
  components: Componente[];
}
const listado = (e: unknown) => llamar<Listado>('listado', e);
const licenciaCdx = (l: string) => llamar<unknown>('licenciaCdx', l);
const nombrePublicado = (n: string) => llamar<string>('nombrePublicado', n);

const entrada = {
  version: '1.0.0',
  commit: 'abc1234',
  fecha: '2026-11-14T05:00:00-05:00',
  rust: [
    { nombre: 'serde', version: '1.0.0', licencia: 'MIT/Apache-2.0', repositorio: '' },
    { nombre: 'rara', version: '0.1.0', licencia: 'SIN DECLARAR', repositorio: '' }
  ],
  node: [{ nombre: '@scope/paquete', version: '1.0.0, 2.0.0', licencia: 'MIT', repositorio: '' }],
  vendorizados: [
    {
      nombre: 'any-tts',
      version: '0.1.3',
      licencia: 'MIT OR Apache-2.0',
      carpeta: 'src-tauri/src/voice/kokoro'
    }
  ],
  pdfium: { version: 'chromium/7881', sha256: 'f'.repeat(64) }
};

// Cada llamada arranca un proceso de Node: con la suite en paralelo, el plazo por
// defecto de cinco segundos no alcanza para dos o tres seguidas.
const PLAZO = { timeout: 30_000 };

describe('el listado de componentes', PLAZO, () => {
  it('es el mismo archivo cada vez que se genera sobre el mismo commit', () => {
    expect(JSON.stringify(listado(entrada))).toBe(JSON.stringify(listado(entrada)));
    expect(listado(entrada).metadata.timestamp).toBe(entrada.fecha);
    expect(listado(entrada)).not.toHaveProperty('serialNumber');
  });

  it('dice de qué commit y de qué versión salió', () => {
    const bom = listado(entrada);
    expect(bom.metadata.component.version).toBe('1.0.0');
    expect(bom.metadata.properties).toContainEqual({ name: 'nb-books:commit', value: 'abc1234' });
  });

  it('cuenta cada versión de un paquete de Node como un componente', () => {
    const purls = listado(entrada).components.map((c) => c.purl);
    expect(purls).toContain('pkg:npm/%40scope/paquete@1.0.0');
    expect(purls).toContain('pkg:npm/%40scope/paquete@2.0.0');
  });

  it('trae el motor de PDF con la huella con la que se descargó', () => {
    const pdfium = listado(entrada).components.find((c) => c.name === 'pdfium');
    expect(pdfium?.hashes).toEqual([{ alg: 'SHA-256', content: 'f'.repeat(64) }]);
  });

  it('no presenta como expresión SPDX una licencia que no se declaró', () => {
    expect(licenciaCdx('MIT/Apache-2.0')).toEqual([{ expression: 'MIT OR Apache-2.0' }]);
    expect(licenciaCdx('SIN DECLARAR')).toEqual([{ license: { name: 'SIN DECLARAR' } }]);
    expect(licenciaCdx('ver archivo de licencia')).toEqual([
      { license: { name: 'ver archivo de licencia' } }
    ]);
  });
});

describe('el nombre con el que se publica un instalador', PLAZO, () => {
  it('no lleva el espacio del nombre del producto', () => {
    expect(nombrePublicado('NB Books_0.1.1_amd64.AppImage')).toBe('nb-books_0.1.1_amd64.AppImage');
    expect(nombrePublicado('NB Books-1.0.0-1.x86_64.rpm')).toBe('nb-books-1.0.0-1.x86_64.rpm');
  });
});

describe('las notas de una publicación', PLAZO, () => {
  const changelog = '# Cambios\n\n## 1.0.1\n\n* Corrige algo.\n\n## 1.0.0\n\nPrimera versión.\n';

  it('son la sección de su versión y nada más', () => {
    expect(llamar('notasDeLaVersion', { changelog, version: '1.0.1' })).toBe('* Corrige algo.\n');
    expect(llamar('notasDeLaVersion', { changelog, version: '1.0.0' })).toBe('Primera versión.\n');
  });

  it('quedan vacías si la versión no tiene sección', () => {
    expect(llamar('notasDeLaVersion', { changelog, version: '2.0.0' })).toBe('');
  });
});

describe('el catálogo para NB Launcher', PLAZO, () => {
  it('lleva los cuatro formatos de Linux, solo el tar como instalable por el Launcher, y caduca a los catorce días', () => {
    const artefactos = Object.fromEntries(
      [
        'nb-books_1.0.0_linux-x86_64.tar.gz',
        'nb-books_1.0.0_amd64.AppImage',
        'nb-books_1.0.0_amd64.deb',
        'nb-books-1.0.0-1.x86_64.rpm'
      ].map((n, i) => [n, { bytes: 10 + i, sha256: 'a'.repeat(64), firma: `firma-${i}` }])
    );
    const catalogo = llamar<{
      owner: string;
      repository: string;
      issued_at: number;
      expires_at: number;
      artifacts: Array<{ id: string; format: string; managed_layout?: string; tag: string }>;
    }>('catalogoDelLauncher', {
      version: '1.0.0',
      emitido: 1000,
      clavePublica: 'clave',
      artefactos
    });
    expect(catalogo.owner).toBe('nb.books');
    expect(catalogo.repository).toBe('nb-digital-hq/nb-books');
    expect(catalogo.expires_at - catalogo.issued_at).toBe(14 * 86400);
    expect(catalogo.artifacts.map((a) => a.id)).toEqual([
      'books-linux-x64-tar-v1.0.0',
      'books-linux-x64-appimage-v1.0.0',
      'books-linux-x64-deb-v1.0.0',
      'books-linux-x64-rpm-v1.0.0'
    ]);
    expect(catalogo.artifacts.filter((a) => a.managed_layout).map((a) => a.format)).toEqual([
      'tar-gz'
    ]);
    expect(catalogo.artifacts.every((a) => a.tag === 'v1.0.0')).toBe(true);
  });
});

describe('ediciones de otras plataformas', () => {
  const nombreDeEdicion = (sufijo: string, archivo: string) =>
    llamar<string | null>('nombreDeEdicion', ['1.0.0', sufijo, archivo]);

  it('dos ediciones de la misma plataforma no se pisan en la publicación', () => {
    const universal = nombreDeEdicion('windows-x64', 'NB Books_1.0.0_x64-setup.exe');
    const rapida = nombreDeEdicion('windows-x64-avx2', 'NB Books_1.0.0_x64-setup.exe');
    expect(universal).toBe('nb-books_1.0.0_windows-x64-setup.exe');
    expect(rapida).toBe('nb-books_1.0.0_windows-x64-avx2-setup.exe');
  });

  it('cada formato conserva la terminación por la que el sistema lo reconoce', () => {
    expect(nombreDeEdicion('windows-x64', 'NB Books_1.0.0_x64_en-US.msi')).toBe(
      'nb-books_1.0.0_windows-x64.msi'
    );
    expect(nombreDeEdicion('macos-arm64', 'NB Books_1.0.0_aarch64.dmg')).toBe(
      'nb-books_1.0.0_macos-arm64.dmg'
    );
    expect(nombreDeEdicion('linux-x86_64-cuda', 'NB Books_1.0.0_amd64.AppImage')).toBe(
      'nb-books_1.0.0_linux-x86_64-cuda.AppImage'
    );
  });

  it('lo que no es un instalador no entra en la publicación', () => {
    expect(nombreDeEdicion('macos-arm64', 'bundle_dmg.sh')).toBeNull();
    expect(nombreDeEdicion('windows-x64', 'NB Books.exe')).toBeNull();
  });
});
