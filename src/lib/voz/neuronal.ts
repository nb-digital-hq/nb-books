/**
 * La voz neuronal **dentro de la aplicación**.
 *
 * # Qué la diferencia de las otras dos
 *
 * De la del sistema, que suena a máquina de los noventa porque `espeak-ng` es un
 * sintetizador formántico, y eso no se arregla ajustando la velocidad. De la de
 * un servicio, que suena bien y **manda el texto del libro a otro ordenador**.
 * Esta suena como la de un servicio y no sale del dispositivo.
 *
 * # Por qué puede no ofrecerse aunque esté instalada
 *
 * Porque lo que decide no es si tarda mucho, sino si **tarda más de lo que dura
 * lo que dice**. El runtime lo mide al cargar el modelo —sintetiza una frase
 * corta y calcula su factor de tiempo real— y si la máquina no llega, esto se
 * declara no disponible con su motivo. Medido en la máquina de desarrollo: 1,67,
 * así que ahí no se ofrece narrar en vivo. Ver `local-models.md` sección 7.3.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import { synthesizeNeuralSpeech } from '$core/ipc';
import { neuralVoice } from '$core/ipc/voz';

import { VozDeArchivo } from './archivo';
import type { DeclaracionDeVoz, OpcionesDeVoz, Tramo, VozDisponible } from './contrato';

/** Lo que se declara mientras no se ha preguntado al runtime. */
const SIN_PREGUNTAR: DeclaracionDeVoz = {
  id: 'neuronal',
  disponible: false,
  motivo: 'sin-recursos',
  necesitaRed: false,
  marcaPorPalabra: false
};

export class VozNeuronalLocal extends VozDeArchivo {
  private declaracion: DeclaracionDeVoz = SIN_PREGUNTAR;

  private catalogo: VozDisponible[] = [];

  /**
   * Le pregunta al runtime qué hay, y se queda con la respuesta.
   *
   * **Puede tardar unos segundos la primera vez**: si el modelo está instalado y
   * cabe, el runtime lo carga y mide la máquina. Se hace aquí y no al arrancar
   * la aplicación porque cargar 600 MB para saber si alguien va a querer
   * escuchar sería cobrarle el precio a quien nunca lo va a usar.
   */
  async preparar(): Promise<DeclaracionDeVoz> {
    try {
      const vista = await neuralVoice();
      this.catalogo = vista.voices.map((voz) => ({
        id: voz.id,
        nombre: voz.name,
        idioma: voz.language
      }));
      // **Lenta no es no disponible** ahora: la máquina la mueve,
      // solo que más despacio de lo que dura lo que dice. Se ofrece y se avisa,
      // porque recomendar no es obligar.
      const lenta = vista.reason === 'DEMASIADO_LENTA';
      const disponible = vista.ready || lenta;
      this.declaracion = {
        id: 'neuronal',
        disponible,
        // Los motivos del runtime son suyos; aquí se traducen a los del
        // contrato, que son los que la interfaz sabe explicar. `NO_INSTALADA` es
        // lo que se resuelve instalando, y por eso no es `sin-sintesis`: hay
        // síntesis, falta el modelo.
        motivo: disponible ? undefined : 'sin-recursos',
        advertencia: lenta ? 'lenta' : undefined,
        factor: vista.factor ?? undefined,
        necesitaRed: false,
        marcaPorPalabra: false
      };
    } catch {
      this.catalogo = [];
      this.declaracion = SIN_PREGUNTAR;
    }
    return this.declaracion;
  }

  declarar(): DeclaracionDeVoz {
    return this.declaracion;
  }

  /** Las del catálogo del modelo. Hoy, las tres de español. */
  voces(): VozDisponible[] {
    return this.catalogo;
  }

  protected pedir(tramo: Tramo, opciones: OpcionesDeVoz): Promise<string> {
    return synthesizeNeuralSpeech(tramo.texto, opciones.voz ?? null, opciones.velocidad);
  }
}
