/**
 * La voz **del runtime**: el proveedor para cuando el webview no tiene ninguna.
 *
 * # Por qué hace falta un segundo proveedor del sistema
 *
 * Porque en Linux el webview no habla. WebKitGTK 2.52 declara la característica
 * `SpeechSynthesisAPI` activada por defecto y aun así `window.speechSynthesis`
 * llega **indefinido**: está en la lista de características y no en el binario.
 * Comprobado sobre el WebKitGTK 4.1 de una máquina con el sintetizador del
 * sistema instalado y funcionando.
 *
 * Ningún paquete arregla eso, porque no falta un paquete: falta la API. Así que
 * la voz sale de donde sí se puede, que es el runtime, y llega hasta aquí por el
 * puente.
 *
 * # Esto es exactamente para lo que estaba el contrato
 *
 * Se escribió un contrato de proveedor pensando en que el motor neuronal
 * entraría por él. El primero en entrar acabó siendo este, y la máquina
 * de narración no ha cambiado una línea: solo hay otro proveedor detrás.
 *
 * # Lo que no puede hacer, y por qué
 *
 * **Pausar.** La línea de órdenes de `speech-dispatcher` no la expone, así que
 * pausar corta y reanudar vuelve a empezar el bloque. Es lo mismo que ya hacía
 * la máquina al cambiar la velocidad, y es preferible a una pausa que en
 * realidad no pausa.
 *
 * **Marcar por palabra.** El fin de cada tramo sí se sabe; por dónde va dentro
 * del párrafo, no. Lo declara, y el seguimiento visual resalta el párrafo entero
 * en lugar de la palabra. Es lo mínimo del contrato, y lo dice en vez de fingir.
 */

import { onDomainEvent, voiceSpeak, voiceStatus, voiceStop } from '$core/ipc';

import type {
  DeclaracionDeVoz,
  EscuchaDeVoz,
  OpcionesDeVoz,
  ProveedorDeVoz,
  Tramo,
  VozDisponible
} from './contrato';

/**
 * Proveedor que habla por el runtime.
 *
 * Se construye vacío y se **prepara** antes de usarse: consultar al sistema es
 * asíncrono y `declarar()` no puede serlo sin que todo lo demás lo sea.
 */
export class VozDelRuntime implements ProveedorDeVoz {
  private declaracion: DeclaracionDeVoz = {
    id: 'runtime',
    disponible: false,
    motivo: 'sin-sintesis',
    necesitaRed: false,
    marcaPorPalabra: false
  };

  private catalogo: VozDisponible[] = [];

  /** Tramo que se está diciendo, para saber a quién avisar cuando termine. */
  private enCurso: { tramo: Tramo; escucha: EscuchaDeVoz } | null = null;

  private desuscribir: (() => void) | null = null;

  /**
   * Pregunta al runtime qué puede hacer y se suscribe a lo que le pase.
   *
   * Devuelve la declaración para que quien lo prepara decida si usarlo.
   */
  async preparar(): Promise<DeclaracionDeVoz> {
    try {
      const estado = await voiceStatus();
      this.catalogo = estado.voices.map((v) => ({
        id: v.id,
        nombre: v.name,
        idioma: v.language
      }));
      this.declaracion = {
        id: 'runtime',
        disponible: estado.available,
        motivo: (estado.reason as DeclaracionDeVoz['motivo']) ?? undefined,
        necesitaRed: false,
        marcaPorPalabra: false
      };
    } catch {
      // Sin puente no hay voz, y eso ya se declara. No es motivo para romper
      // nada: la lectura sigue, sin voz.
      this.declaracion = { ...this.declaracion, disponible: false };
    }

    await this.escucharAlRuntime();
    return this.declaracion;
  }

  private async escucharAlRuntime(): Promise<void> {
    if (this.desuscribir) return;
    try {
      const fin = await onDomainEvent('VoiceFinished', ({ block }) => {
        const curso = this.enCurso;
        // Un fin de un bloque que ya no se está diciendo llegó tarde: el
        // proveedor lo canceló y el aviso venía de camino.
        if (!curso || curso.tramo.bloque !== block) return;
        this.enCurso = null;
        curso.escucha.alTerminarTramo(block);
      });
      const fallo = await onDomainEvent('VoiceFailed', () => {
        const curso = this.enCurso;
        this.enCurso = null;
        curso?.escucha.alFallar('sin-voz');
      });
      this.desuscribir = () => {
        fin();
        fallo();
      };
    } catch {
      // Sin bus, la voz habla y no avisa de que terminó: se para al final de
      // cada tramo en lugar de encadenar. Peor que con bus, mejor que muda.
    }
  }

  declarar(): DeclaracionDeVoz {
    return this.declaracion;
  }

  voces(): VozDisponible[] {
    return this.catalogo;
  }

  hablar(tramo: Tramo, opciones: OpcionesDeVoz, escucha: EscuchaDeVoz): void {
    this.enCurso = { tramo, escucha };

    // Sin alineación por palabra, el tramo entero es la marca. Es lo mínimo
    // para que el seguimiento visual exista, y se emite ya para que el párrafo
    // se resalte al empezar y no al terminar.
    escucha.alAvanzar({
      bloque: tramo.bloque,
      locator: tramo.locator,
      desde: 0,
      hasta: tramo.texto.length
    });

    void voiceSpeak(tramo.bloque, tramo.texto, opciones.velocidad, opciones.voz).catch(() => {
      this.enCurso = null;
      escucha.alFallar('sin-voz');
    });
  }

  /**
   * Pausar corta.
   *
   * `speech-dispatcher` no expone pausa por su línea de órdenes. Cortar y volver
   * a empezar el bloque al reanudar es lo mismo que ya hacía la máquina al
   * cambiar la velocidad, y es preferible a una pausa que no pausa.
   */
  pausar(): void {
    this.parar();
  }

  reanudar(): void {
    // Lo reanuda quien lleva la máquina, volviendo a mandar el tramo: este
    // proveedor no guarda dónde se quedó porque el sintetizador tampoco.
  }

  parar(): void {
    this.enCurso = null;
    void voiceStop().catch(() => {
      // Parar algo que ya no suena no es un error.
    });
  }
}
