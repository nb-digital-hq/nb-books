import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Locator, NavigationEntry } from '$lib/bindings';
import OutlinePanel from './OutlinePanel.svelte';

const pagina = (n: number): Locator =>
  ({ href: 'doc.pdf', type: 'application/pdf', locations: { position: n } }) as unknown as Locator;
const entrada = (titulo: string, n: number): NavigationEntry =>
  ({ title: titulo, locator: pagina(n), children: [] }) as unknown as NavigationEntry;

describe('el índice de un documento', () => {
  it('marca una sola entrada como "aquí" aunque todas compartan recurso, como las páginas de un PDF', () => {
    const entradas = [1, 2, 3, 4, 5].map((n) => entrada(`Página ${n}`, n));
    const { body } = render(OutlinePanel, {
      props: { entradas, actual: pagina(3), alSaltar: () => {}, etiquetas: { aqui: 'AQUI' } }
    });
    expect(body.match(/AQUI/g)?.length).toBe(1);
    expect(body.indexOf('Página 3')).toBeLessThan(body.indexOf('AQUI'));
    expect(body.indexOf('AQUI')).toBeLessThan(body.indexOf('Página 4'));
  });

  it('entre capítulos con posición, "aquí" es el último que empieza antes de la lectura', () => {
    const entradas = [entrada('Uno', 1), entrada('Dos', 10), entrada('Tres', 20)];
    const { body } = render(OutlinePanel, {
      props: { entradas, actual: pagina(15), alSaltar: () => {}, etiquetas: { aqui: 'AQUI' } }
    });
    expect(body.match(/AQUI/g)?.length).toBe(1);
    expect(body.indexOf('Dos')).toBeLessThan(body.indexOf('AQUI'));
    expect(body.indexOf('AQUI')).toBeLessThan(body.indexOf('Tres'));
  });
});
