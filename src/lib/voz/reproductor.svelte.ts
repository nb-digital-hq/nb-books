/**
 * El reproductor de lo preparado: un capítulo o un libro entero, como audiolibro.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 *
 * # Por qué vive en la página y no en el panel
 *
 * Porque el panel de voz es momentáneo -se cierra al pulsar fuera- y un audio que
 * viviera dentro dejaría de sonar en cuanto se tocara el texto. El narrador en vivo
 * vive en la página por lo mismo. Este también: se sigue escuchando con el panel
 * cerrado, y lo único que lo para es pararlo o cerrar el libro.
 *
 * # Qué hace que no hacía nada
 *
 * **Seguir el texto**: mientras suena, cada vez que empieza otro fragmento avisa con
 * su `Locator`, y la página mueve el lector ahí. **Empezar donde se va leyendo**,
 * por la marca que contiene la posición. Antes las marcas se
 * guardaban y el panel solo las contaba.
 */

import type { AudioMark, Locator, PreparedAudio, SynchronizedAudio } from '$lib/bindings';

import { enBlob, marcaDesde, marcaEn } from './preparado';

/**
 * Lo que el reproductor sabe sonar: un audio y sus marcas por `Locator`.
 *
 * Lo preparado por la voz de aquí dentro y la narración que trae el propio libro
 * tienen esta misma forma, y para quien escucha son lo mismo: audio que
 * se sigue con el texto. Un reproductor para cada uno serían dos formas de seguir
 * el texto que acabarían comportándose distinto.
 */
export type Pista = Pick<PreparedAudio, 'path' | 'marks'> | SynchronizedAudio;

export class ReproductorDePreparado {
  /** Lo que está cargado, si hay algo. */
  preparado = $state<Pista | null>(null);
  /** Si suena ahora. */
  sonando = $state(false);
  /** El fragmento que suena, o -1. */
  marca = $state(-1);
  /** Si mover el lector al fragmento que suena. De fábrica sí: es para lo que está. */
  seguir = $state(true);
  /** Segundos que lleva, para enseñarlo. */
  segundo = $state(0);
  /** Si falló al cargar o al sonar. */
  fallo = $state(false);
  /**
   * Qué hacer cuando se acaba el audio. La narración del libro va por capítulos, y
   * al acabar uno la página carga el siguiente; lo preparado no lo necesita.
   */
  alTerminar: (() => void) | null = null;

  private audio: HTMLAudioElement | null = null;
  private fuente: string | null = null;
  private alMarca: (locator: Locator) => void;
  private alEmpezar: () => void;

  /**
   * `alMarca` recibe el fragmento que empieza a sonar, para mover el lector ahí;
   * `alEmpezar` se llama al empezar a sonar, para que la página pare la narración en
   * vivo: dos voces a la vez no se entienden.
   */
  constructor(alMarca: (locator: Locator) => void, alEmpezar: () => void = () => {}) {
    this.alMarca = alMarca;
    this.alEmpezar = alEmpezar;
  }

  /**
   * Dónde está lo que suena, mientras suena: es lo que el lector resalta, con el
   * mismo mecanismo que la voz en vivo.
   */
  get sonandoEn(): Locator | null {
    if (!this.sonando || this.marca < 0) return null;
    return this.preparado?.marks[this.marca]?.locator ?? null;
  }

  /** Carga lo preparado, soltando lo anterior. No empieza a sonar. */
  async cargar(preparado: Pista): Promise<void> {
    if (this.preparado?.path === preparado.path && this.audio) return;
    this.parar();
    this.fallo = false;
    try {
      const fuente = await enBlob(preparado.path);
      const audio = new Audio(fuente);
      audio.ontimeupdate = () => this.avanzar(audio.currentTime);
      audio.onplay = () => {
        this.sonando = true;
        this.alEmpezar();
      };
      audio.onpause = () => {
        this.sonando = false;
      };
      audio.onended = () => {
        this.sonando = false;
        this.alTerminar?.();
      };
      audio.onerror = () => {
        this.fallo = true;
        this.sonando = false;
      };
      this.audio = audio;
      this.fuente = fuente;
      this.preparado = preparado;
    } catch {
      this.fallo = true;
    }
  }

  /** Suena desde donde se va leyendo. */
  desde(actual: Locator): void {
    const marcas = this.preparado?.marks ?? [];
    const indice = marcaDesde(marcas, actual);
    this.irA(marcas, indice);
  }

  alternar(): void {
    if (!this.audio) return;
    if (this.audio.paused) {
      void this.audio.play().catch(() => {
        this.fallo = true;
      });
    } else {
      this.audio.pause();
    }
  }

  pausar(): void {
    this.audio?.pause();
  }

  /** Suelta todo: el audio y la dirección temporal, que si no se queda en memoria. */
  parar(): void {
    if (this.audio) {
      this.audio.pause();
      this.audio.src = '';
    }
    if (this.fuente) URL.revokeObjectURL(this.fuente);
    this.audio = null;
    this.fuente = null;
    this.preparado = null;
    this.sonando = false;
    this.marca = -1;
    this.segundo = 0;
    this.alTerminar = null;
  }

  private irA(marcas: readonly AudioMark[], indice: number): void {
    const marca = marcas[indice];
    if (!this.audio || !marca) return;
    this.audio.currentTime = marca.startMs / 1000;
    this.marca = -1;
    void this.audio.play().catch(() => {
      this.fallo = true;
    });
  }

  private avanzar(segundos: number): void {
    this.segundo = segundos;
    const marcas = this.preparado?.marks ?? [];
    const indice = marcaEn(marcas, segundos * 1000);
    if (indice === this.marca) return;
    this.marca = indice;
    const marca = marcas[indice];
    if (this.seguir && marca) this.alMarca(marca.locator);
  }
}
