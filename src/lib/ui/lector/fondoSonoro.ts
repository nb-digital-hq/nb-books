/**
 * Fondo sonoro sintetizado.
 *
 * # Por qué se sintetiza y no se reproduce un archivo
 *
 * Porque el criterio es que la extensión **no añada ni un byte
 * de audio** al instalador. Una grabación de lluvia de diez minutos pesa más
 * que la aplicación entera y trae una licencia que revisar.
 *
 * Lo que aporta la extensión es el nombre y **qué ambiente**. El sonido lo hace
 * el núcleo, aquí.
 *
 * # Por qué ruido pelado no basta, y qué se hizo
 *
 * La primera versión ofrecía tres ruidos —blanco, rosa y marrón— con nombres de
 * ambiente. Sonaban a lo que eran: ruido. Un ruido rosa no es lluvia; la lluvia
 * tiene **gotas**, el oleaje **sube y baja**, el viento **cambia de color**, y
 * el fuego **chasquea**. Sin eso, el nombre promete algo que el oído no
 * encuentra.
 *
 * Así que cada ambiente es ahora una **receta** con tres cosas:
 *
 * 1. Un ruido de base, que da el color general.
 * 2. **Capas de filtro**, cada una con su mezcla y, si toca, un vaivén lento
 *    que le mueve la frecuencia o el volumen. De ahí salen las olas y las
 *    ráfagas: lo que distingue el viento del siseo es que el viento respira.
 * 3. **Granos**: golpes cortos y filtrados —gotas, chispas, traqueteo— que se
 *    hornean dentro del búfer.
 *
 * # Por qué los granos van dentro del búfer y no programados
 *
 * Programarlos exigiría un planificador corriendo en el hilo principal mientras
 * suena, y eso compite con el fotograma de la lectura, que es justo lo que el
 * criterio prohíbe. Horneados en un búfer largo que se repite, el coste es
 * **cero** mientras suena: solo se paga una vez, al pulsar.
 *
 * # Por qué las capas son nodos y no se hornean también
 *
 * Porque los filtros de Web Audio corren en el hilo de audio, y un vaivén de
 * doce segundos horneado obligaría a un búfer de un minuto para que no se
 * notara el bucle. Se reparte: lo que es periódico y lento, a los nodos; lo que
 * es aleatorio y corto, al búfer.
 */

/** Los ruidos de base. Son el color, no el ambiente. */
export const CLASES = [
  'blanco',
  'rosa',
  'marron',
  'lluvia',
  'tormenta',
  'oleaje',
  'viento',
  'fuego',
  'bosque',
  'cafeteria',
  'ventilador',
  'tren'
] as const;

export type ClaseDeRuido = (typeof CLASES)[number];

/** Los tres ruidos puros, que son la base de todo lo demás. */
export type Color = 'blanco' | 'rosa' | 'marron';

/** Un vaivén lento sobre un parámetro de una capa. */
export interface Vaiven {
  /** Ciclos por segundo. Por debajo de 0,5 es lo que se oye como respiración. */
  hz: number;
  /** Cuánto se mueve, en la unidad del parámetro. */
  profundidad: number;
  sobre: 'frecuencia' | 'ganancia';
}

/** Una capa de filtro sobre el ruido de base. */
export interface Capa {
  filtro: BiquadFilterType;
  frecuencia: number;
  q?: number;
  /** Cuánto entra esta capa en la mezcla, de 0 a 1. */
  ganancia: number;
  vaiven?: Vaiven;
}

/** Golpes cortos horneados en el búfer: gotas, chispas, traqueteo. */
export interface Granos {
  /** Cuántos por segundo, en media. */
  porSegundo: number;
  /** Cuánto dura cada uno, en milisegundos. */
  duracion: number;
  /** Con qué fuerza entran, de 0 a 1. */
  ganancia: number;
  /**
   * Si van a intervalos regulares en lugar de al azar.
   *
   * El traqueteo de un tren es rítmico; una gota de lluvia no. Tratarlos igual
   * hacía que el tren sonara a lluvia con prisa.
   */
  regulares?: boolean;
}

/** Lo que hace falta para componer un ambiente. */
export interface Receta {
  base: Color;
  capas: Capa[];
  granos?: Granos;
}

/**
 * El catálogo. Cada ambiente con su receta, y ninguna receta sin ambiente.
 *
 * Los números no son decorativos: la banda de la lluvia está donde está el
 * siseo del agua sobre una superficie, el vaivén del oleaje dura entre ocho y
 * doce segundos porque es lo que dura una ola, y el traqueteo del tren va a
 * 1,4 golpes por segundo porque es la cadencia de un bogie a velocidad de
 * crucero. Se pueden discutir; lo que no se puede es que no estén escritos.
 */
