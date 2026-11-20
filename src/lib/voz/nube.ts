/**
 * La voz **de un servicio que el usuario configura y autoriza**.
 *
 * # Por qué existe, y por qué no rompe la regla
 *
 * La regla del proyecto nunca fue "nada sale". Es **"nada sale sin
 * consentimiento explícito"**, y este proveedor es la cuarta salida que el
 * usuario concede una a una, después de los catálogos, los metadatos y la
 * consulta. Lo que sale aquí es lo que más pesa —el texto del libro, párrafo a
 * párrafo—, así que tiene propósito propio en la puerta de red y se autoriza
 * solo: tener permiso para un catálogo no da permiso para esto.
 *
 * Existe porque la voz local es la que es. `espeak-ng` es un sintetizador
 * formántico y suena a máquina de los noventa porque lo es; eso no se arregla
 * ajustando la velocidad. Quien quiera escuchar un libro entero necesita una voz
 * que aguante una hora.
 *
 * # Lo que este proveedor sí puede y el local no
 *
 * **Pausar de verdad.** El local corta y vuelve a empezar el tramo, porque la
 * síntesis del sistema no expone pausa. Aquí lo que suena es un archivo de
 * audio, así que pausar es pausar y reanudar sigue por donde iba.
 *
 * # Y apuntar a la propia máquina también vale
 *
 * El protocolo es el de `/v1/audio/speech`, que hablan tanto los servicios de
 * pago como los servidores de síntesis que se levantan en el propio equipo.
 * Apuntar el ajuste a `localhost` da voz neuronal **sin que salga nada del
 * dispositivo**, y el runtime lo permite en claro solo hacia uno mismo.
 *
 * Contrato: `docs/architecture/voice-services.md`.
 */

import { synthesizeSpeech } from '$core/ipc';

import { VozDeArchivo } from './archivo';
import type { DeclaracionDeVoz, OpcionesDeVoz, Tramo, VozDisponible } from './contrato';

export class VozEnLaNube extends VozDeArchivo {
  private declaracion: DeclaracionDeVoz = {
    id: 'nube',
    disponible: false,
    motivo: 'sin-sintesis',
    necesitaRed: true,
    marcaPorPalabra: false
  };

  /**
   * Lo deja listo si hay servicio configurado.
   *
   * No comprueba que el servicio responda: eso sería una salida a la red que
   * nadie pidió, cada vez que se abre la aplicación. Si no responde, se dirá
   * cuando se intente hablar, que es cuando el usuario está esperando algo.
   */
  preparar(hayServicio: boolean): DeclaracionDeVoz {
    this.declaracion = {
      id: 'nube',
      disponible: hayServicio,
      motivo: hayServicio ? undefined : 'sin-servicio',
      necesitaRed: true,
      marcaPorPalabra: false
    };
    return this.declaracion;
  }

  declarar(): DeclaracionDeVoz {
    return this.declaracion;
  }

  /**
   * El catálogo de voces lo tiene el servicio, no nosotros.
   *
   * Cada uno nombra las suyas a su manera y no hay forma estándar de
   * preguntárselas, así que la voz se escribe en Ajustes tal y como el servicio
   * la llame. Devolver una lista inventada sería peor que no devolver ninguna.
   */
  voces(): VozDisponible[] {
    return [];
  }

  protected pedir(tramo: Tramo, opciones: OpcionesDeVoz): Promise<string> {
    return synthesizeSpeech(tramo.texto, opciones.velocidad);
  }
}
