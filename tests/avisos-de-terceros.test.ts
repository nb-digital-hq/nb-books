/**
 * Los avisos legales y de terceros viajan en el paquete y se sirven a `Acerca de`
 *.
 *
 * La sección 4 de Apache License 2.0 obliga a conservar y redistribuir `NOTICE` y
 * `LICENSE`, y las licencias permisivas de las dependencias obligan a redistribuir
 * sus avisos (`THIRD-PARTY-NOTICES.md`). Si alguno de los tres sale de
 * `bundle.resources` en `src-tauri/tauri.conf.json`, el instalador deja de
 * cumplir la licencia aunque el archivo siga en el repositorio.
 *
 * Contrato: `docs/legal/third-party-notices.md`, secciones 3 y 5.
 */

import { readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const RAIZ = resolve(import.meta.dirname, '..');
const RUTA_TAURI_CONF = join(RAIZ, 'src-tauri', 'tauri.conf.json');

interface ConfiguracionTauri {
  bundle?: {
    licenseFile?: string;
    resources?: Record<string, string>;
  };
}

function verificarRecursosLegalesEnPaquete(config: ConfiguracionTauri, raiz = RAIZ): string[] {
  const recursos = config.bundle?.resources ?? {};
  const requeridos: Array<[string, string]> = [
    ['../LICENSE', 'LICENSE'],
    ['../NOTICE', 'NOTICE'],
    ['../THIRD-PARTY-NOTICES.md', 'THIRD-PARTY-NOTICES.md']
  ];
  const problemas: string[] = [];

  for (const [origenRelativo, destinoEnPaquete] of requeridos) {
    if (!(origenRelativo in recursos)) {
      problemas.push(`falta "${origenRelativo}" en bundle.resources`);
      continue;
    }
    if (recursos[origenRelativo] !== destinoEnPaquete) {
      problemas.push(
        `"${origenRelativo}" se empaqueta como "${recursos[origenRelativo]}" en lugar de "${destinoEnPaquete}"`
      );
    }
    const rutaReal = resolve(join(raiz, 'src-tauri'), origenRelativo);
    try {
      const info = statSync(rutaReal);
      if (!info.isFile() || info.size === 0) {
        problemas.push(`"${origenRelativo}" está vacío o no es un archivo regular`);
      }
    } catch {
      problemas.push(`"${origenRelativo}" no existe en disco`);
    }
  }

  return problemas;
}

describe('los avisos legales viajan en el paquete y llegan a Acerca de', () => {
  const config = JSON.parse(readFileSync(RUTA_TAURI_CONF, 'utf8')) as ConfiguracionTauri;

  it('LICENSE, NOTICE y THIRD-PARTY-NOTICES.md están en bundle.resources y existen en disco', () => {
    expect(verificarRecursosLegalesEnPaquete(config)).toEqual([]);
    expect(config.bundle?.licenseFile).toBe('../LICENSE');
  });

  it('falla si cualquiera de los tres archivos deja de ir en bundle.resources', () => {
    for (const clave of ['../LICENSE', '../NOTICE', '../THIRD-PARTY-NOTICES.md']) {
      const recursosIncompletos = { ...(config.bundle?.resources ?? {}) };
      delete recursosIncompletos[clave];
      const problemas = verificarRecursosLegalesEnPaquete({
        bundle: { ...config.bundle, resources: recursosIncompletos }
      });
      expect(problemas, `quitar ${clave} tiene que hacer fallar la comprobación`).toContain(
        `falta "${clave}" en bundle.resources`
      );
    }
  });

  it('el comando legal_notices sirve los mismos tres archivos a Acerca de', () => {
    const ipc = readFileSync(join(RAIZ, 'src-tauri', 'src', 'ipc.rs'), 'utf8');
    expect(ipc).toContain('pub fn legal_notices()');
    expect(ipc).toContain('include_str!("../../LICENSE")');
    expect(ipc).toContain('include_str!("../../NOTICE")');
    expect(ipc).toContain('include_str!("../../THIRD-PARTY-NOTICES.md")');
  });
});