export const RECETAS: Readonly<Record<ClaseDeRuido, Receta>> = {
  // --- Los tres puros, sin adornos: quien quiere ruido, quiere ruido --------
  blanco: { base: 'blanco', capas: [{ filtro: 'allpass', frecuencia: 1000, ganancia: 1 }] },
  rosa: { base: 'rosa', capas: [{ filtro: 'allpass', frecuencia: 1000, ganancia: 1 }] },
  marron: { base: 'marron', capas: [{ filtro: 'allpass', frecuencia: 1000, ganancia: 1 }] },

  // --- Y los ambientes ------------------------------------------------------
  lluvia: {
    base: 'rosa',
    capas: [
      // El cuerpo del agua cayendo: una banda ancha alrededor de 1,2 kHz.
      { filtro: 'bandpass', frecuencia: 1200, q: 0.7, ganancia: 0.75 },
      // Y el brillo de las gotas al romper, que es lo que le da cercanía.
      { filtro: 'highpass', frecuencia: 2600, ganancia: 0.28 }
    ],
    granos: { porSegundo: 55, duracion: 7, ganancia: 0.9 }
  },
  tormenta: {
    base: 'marron',
    capas: [
      // La lluvia fuerte, más grave y más cerrada que la lluvia fina.
      { filtro: 'bandpass', frecuencia: 700, q: 0.6, ganancia: 0.8 },
      // El retumbar lejano: una banda muy baja que va y viene cada veinte
      // segundos. No es un trueno -un trueno es un suceso, no un fondo-, es lo
      // que queda de él.
      {
        filtro: 'lowpass',
        frecuencia: 120,
        ganancia: 0.6,
        vaiven: { hz: 0.05, profundidad: 0.45, sobre: 'ganancia' }
      }
    ],
    granos: { porSegundo: 90, duracion: 5, ganancia: 0.75 }
  },
  oleaje: {
    base: 'marron',
    capas: [
      // Una ola tarda entre ocho y doce segundos en romper y retirarse. El
      // vaivén va sobre la frecuencia **y** el volumen: una ola no solo sube de
      // volumen, se abre y se cierra.
      {
        filtro: 'lowpass',
        frecuencia: 600,
        ganancia: 0.9,
        vaiven: { hz: 0.1, profundidad: 380, sobre: 'frecuencia' }
      },
      {
        filtro: 'bandpass',
        frecuencia: 900,
        q: 0.5,
        ganancia: 0.35,
        vaiven: { hz: 0.1, profundidad: 0.3, sobre: 'ganancia' }
      }
    ]
  },
  viento: {
    base: 'rosa',
    capas: [
      // Lo que distingue al viento del siseo es que **cambia de color**: la
      // banda se mueve mientras suena.
      {
        filtro: 'bandpass',
        frecuencia: 520,
        q: 1.4,
        ganancia: 0.95,
        vaiven: { hz: 0.07, profundidad: 300, sobre: 'frecuencia' }
      },
      {
        filtro: 'highpass',
        frecuencia: 1800,
        ganancia: 0.15,
        vaiven: { hz: 0.11, profundidad: 0.12, sobre: 'ganancia' }
      }
    ]
  },
  fuego: {
    base: 'marron',
    capas: [
      // La brasa: grave, constante y sin brillo.
      { filtro: 'lowpass', frecuencia: 380, ganancia: 0.85 },
      // Y el aire de la llama, que respira despacio.
      {
        filtro: 'bandpass',
        frecuencia: 1100,
        q: 0.8,
        ganancia: 0.3,
        vaiven: { hz: 0.23, profundidad: 0.22, sobre: 'ganancia' }
      }
    ],
    // Los chasquidos son pocos, cortos y fuertes: eso es lo que se reconoce.
    granos: { porSegundo: 7, duracion: 14, ganancia: 1 }
  },
  bosque: {
    base: 'rosa',
    capas: [
      // Hojas: banda alta y ancha, con ráfagas lentas.
      {
        filtro: 'bandpass',
        frecuencia: 2400,
        q: 0.9,
        ganancia: 0.6,
        vaiven: { hz: 0.09, profundidad: 0.35, sobre: 'ganancia' }
      },
      { filtro: 'lowpass', frecuencia: 500, ganancia: 0.35 }
    ],
    granos: { porSegundo: 18, duracion: 9, ganancia: 0.7 }
  },
  cafeteria: {
    base: 'rosa',
    capas: [
      // El murmullo humano vive donde vive la voz, filtrado por la distancia y
      // por estar de espaldas: nada por encima de 900 Hz.
      {
        filtro: 'lowpass',
        frecuencia: 900,
        ganancia: 0.9,
        vaiven: { hz: 0.3, profundidad: 0.18, sobre: 'ganancia' }
      },
      { filtro: 'bandpass', frecuencia: 300, q: 1.1, ganancia: 0.45 }
    ],
    // Loza, cucharillas y sillas: pocas, secas y agudas.
    granos: { porSegundo: 4, duracion: 6, ganancia: 0.85 }
  },
  ventilador: {
    base: 'blanco',
    capas: [
      { filtro: 'lowpass', frecuencia: 700, ganancia: 0.9 },
      // El zumbido del motor, constante y sin sorpresas: es lo que lo
      // distingue de un ruido blanco filtrado, que sería lo mismo sin alma.
      {
        filtro: 'bandpass',
        frecuencia: 220,
        q: 3.5,
        ganancia: 0.5,
        vaiven: { hz: 0.02, profundidad: 0.06, sobre: 'ganancia' }
      }
    ]
  },
  tren: {
    base: 'marron',
    capas: [
      { filtro: 'lowpass', frecuencia: 320, ganancia: 0.95 },
      {
        filtro: 'bandpass',
        frecuencia: 800,
        q: 0.7,
        ganancia: 0.28,
        vaiven: { hz: 0.13, profundidad: 0.16, sobre: 'ganancia' }
      }
    ],
    // El traqueteo es **rítmico**, y esa es toda la diferencia con la lluvia.
    granos: { porSegundo: 1.4, duracion: 26, ganancia: 0.95, regulares: true }
  }
};

