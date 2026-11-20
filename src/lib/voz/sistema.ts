/**
 * La voz del sistema: el primer proveedor del contrato.
 *
 * # Por qué esta y no una mejor
 *
 * Porque es la que **está siempre**, o casi. Leer en voz alta es accesibilidad, no
 * calidad: no puede depender de que el equipo aguante un modelo ni de que exista
 * una voz neuronal en el idioma del documento. El motor neuronal entra
 * por el mismo contrato y el usuario elige; la del sistema no se sustituye
 * nunca, es el suelo.
 *
 * # "O casi": el caso que este archivo existe para no disimular
 *
 * **WebKitGTK no siempre trae síntesis de voz.** Es el webview de Linux, y ahí
 * `speechSynthesis` puede no existir, o existir y no tener ni una voz instalada.
 * Las dos situaciones se declaran con su motivo en lugar de enseñar un botón que
 * no hace nada, que es exactamente el criterio de aceptación.
 *
 * # De dónde salen las marcas
 *
 * Del evento `boundary`, que el navegador emite al empezar cada palabra con el
 * desplazamiento en caracteres dentro de lo que se está diciendo. No todas las
 * implementaciones lo emiten: cuando no llega ninguno, el tramo se marca entero
 * al empezar, que es lo mínimo para resaltar por dónde va la lectura.
 */

import type {
  DeclaracionDeVoz,
  EscuchaDeVoz,
  OpcionesDeVoz,
  ProveedorDeVoz,
  Tramo,
  VozDisponible
} from './contrato';

/** La API del webview, o `null` donde no exista. */
function sintesis(): SpeechSynthesis | null {
  // `globalThis` y no `window`: este módulo se importa también desde pruebas
  // que corren sin navegador, y ahí `window` no existe.
  const api = (globalThis as { speechSynthesis?: SpeechSynthesis }).speechSynthesis;
  return api ?? null;
}

function constructorDeEnunciado(): typeof SpeechSynthesisUtterance | null {
  const ctor = (globalThis as { SpeechSynthesisUtterance?: typeof SpeechSynthesisUtterance })
    .SpeechSynthesisUtterance;
  return ctor ?? null;
}

/**
 * El proveedor de la síntesis del sistema.
 *
 * Es una clase y no un objeto suelto porque tiene que recordar qué está diciendo
 * para poder pausarlo: `speechSynthesis` es global y compartida, así que el
 * proveedor lleva su propia cuenta en lugar de fiarse de lo que la API diga de
 * sí misma, que en algunas plataformas se queda desincronizado tras pausar.
 */
export class VozDelSistema implements ProveedorDeVoz {
  /** El enunciado en curso, para saber si un evento es nuestro. */
  private enCurso: SpeechSynthesisUtterance | null = null;

  declarar(): DeclaracionDeVoz {
    const api = sintesis();
    if (!api || !constructorDeEnunciado()) {
      return {
        id: 'sistema',
        disponible: false,
        motivo: 'sin-sintesis',
        necesitaRed: false,
        marcaPorPalabra: true
      };
    }

    // Una síntesis sin ninguna voz instalada no habla. Ocurre en instalaciones
    // mínimas de Linux, y decir "no hay voces" es información distinta de "esta
    // plataforma no tiene síntesis": la primera se arregla instalando una.
    if (api.getVoices().length === 0) {
      return {
        id: 'sistema',
        disponible: false,
        motivo: 'sin-voces',
        necesitaRed: false,
        marcaPorPalabra: true
      };
    }

    return { id: 'sistema', disponible: true, necesitaRed: false, marcaPorPalabra: true };
  }

  voces(): VozDisponible[] {
    const api = sintesis();
    if (!api) return [];
    return api.getVoices().map((voz) => ({
      id: voz.voiceURI,
      nombre: voz.name,
      idioma: voz.lang,
      porDefecto: voz.default
    }));
  }

  hablar(tramo: Tramo, opciones: OpcionesDeVoz, escucha: EscuchaDeVoz): void {
    const api = sintesis();
    const Enunciado = constructorDeEnunciado();
    if (!api || !Enunciado) {
      escucha.alFallar('sin-voz');
      return;
    }

    // Cancelar antes de encolar: sin esto, dos tramos seguidos se dicen a la vez
    // en algunas plataformas, porque `speak` encola en lugar de sustituir.
    api.cancel();

    const enunciado = new Enunciado(tramo.texto);
    enunciado.rate = opciones.velocidad;

    const elegida = api.getVoices().find((v) => v.voiceURI === opciones.voz);
    if (elegida) {
      enunciado.voice = elegida;
      // El idioma va con la voz: dejarlo suelto hace que algunas plataformas
      // pronuncien el texto con las reglas del idioma del sistema.
      enunciado.lang = elegida.lang;
    }

    let hubieraMarca = false;

    enunciado.onboundary = (evento: SpeechSynthesisEvent) => {
      if (this.enCurso !== enunciado) return;
      hubieraMarca = true;
      const desde = evento.charIndex ?? 0;
      // `charLength` no lo emiten todas las implementaciones. Sin él, la marca
      // llega hasta el final del tramo, que resalta de más pero nunca de menos:
      // perder de vista dónde va la voz es peor que resaltar un poco más.
      const largo = evento.charLength ?? tramo.texto.length - desde;
      escucha.alAvanzar({
        bloque: tramo.bloque,
        locator: tramo.locator,
        desde,
        hasta: desde + largo
      });
    };

    enunciado.onend = () => {
      if (this.enCurso !== enunciado) return;
      this.enCurso = null;
      escucha.alTerminarTramo(tramo.bloque);
    };

    enunciado.onerror = (evento: SpeechSynthesisErrorEvent) => {
      if (this.enCurso !== enunciado) return;
      this.enCurso = null;
      // Cancelar cuenta como error en esta API, y no lo es: es lo que pasa
      // cuando el usuario para. Distinguirlo evita enseñar un fallo por haber
      // pulsado el botón de parar.
      escucha.alFallar(evento.error === 'canceled' ? 'interrumpida' : 'desconocido');
    };

    enunciado.onstart = () => {
      if (this.enCurso !== enunciado) return;
      // Sin alineación por palabra, el tramo entero es la marca. Es lo mínimo
      // para que el seguimiento visual exista en todas las plataformas.
      if (!hubieraMarca) {
        escucha.alAvanzar({
          bloque: tramo.bloque,
          locator: tramo.locator,
          desde: 0,
          hasta: tramo.texto.length
        });
      }
    };

    this.enCurso = enunciado;
    api.speak(enunciado);
  }

  pausar(): void {
    sintesis()?.pause();
  }

  reanudar(): void {
    sintesis()?.resume();
  }

  parar(): void {
    this.enCurso = null;
    sintesis()?.cancel();
  }
}
