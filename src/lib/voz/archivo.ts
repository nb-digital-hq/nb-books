/**
 * Lo que comparten las voces que **producen un archivo** en lugar de hablar.
 *
 * # Por qué existe esta clase
 *
 * Porque hay dos —la de un servicio y la neuronal local— y lo
 * que las diferencia es una línea: a quién le piden el audio. Todo lo demás es
 * idéntico y **nada de ello es obvio**: la generación que invalida lo que llega
 * tarde, soltar el blob para no dejar cada párrafo del libro en memoria, y el
 * rodeo de WebKitGTK que está explicado abajo. Duplicarlo sería duplicar tres
 * decisiones que costaron medirse, y la copia se quedaría atrás a la primera.
 *
 * # El rodeo de WebKitGTK, que no es un rodeo
 *
 * **El audio se trae con `fetch` y se vuelve a servir como blob**, en lugar de
 * enchufarle al elemento la dirección del archivo. WebKitGTK, que es el motor
 * real en Linux, no reproduce medios servidos por el esquema propio de los
 * recursos de la aplicación: las imágenes sí, porque las decodifica él; el audio
 * se lo entrega a GStreamer, que no sabe hablar ese esquema y responde que la
 * fuente no está soportada (`MediaError` 4). Medido: por la dirección directa
 * `play()` es rechazado, y como blob suena entero.
 *
 * Traerlo con `fetch` lo pide el motor, que sí sabe el esquema, así que el audio
 * no cruza el puente ni se codifica en base64: sigue viniendo del archivo, solo
 * que por otra puerta.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import { rutaDeImagen } from '$core/ipc';

import type {
  DeclaracionDeVoz,
  EscuchaDeVoz,
  OpcionesDeVoz,
  ProveedorDeVoz,
  Tramo,
  VozDisponible
} from './contrato';

export abstract class VozDeArchivo implements ProveedorDeVoz {
  /** Lo que suena ahora. Uno solo: dos audios a la vez no se entienden. */
  private audio: HTMLAudioElement | null = null;

  /**
   * La dirección temporal del audio que suena, para poder soltarla.
   *
   * Un `blob:` vive hasta que alguien lo revoca o hasta que se cierra la
   * pestaña, y aquí no se cierra ninguna: sin soltarlo, cada frase de cada libro
   * dejaría su audio en memoria para siempre.
   */
  private fuente: string | null = null;

  /**
   * En qué petición vamos.
   *
   * Sintetizar tarda, y en ese hueco se puede haber pulsado parar o haber pasado
   * al tramo siguiente. Sin esto, el audio que llegara tarde empezaría a sonar
   * encima de lo que hubiera.
   */
  private generacion = 0;

  /** Le pide el audio a quien sea, y devuelve la ruta del archivo. */
  protected abstract pedir(tramo: Tramo, opciones: OpcionesDeVoz): Promise<string>;

  abstract declarar(): DeclaracionDeVoz;

  abstract voces(): VozDisponible[];

  hablar(tramo: Tramo, opciones: OpcionesDeVoz, escucha: EscuchaDeVoz): void {
    this.parar();
    const mia = ++this.generacion;

    // El tramo entero es la marca: estos proveedores no alinean por palabra, y
    // lo declaran. Se emite ya para que el párrafo se resalte mientras se
    // sintetiza y no cuando empieza a sonar.
    escucha.alAvanzar({
      bloque: tramo.bloque,
      locator: tramo.locator,
      desde: 0,
      hasta: tramo.texto.length
    });

    void this.pedir(tramo, opciones)
      .then(async (ruta) => {
        // Llegó tarde: alguien paró o pasó de tramo mientras se sintetizaba.
        if (mia !== this.generacion) return;

        const respuesta = await fetch(rutaDeImagen(ruta));
        const blob = await respuesta.blob();
        if (mia !== this.generacion) return;

        const fuente = URL.createObjectURL(blob);
        const audio = new Audio(fuente);
        this.audio = audio;
        this.fuente = fuente;
        audio.onended = () => {
          if (mia !== this.generacion) return;
          this.soltar();
          escucha.alTerminarTramo(tramo.bloque);
        };
        audio.onerror = () => {
          if (mia !== this.generacion) return;
          this.soltar();
          escucha.alFallar('sin-voz');
        };
        void audio.play().catch(() => {
          if (mia === this.generacion) escucha.alFallar('sin-voz');
        });
      })
      .catch(() => {
        if (mia === this.generacion) escucha.alFallar('sin-voz');
      });
  }

  /**
   * Pide el audio del tramo siguiente y no hace nada con él.
   *
   * Parece tirar el resultado y no lo tira: el runtime guarda cada síntesis en
   * su caché indexada por el contenido de la petición, así que cuando llegue el
   * turno de ese tramo el archivo ya está. Es la diferencia entre escuchar
   * seguido y escuchar con un silencio entre frases.
   *
   * **Ni informa ni interrumpe.** No toca la generación ni el audio: si falla o
   * llega tarde, cuando toque se pedirá otra vez por la vía normal, y quien
   * escucha no se entera de nada.
   */
  adelantar(tramo: Tramo, opciones: OpcionesDeVoz): void {
    void this.pedir(tramo, opciones).catch(() => {
      // Adelantar es una mejora, no una obligación: su fallo no es del usuario.
    });
  }

  /** Pausar pausa: lo que suena es un archivo, no una síntesis en curso. */
  pausar(): void {
    this.audio?.pause();
  }

  reanudar(): void {
    void this.audio?.play().catch(() => {
      // Reanudar algo que el navegador ya soltó no es un error que merezca
      // interrumpir la lectura: se queda parada y se vuelve a pulsar.
    });
  }

  parar(): void {
    // La generación se invalida **antes** de soltar el audio: al revés, una
    // síntesis en vuelo se vería a sí misma como vigente y empezaría a sonar
    // después de haber parado.
    this.generacion += 1;
    const audio = this.audio;
    this.soltar();
    if (!audio) return;
    audio.pause();
    audio.onended = null;
    audio.onerror = null;
    // Se suelta la fuente para que el motor libere el audio en lugar de dejarlo
    // cargado por si acaso.
    audio.removeAttribute('src');
    audio.load();
  }

  /** Olvida el audio que sonaba y devuelve la memoria de su blob. */
  private soltar(): void {
    this.audio = null;
    if (this.fuente === null) return;
    URL.revokeObjectURL(this.fuente);
    this.fuente = null;
  }
}
