<!--
  @component
  Modos de lectura y kit de lectura accesible.

  Contrato: ui-components.md, secciones 5 y 6..

  **Un modo es un perfil completo**, no una paleta: fija fondo, texto, brillo y
  temperatura a la vez, y tiene nombre. Es la vía principal; el ajuste fino vive
  en el panel del libro y siempre está.

  **Lo emulado se llama emulación.** Un webview no controla el brillo del panel
  ni la temperatura del hardware: compone una capa encima. Decirlo aquí no es
  humildad, es la regla 6 de ADR-012.
-->
<script lang="ts">
  import { textos } from '$lib/i18n';
  import Icon from '../Icon.svelte';
  import Switch from '../Switch.svelte';
  import Seccion from '../vistas/Seccion.svelte';
  import { type KitAccesible, MODOS, type Modo } from './modos';

  interface Props {
    modo: string;
    kit: KitAccesible;
    /**
     * Los que aportan los paquetes de personalización instalados.
     *
     * Van en la **misma lista** que los seis del núcleo y no en una sección
     * aparte: para quien lee son modos, y separarlos sería enseñar un detalle
     * de implementación como si fuera una categoría. Llegan ya filtrados por
     * contraste: uno ilegible en la lista es una trampa, porque quien lo elige
     * descubre el problema justo cuando ya no puede leer para arreglarlo.
     */
    modosDeExtension?: readonly Modo[];
    alCambiarModo: (modo: string) => void;
    alCambiarKit: (campo: keyof KitAccesible, valor: KitAccesible[keyof KitAccesible]) => void;
    etiquetas?: Readonly<Record<string, string>>;
  }

  let {
    modo,
    kit,
    modosDeExtension = [],
    alCambiarModo,
    alCambiarKit,
    etiquetas
  }: Props = $props();

  const todos = $derived([...MODOS, ...modosDeExtension]);

  // El diccionario del idioma en uso, con las anulaciones de quien monta el
  // componente encima. `$derived` y no una constante: cambiar de idioma tiene
  // que repintar, y una constante capturaría el diccionario de un solo momento.
  const t = $derived({ ...textos().modo, ...etiquetas } as Record<string, string>);

  // El nombre del modo vive en su propio grupo del diccionario, con la clave
  // puesta en el identificador: `MODOS` guarda colores y números, no texto.
  const nombres = $derived(textos().modosDeLectura as Record<string, string>);
  // El de una extensión trae su etiqueta y **no se traduce**: es de su autor,
  // igual que el título de una orden.
  const nombreDelModo = (candidato: Modo): string =>
    candidato.etiqueta ?? nombres[candidato.nombre] ?? candidato.nombre;
  const texto = (clave: string): string => t[clave] ?? clave;

  const FAMILIAS = ['ninguna', 'dislexia', 'bajaVision'] as const;
  const INTERRUPTORES = ['bionica', 'regla', 'focoDeLinea'] as const;
</script>

<div class="nb-modos">
  <Seccion titulo={texto('modos')}>
    <ul class="nb-lista">
      {#each todos as candidato (candidato.nombre)}
        <li>
          <button
            type="button"
            class="nb-modo"
            aria-pressed={modo === candidato.nombre}
            onclick={() => alCambiarModo(candidato.nombre)}
          >
            <!--
              La muestra usa los colores del propio modo, así que no es un color
              literal de la interfaz: es el dato que el modo declara. Y las dos
              letras no son texto: enseñan cómo se ve, no dicen nada, y por eso
              van ocultas al lector de pantalla. check-i18n: no es texto
            -->
            <span
              class="nb-muestra"
              style:background={candidato.fondo}
              style:color={candidato.texto}
              aria-hidden="true">Aa</span
            >
            <span class="nb-modo-nombre">{nombreDelModo(candidato)}</span>
            {#if modo === candidato.nombre}
              <Icon nombre="comprobado" tamano={16} />
            {/if}
          </button>
        </li>
      {/each}
    </ul>
    <p class="nb-ayuda">{texto('emulacion')}</p>
  </Seccion>

  <Seccion titulo={texto('kit')}>
    <div class="nb-fila">
      <span>{texto('familia')}</span>
      <select
        class="nb-control"
        aria-label={texto('familia')}
        value={kit.familia}
        onchange={(e) => alCambiarKit('familia', e.currentTarget.value as KitAccesible['familia'])}
      >
        {#each FAMILIAS as familia (familia)}
          <option value={familia}>{texto(familia)}</option>
        {/each}
      </select>
    </div>

    {#each INTERRUPTORES as campo (campo)}
      <div class="nb-fila">
        <span>{texto(campo)}</span>
        <Switch
          etiqueta={texto(campo)}
          activo={kit[campo]}
          alCambiar={(valor) => alCambiarKit(campo, valor)}
        />
      </div>
    {/each}

    <div class="nb-fila">
      <span>{texto('animarPagina')}</span>
      <Switch
        etiqueta={texto('animarPagina')}
        activo={kit.animarPagina}
        alCambiar={(valor) => alCambiarKit('animarPagina', valor)}
      />
    </div>
    <p class="nb-ayuda">{texto('animarAyuda')}</p>
  </Seccion>
</div>

<style>
  .nb-modos {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-5);
  }

  .nb-lista {
    display: flex;
    flex-direction: column;
    gap: var(--nb-space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nb-modo {
    display: flex;
    align-items: center;
    gap: var(--nb-space-3);
    width: 100%;
    min-height: 48px;
    padding: 0 var(--nb-space-2);
    border: 1px solid transparent;
    border-radius: var(--nb-radius-md);
    background: transparent;
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
    text-align: left;
    cursor: pointer;
  }

  .nb-modo:hover {
    background: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }

  .nb-modo[aria-pressed='true'] {
    border-color: var(--m3c-primary);
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }

  .nb-muestra {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2.25rem;
    height: 2.25rem;
    border: 1px solid color-mix(in srgb, var(--m3c-outline-variant) 40%, transparent);
    border-radius: var(--nb-radius-sm);
    font-size: var(--nb-text-sm);
  }

  .nb-modo-nombre {
    flex: 1 1 auto;
  }

  .nb-fila {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--nb-space-3);
    min-height: 32px;
    font-size: var(--nb-text-sm);
  }

  .nb-control {
    padding: var(--nb-space-1) var(--nb-space-2);
    border: 1px solid var(--m3c-outline-variant);
    border-radius: var(--nb-radius-md);
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface);
    font-family: inherit;
    font-size: var(--nb-text-sm);
  }

  .nb-ayuda {
    margin: 0;
    color: var(--m3c-on-surface-variant);
    font-size: var(--nb-text-xs);
  }
</style>
