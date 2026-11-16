/**
 * La elección de voz, que es la diferencia entre leer y hacer ruido.
 *
 * El catálogo de ejemplo empieza por afrikáans a propósito: es el que devuelve
 * `speech-dispatcher` de verdad, ordenado alfabéticamente, y era el que se
 * acababa usando para leer en cualquier idioma.
 */

import { describe, expect, it } from 'vitest';

import type { VozDisponible } from './contrato';
import { catalogoLegible, elegirVoz, idiomaBase } from './eleccion';

const CATALOGO: VozDisponible[] = [
  { id: 'Afrikaans', nombre: 'Afrikaans', idioma: 'af' },
  { id: 'English (Great Britain)', nombre: 'English (Great Britain)', idioma: 'en-GB' },
  { id: 'Portuguese (Brazil)', nombre: 'Portuguese (Brazil)', idioma: 'pt-br' },
  { id: 'Spanish (Latin America)', nombre: 'Spanish (Latin America)', idioma: 'es-419' },
  { id: 'Spanish (Spain)', nombre: 'Spanish (Spain)', idioma: 'es' }
];

describe('elegir la voz', () => {
  it('lee un libro en español con una voz española y no con la primera de la lista', () => {
    expect(elegirVoz(CATALOGO, { documento: 'es', interfaz: 'es' })).toBe('Spanish (Spain)');
  });

  it('prefiere la variante exacta cuando el catálogo la tiene', () => {
    expect(elegirVoz(CATALOGO, { documento: 'es-419', interfaz: 'es' })).toBe(
      'Spanish (Latin America)'
    );
  });

  it('cae al idioma base cuando la variante no está', () => {
    // Un EPUB mexicano y un catálogo sin voz mexicana: se lee en español, que es
    // lo que se entiende, y no en el primer idioma que haya.
    expect(elegirVoz(CATALOGO, { documento: 'es-MX', interfaz: 'en' })).toBe('Spanish (Spain)');
  });

  it('prefiere la voz genérica del idioma a una variante suya', () => {
    // `es-419` está antes que `es` en el catálogo. Quedarse con la primera que
    // empiece por `es` haría depender la elección del orden en que el
    // sintetizador devuelva su lista, que cambia entre versiones.
    expect(elegirVoz(CATALOGO, { documento: 'es-AR', interfaz: 'en' })).toBe('Spanish (Spain)');
  });

  it('usa el idioma de la interfaz cuando el documento no dice el suyo', () => {
    expect(elegirVoz(CATALOGO, { documento: null, interfaz: 'pt' })).toBe('Portuguese (Brazil)');
  });

  it('lo que el usuario eligió gana a cualquier deducción', () => {
    expect(
      elegirVoz(CATALOGO, { documento: 'es', interfaz: 'es', guardada: 'English (Great Britain)' })
    ).toBe('English (Great Britain)');
  });

  it('una elección guardada que ya no existe no bloquea la deducción', () => {
    // Cambiar de máquina, o desinstalar un sintetizador, deja una preferencia
    // apuntando a una voz que no está. Sin esto, la voz quedaba en nada.
    expect(elegirVoz(CATALOGO, { documento: 'es', interfaz: 'es', guardada: 'Klingon' })).toBe(
      'Spanish (Spain)'
    );
  });

  it('sin ninguna coincidencia se queda con la primera, y lo hace explícito', () => {
    expect(elegirVoz(CATALOGO, { documento: 'ja', interfaz: 'ko' })).toBe('Afrikaans');
  });

  it('sin voces no inventa ninguna', () => {
    expect(elegirVoz([], { documento: 'es', interfaz: 'es' })).toBeNull();
  });
});

describe('el idioma base', () => {
  it('recorta la variante venga como venga', () => {
    expect(idiomaBase('es-MX')).toBe('es');
    expect(idiomaBase('pt_BR')).toBe('pt');
    expect(idiomaBase('  ES  ')).toBe('es');
    expect(idiomaBase('es')).toBe('es');
  });
});

describe('el catálogo que se enseña', () => {
  it('nombra los idiomas en la lengua de quien mira', () => {
    const legible = catalogoLegible(CATALOGO, 'es');
    const espanol = legible.find((v) => v.id === 'Spanish (Spain)');
    // No se fija el texto exacto: lo pone el sistema y cambia con la versión de
    // ICU. Lo que se comprueba es que ya no es el código.
    expect(espanol?.etiqueta).not.toBe('es');
    expect(espanol?.etiqueta.toLowerCase()).toContain('espa');
  });

  it('viene ordenado, que es lo que hace recorrible una lista de ciento treinta', () => {
    const etiquetas = catalogoLegible(CATALOGO, 'es').map((v) => v.etiqueta);
    const ordenadas = [...etiquetas].sort(new Intl.Collator('es').compare);
    expect(etiquetas).toEqual(ordenadas);
  });

  it('no pierde ninguna voz por el camino', () => {
    expect(catalogoLegible(CATALOGO, 'es')).toHaveLength(CATALOGO.length);
  });
});
