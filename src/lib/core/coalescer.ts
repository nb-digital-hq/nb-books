/**
 * Junta las peticiones repetidas de una misma relectura en una sola.
 *
 * Mientras se importa, el runtime anuncia lo que entra cada pocos documentos, y
 * Inicio se releía entero con cada anuncio: siete vueltas en cuatro segundos,
 * con fotogramas de hasta 330 ms. Con esto, una relectura en marcha absorbe las
 * que lleguen mientras dura, y como mucho se hace una más al terminar, pasada la
 * pausa. Lo que el usuario ve sigue siendo lo último; lo que se ahorra es pedir
 * lo mismo diez veces.
 */
export function coalescer(tarea: () => Promise<void>, pausaMs: number): () => Promise<void> {
  let enMarcha: Promise<void> | null = null;
  let otraPendiente = false;
  const correr = async (): Promise<void> => {
    if (enMarcha) {
      otraPendiente = true;
      return enMarcha;
    }
    enMarcha = (async () => {
      try {
        await tarea();
      } finally {
        if (otraPendiente) {
          otraPendiente = false;
          await new Promise((r) => setTimeout(r, pausaMs));
          await tarea().catch(() => undefined);
        }
        enMarcha = null;
      }
    })();
    return enMarcha;
  };
  return correr;
}

/**
 * Deja pasar una llamada como mucho cada `intervaloMs`.
 *
 * Entrar en Inicio o en la Biblioteca pedía repasar las 19.350 carpetas de la
 * biblioteca cada vez, de 250 a 420 ms de runtime ocupado por visita.
 */
export function espaciar(tarea: () => void, intervaloMs: number): () => void {
  let ultima = Number.NEGATIVE_INFINITY;
  return () => {
    const ahora = Date.now();
    if (ahora - ultima < intervaloMs) return;
    ultima = ahora;
    tarea();
  };
}