/**
 * Genera un tramo de ruido en el intervalo [-1, 1].
 *
 * El rosa se aproxima con el filtro de Voss-McCartney en su forma de seis
 * polos, que da una pendiente de 3 dB por octava; el marrón es la integral del
 * blanco, acotada para que no derive.
 *
 * `azar` se inyecta para poder comprobar la forma del ruido con una secuencia
 * conocida: con `Math.random` dentro, la prueba solo podría mirar el rango.
 */
export function ruido(
  color: Color,
  muestras: number,
  azar: () => number = Math.random
): Float32Array<ArrayBuffer> {
  const salida = new Float32Array(new ArrayBuffer(muestras * 4));
  let b0 = 0;
  let b1 = 0;
  let b2 = 0;
  let b3 = 0;
  let b4 = 0;
  let b5 = 0;
  let b6 = 0;
  let anterior = 0;

  for (let i = 0; i < muestras; i += 1) {
    const blanco = azar() * 2 - 1;
    if (color === 'blanco') {
      salida[i] = blanco;
      continue;
    }
    if (color === 'marron') {
      anterior = (anterior + 0.02 * blanco) / 1.02;
      salida[i] = anterior * 3.5;
      continue;
    }
    b0 = 0.99886 * b0 + blanco * 0.0555179;
    b1 = 0.99332 * b1 + blanco * 0.0750759;
    b2 = 0.969 * b2 + blanco * 0.153852;
    b3 = 0.8665 * b3 + blanco * 0.3104856;
    b4 = 0.55 * b4 + blanco * 0.5329522;
    b5 = -0.7616 * b5 - blanco * 0.016898;
    salida[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + blanco * 0.5362) * 0.11;
    b6 = blanco * 0.115926;
  }
  return salida;
}

/**
 * Hornea los granos dentro de un búfer de ruido.
 *
 * Cada grano es una explosión corta con caída exponencial: es lo que suena a
 * gota, a chispa o a golpe según lo corto que sea y con qué fuerza entre. Se
 * **suman** al ruido en lugar de sustituirlo, porque una gota cae *sobre* la
 * lluvia, no en su lugar.
 *
 * El resultado se acota a [-1, 1] al final: sumar sin acotar satura, y saturar
 * suena a chasquido de altavoz roto, que es justo lo contrario de un fondo para
 * concentrarse.
 */
