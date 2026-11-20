/**
 * Pruebas de accesibilidad y contrato del sistema de diseño.
 *
 * # Por qué existe este archivo
 *
 * Verifica los contratos de accesibilidad y teclado de los componentes de la interfaz,
 * en particular la etiqueta emergente (`Tooltip`) montada fuera de la barra que recorta
 * y los destinos de la barra lateral (`NavItem`) plegada y desplegada.
 *
 * Los otros cuatro componentes (TopAppBar, DataTable, Accordion, Autocomplete)
 * se retiraron al no tener pantalla que los use, conforme a ADR-014.
 */

import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';

import NavItem from './NavItem.svelte';
import TarjetaLateral from './TarjetaLateral.svelte';
import Tooltip from './Tooltip.svelte';

describe('la etiqueta emergente', () => {
  /*
   * **Regresión.** Nacía con `role="tooltip"` y sin que nada apuntara a ella:
   * un lector de pantalla no anuncia una etiqueta por su papel, la anuncia
   * porque el elemento con el foco la referencia con `aria-describedby`. Como
   * ese elemento lo pone quien usa el componente, el identificador se le pasa al
   * hijo; si dejara de pasarse, no habría forma de escribir esa referencia y
   * esta prueba es lo que lo detecta.
   */
  it('le da a su hijo el identificador con el que referenciarla', () => {
    let recibido: string | undefined;
    const hijo = createRawSnippet((parametros: () => { describedBy: string }) => {
      recibido = parametros().describedBy;
      return { render: () => `<button aria-describedby="${parametros().describedBy}">x</button>` };
    });

    const html = render(Tooltip, { props: { text: 'Volver', children: hijo } }).body;

    expect(recibido, 'el hijo no recibe con qué referenciar la etiqueta').toBeTruthy();
    expect(html).toContain(`aria-describedby="${recibido}"`);
  });

  it('dos etiquetas no comparten identificador', () => {
    // Dos identificadores iguales en la misma pantalla hacen que la referencia
    // apunte a cualquiera de las dos, y el lector anuncia la equivocada.
    const identificadores: string[] = [];
    const hijo = createRawSnippet((parametros: () => { describedBy: string }) => {
      identificadores.push(parametros().describedBy);
      return { render: () => '<button>x</button>' };
    });

    render(Tooltip, { props: { text: 'Una', children: hijo } }).body;
    render(Tooltip, { props: { text: 'Otra', children: hijo } }).body;

    expect(new Set(identificadores).size).toBe(2);
  });
});

describe('el destino de navegación (NavItem)', () => {
  it('desplegado enseña el texto directamente y no necesita etiqueta emergente', () => {
    const html = render(NavItem, {
      props: { icono: 'biblioteca', etiqueta: 'Biblioteca', plegado: false }
    }).body;

    expect(html).toContain('Biblioteca');
    expect(html).not.toContain('nb-tooltip-envoltura');
  });

  it('plegado se envuelve en etiqueta emergente con aria-describedby y sin title nativo', () => {
    const html = render(NavItem, {
      props: { icono: 'biblioteca', etiqueta: 'Biblioteca', plegado: true }
    }).body;

    // Se envuelve en Tooltip
    expect(html).toContain('nb-tooltip-envoltura');
    // El botón tiene aria-label y aria-describedby para que el lector lo anuncie
    expect(html).toMatch(/aria-label="Biblioteca"/);
    expect(html).toMatch(/aria-describedby="nb-tooltip-[a-z0-9]+"/);
    // Sin title nativo para no duplicar el cartel del navegador con el tooltip
    expect(html).not.toMatch(/title="Biblioteca"/);
    // Marcado con data-plegado="true"
    expect(html).toMatch(/data-plegado="true"/);
  });
});

describe('la tarjeta lateral viva (TarjetaLateral)', () => {
  it('desplegada enseña cabecera con icono, título, cifra y contenido hijo', () => {
    const hijo = createRawSnippet(() => ({
      render: () => '<span class="nb-lateral-tarjeta-detalle">5 de 10 documentos</span>'
    }));

    const html = render(TarjetaLateral, {
      props: {
        icono: 'importar',
        titulo: 'Importando',
        cifra: '50 %',
        plegada: false,
        etiquetaPlegar: 'Plegar la Tarjeta',
        etiquetaDesplegar: 'Desplegar la Tarjeta',
        alAlternar: () => {},
        onclick: () => {},
        children: hijo
      }
    }).body;

    expect(html).toContain('data-plegada="false"');
    expect(html).toContain('Importando');
    expect(html).toContain('50 %');
    expect(html).toContain('5 de 10 documentos');
    expect(html).toContain('aria-label="Plegar la Tarjeta"');
    expect(html).toContain('aria-expanded="true"');
  });

  it('plegada conserva la cabecera y la cifra pero no renderiza el contenido hijo', () => {
    const hijo = createRawSnippet(() => ({
      render: () => '<span class="nb-lateral-tarjeta-detalle">5 de 10 documentos</span>'
    }));

    const html = render(TarjetaLateral, {
      props: {
        icono: 'importar',
        titulo: 'Importando',
        cifra: '50 %',
        plegada: true,
        etiquetaPlegar: 'Plegar la Tarjeta',
        etiquetaDesplegar: 'Desplegar la Tarjeta',
        alAlternar: () => {},
        onclick: () => {},
        children: hijo
      }
    }).body;

    expect(html).toContain('data-plegada="true"');
    expect(html).toContain('Importando');
    expect(html).toContain('50 %');
    expect(html).not.toContain('5 de 10 documentos');
    expect(html).toContain('aria-label="Desplegar la Tarjeta"');
    expect(html).toContain('aria-expanded="false"');
  });
});