export function hornear(
  muestras: Float32Array<ArrayBuffer>,
  granos: Granos,
  tasa: number,
  azar: () => number = Math.random
): Float32Array<ArrayBuffer> {
  // Se le hace sitio a los granos **bajando la base**, no recortando la suma.
  // Medido: con la base a su nivel, las gotas de la lluvia quedaban por debajo
  // del propio ruido -cuatro por segundo pasaban del umbral, de cincuenta y
  // cinco que se hornean- y por eso sonaba a ruido con nombre de lluvia.
  // Recortar la suma habría sido peor: recortar es distorsionar.
  const sitio = 1 - Math.min(0.6, granos.ganancia * 0.6);
  for (let i = 0; i < muestras.length; i += 1) {
    muestras[i] = (muestras[i] ?? 0) * sitio;
  }

  const largo = Math.max(1, Math.round((granos.duracion / 1000) * tasa));
  const cuantos = Math.max(1, Math.round((muestras.length / tasa) * granos.porSegundo));

  for (let n = 0; n < cuantos; n += 1) {
    // Regulares para el traqueteo, al azar para todo lo demás. Un pequeño
    // desajuste en los regulares evita que suene a metrónomo, que no es lo
    // mismo que un tren.
    const inicio = granos.regulares
      ? Math.round(((n + azar() * 0.08) * muestras.length) / cuantos) % muestras.length
      : Math.floor(azar() * muestras.length);

    const fuerza = granos.ganancia * (0.55 + azar() * 0.45);
    for (let i = 0; i < largo && inicio + i < muestras.length; i += 1) {
      const caida = Math.exp((-5 * i) / largo);
      muestras[inicio + i] = (muestras[inicio + i] ?? 0) + (azar() * 2 - 1) * fuerza * caida;
    }
  }

  for (let i = 0; i < muestras.length; i += 1) {
    muestras[i] = Math.max(-1, Math.min(1, muestras[i] ?? 0));
  }
  return muestras;
}

/**
 * Cuánta energía tiene la mitad alta del espectro frente a la mitad baja.
 *
 * Es una medida cruda —la diferencia media entre muestras consecutivas frente a
 * la amplitud— pero basta para comprobar que el rosa tenga menos agudos que el
 * blanco y el marrón menos que el rosa. Sin esto, los tres podrían ser el mismo
 * ruido y las pruebas seguirían en verde.
 */
export function agudeza(muestras: Float32Array): number {
  if (muestras.length < 2) return 0;
  let diferencia = 0;
  let amplitud = 0;
  for (let i = 1; i < muestras.length; i += 1) {
    diferencia += Math.abs((muestras[i] ?? 0) - (muestras[i - 1] ?? 0));
    amplitud += Math.abs(muestras[i] ?? 0);
  }
  return amplitud === 0 ? 0 : diferencia / amplitud;
}

/**
 * Cuántas veces la media hay que superar para considerarlo un grano y no una
 * cresta del propio ruido.
 *
 * Medido: el ruido rosa tiene un factor de cresta de 3,6 y el marrón de 3,7, así
 * que un umbral de tres contaba el ruido como si fueran gotas. Con cuatro, los
 * ruidos puros dan cero y los ambientes con granos siguen dando lo suyo. Es la
 * clase de número que hay que medir antes de escribir, y la primera versión de
 * esta prueba lo había supuesto.
 */
const UMBRAL_DE_GRANO = 4;

/**
 * Cuántos picos destacan sobre el fondo, por segundo.
 *
 * Es la medida que distingue la lluvia del oleaje: la lluvia son gotas —picos
 * cortos y frecuentes— y el oleaje es una masa que sube y baja sin picos. Sin
 * medirlo, "más realista" sería una opinión.
 */
export function picos(muestras: Float32Array, tasa: number, veces = UMBRAL_DE_GRANO): number {
  if (muestras.length === 0) return 0;
  let suma = 0;
  for (const valor of muestras) suma += valor * valor;
  const eficaz = Math.sqrt(suma / muestras.length);
  if (eficaz === 0) return 0;

  const umbral = eficaz * veces;
  // Un grano es ruido, así que dentro de sus milisegundos cruza el umbral
  // muchas veces. Sin este periodo sordo, una gota contaba por cinco y el
  // número dejaba de significar "gotas por segundo".
  const sordo = Math.round(tasa * 0.01);
  let cuenta = 0;
  let ultimo = -sordo;
  for (let i = 0; i < muestras.length; i += 1) {
    if (Math.abs(muestras[i] ?? 0) > umbral && i - ultimo >= sordo) {
      cuenta += 1;
      ultimo = i;
    }
  }
  return cuenta / (muestras.length / tasa);
}

/** Lo que hace falta para parar lo que está sonando. */
export interface Sonando {
  clase: ClaseDeRuido;
  parar: () => void;
}

/** Cuánto dura el búfer que se repite. */
const SEGUNDOS_DE_BUFER = 12;

/**
 * A qué nivel eficaz se deja cada ambiente.
 *
 * Todos al mismo, y no es cosmética: medido, el ruido blanco salía a 0,58 y la
 * chimenea a 0,09, seis veces más flojo. Cambiar de uno a otro con el volumen
 * puesto habría dado un susto, que es lo contrario de un fondo para
 * concentrarse. El número deja sitio de sobra para los picos de los granos sin
 * llegar a recortar.
 */
const NIVEL = 0.15;

/** Lleva un tramo al nivel común, sin tocar su forma. */
function nivelar(muestras: Float32Array<ArrayBuffer>): Float32Array<ArrayBuffer> {
  let suma = 0;
  let pico = 0;
  for (const valor of muestras) {
    suma += valor * valor;
    pico = Math.max(pico, Math.abs(valor));
  }
  const eficaz = Math.sqrt(suma / Math.max(1, muestras.length));
  if (eficaz === 0 || pico === 0) return muestras;

  // Se respeta **el pico además del nivel**, y esa es la diferencia entre
  // nivelar y recortar. Con solo el nivel, la chimenea perdía ciento treinta y
  // seis muestras recortadas por búfer: chasquidos de más, encima de los
  // suyos. Un ambiente muy picudo se queda algo por debajo del nivel común, y
  // eso es correcto: una chimenea **es** más picuda que el viento.
  const factor = Math.min(NIVEL / eficaz, 0.98 / pico);
  for (let i = 0; i < muestras.length; i += 1) {
    muestras[i] = (muestras[i] ?? 0) * factor;
  }
  return muestras;
}

/**
 * Construye el búfer de una receta: su ruido de base con sus granos dentro.
 *
 * Aparte de [`sonar`] para poder medirlo sin navegador, que es lo que permite
 * comprobar que la lluvia tiene gotas y el oleaje no.
 */
export function buferDe(
  clase: ClaseDeRuido,
  tasa: number,
  segundos = SEGUNDOS_DE_BUFER,
  azar: () => number = Math.random
): Float32Array<ArrayBuffer> {
  const receta = RECETAS[clase];
  const muestras = ruido(receta.base, Math.round(tasa * segundos), azar);
  return nivelar(receta.granos ? hornear(muestras, receta.granos, tasa, azar) : muestras);
}

/**
 * Empieza a sonar, y devuelve con qué pararlo.
 *
 * Se pasa el contexto en lugar de crearlo aquí para que quien lo monte pueda
 * reutilizar el suyo: cada `AudioContext` es un hilo de audio del sistema, y
 * abrir uno por pulsación es la forma de quedarse sin ninguno.
 */
export function sonar(contexto: BaseAudioContext, clase: ClaseDeRuido, volumen: number): Sonando {
  const receta = RECETAS[clase];
  const bufer = contexto.createBuffer(
    1,
    contexto.sampleRate * SEGUNDOS_DE_BUFER,
    contexto.sampleRate
  );
  bufer.copyToChannel(buferDe(clase, contexto.sampleRate), 0);

  const fuente = contexto.createBufferSource();
  fuente.buffer = bufer;
  fuente.loop = true;

  const maestra = contexto.createGain();
  maestra.gain.value = Math.min(1, Math.max(0, volumen));
  maestra.connect(contexto.destination);

  const osciladores: OscillatorNode[] = [];
  const nodos: AudioNode[] = [fuente, maestra];

  for (const capa of receta.capas) {
    const filtro = contexto.createBiquadFilter();
    filtro.type = capa.filtro;
    filtro.frequency.value = capa.frecuencia;
    if (capa.q !== undefined) filtro.Q.value = capa.q;

    const mezcla = contexto.createGain();
    mezcla.gain.value = capa.ganancia;

    fuente.connect(filtro).connect(mezcla).connect(maestra);
    nodos.push(filtro, mezcla);

    if (!capa.vaiven) continue;

    // El vaivén es un oscilador lento sumado al parámetro. Va por nodos y no
    // por temporizador: así corre en el hilo de audio y no gasta fotogramas.
    const lento = contexto.createOscillator();
    lento.frequency.value = capa.vaiven.hz;
    const profundidad = contexto.createGain();
    profundidad.gain.value = capa.vaiven.profundidad;
    lento
      .connect(profundidad)
      .connect(capa.vaiven.sobre === 'frecuencia' ? filtro.frequency : mezcla.gain);
    lento.start();
    osciladores.push(lento);
    nodos.push(profundidad);
  }

  fuente.start();

  return {
    clase,
    parar: () => {
      fuente.stop();
      for (const oscilador of osciladores) oscilador.stop();
      for (const nodo of [...nodos, ...osciladores]) nodo.disconnect();
    }
  };
}
