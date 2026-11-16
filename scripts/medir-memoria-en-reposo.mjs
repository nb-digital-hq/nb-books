// RAM en reposo del binario de release, **y el suelo del motor medido a la vez**.
// Mide el presupuesto de la sección 1 de
// `docs/architecture/performance-budgets.md`; los resultados están en
// sus secciones 4.13 y 4.22.
//
//   pnpm tauri build   # el binario con la interfaz dentro; `cargo build` a secas no la lleva
//   node scripts/medir-memoria-en-reposo.mjs
//
// # Lo que se vigila
//
// **No hay techo absoluto**: no se arriesga una función ni el aspecto por caber en un
// techo propio, y el total lo mueve sobre todo el motor. Lo que se vigila es **lo
// nuestro**: lo que NB Books gasta por encima del suelo del motor, que es lo que gasta
// WebKitGTK pintando una sola línea (`scripts/suelo-de-webkitgtk.py`). El suelo se mide
// **en la misma ejecución**, justo antes que la aplicación: cambia con cada versión de
// WebKitGTK y de los controladores gráficos -219 MB un día, 243 el siguiente-, y
// comparar contra un suelo de otro día confundiría eso con una regresión nuestra, o la
// escondería.
// Se miden uno detrás de otro y nunca a la vez: PSS reparte las páginas compartidas
// entre quien las tiene abiertas, y con los dos vivos cada uno saldría más barato.
//
// **No está en `pnpm verify`**, y no por gusto: lanza la aplicación de verdad, así
// que necesita una pantalla y el binario de release compilado. Solo corre en Linux,
// porque lee `/proc`; en Windows y macOS habría que preguntarle al sistema de otra
// forma, y aquí no hay ninguna de las dos para comprobarlo.
//
// La aplicación se lanza con NB_BOOKS_TEST_DIR en un directorio temporal, así que no
// toca el perfil del usuario. Se espera a que el runtime diga que la base está lista
// -el suelo, a que diga `listo`-, se deja asentar y se suma la memoria proporcional
// (PSS) del proceso y de todos sus descendientes, que en Linux son los procesos de
// WebKit. PSS y no RSS: las bibliotecas compartidas se cuentan una vez repartidas, en
// lugar de sumar la pila de GTK entera por cada proceso. Del desglose salen además lo
// anónimo -montículo y pilas, que es memoria propia- y lo mapeado de archivos.
//
// Sale con código 1 si lo nuestro pasa de la referencia en más de `MARGEN_MB`. No es un
// techo: es un aviso de que algo nuestro creció sin que nadie lo decidiera. Si lo
// explica algo decidido, se sube la referencia con su motivo en la sección 4.22; si
// no, se arregla. Nunca se quita una función para volver a caber.
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..');
const BINARIO = join(RAIZ, 'src-tauri/target/release/nb-books');
const SUELO = join(RAIZ, 'scripts/suelo-de-webkitgtk.py');
/** Lo nuestro medido el 2026-09-24 (performance-budgets.md, sección 4.22). */
const REFERENCIA_NUESTRO_MB = 80;
/** Más que la variación entre vueltas de la misma medida, que fue de cinco. */
const MARGEN_MB = 10;
const CAMPOS = ['Pss', 'Pss_Anon', 'Pss_File', 'Private_Dirty'];
const esperar = (ms) => new Promise((r) => setTimeout(r, ms));

const hijos = (pid) => {
  const directos = [];
  for (const entrada of readdirSync('/proc')) {
    if (!/^\d+$/.test(entrada)) continue;
    try {
      const estado = readFileSync(`/proc/${entrada}/stat`, 'utf8');
      const ppid = Number(estado.slice(estado.lastIndexOf(')') + 2).split(' ')[1]);
      if (ppid === pid) directos.push(Number(entrada));
    } catch {}
  }
  return directos.flatMap((h) => [h, ...hijos(h)]);
};

const leer = (pid) => {
  const salida = { pid };
  try {
    for (const linea of readFileSync(`/proc/${pid}/smaps_rollup`, 'utf8').split('\n')) {
      const [clave, valor] = linea.split(/:\s+/);
      if (CAMPOS.includes(clave)) salida[clave] = Math.round(Number.parseInt(valor, 10) / 1024);
    }
    salida.nombre = readFileSync(`/proc/${pid}/comm`, 'utf8').trim();
  } catch {}
  return salida;
};

const muestra = (pid) => {
  const procesos = [pid, ...hijos(pid)].map(leer).filter((p) => p.nombre && p.Pss > 0);
  return { totalMb: procesos.reduce((s, p) => s + p.Pss, 0), procesos };
};

const mediana = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

/** Lanza, espera a que esté lista, deja asentar y toma tres muestras a los 20 s y tres a los 60. */
async function medir(comando, argumentos, entorno, listo) {
  const proceso = spawn(comando, argumentos, {
    env: { ...process.env, ...entorno },
    stdio: ['ignore', 'pipe', 'ignore']
  });
  let salida = '';
  proceso.stdout.on('data', (trozo) => {
    salida += trozo;
  });
  try {
    const limite = Date.now() + 30000;
    while (Date.now() < limite && !listo(salida)) await esperar(50);
    if (!listo(salida)) throw new Error(`${comando} no terminó de arrancar`);

    const series = { a20s: [], a60s: [] };
    await esperar(20000);
    for (let i = 0; i < 3; i += 1) {
      if (i > 0) await esperar(5000);
      series.a20s.push(muestra(proceso.pid).totalMb);
    }
    await esperar(30000);
    for (let i = 0; i < 3; i += 1) {
      if (i > 0) await esperar(5000);
      series.a60s.push(muestra(proceso.pid).totalMb);
    }
    return { series, desglose: muestra(proceso.pid).procesos };
  } finally {
    proceso.kill();
    await esperar(1500);
  }
}

/*
 * **El binario tiene que llevar la interfaz dentro.** `cargo build --release` a secas
 * compila sin la función `custom-protocol` de Tauri, y ese binario busca la interfaz
 * en el servidor de desarrollo: sin él, la ventana se queda en una página de error
 * que gasta lo mismo que el suelo. Así se midieron 230 MB el 2026-09-23 y parecía
 * que lo nuestro eran once. Las rutas de los recursos empotrados van en claro en el
 * binario; si no están, no se mide nada.
 */
if (!readFileSync(BINARIO).includes('/_app/immutable/')) {
  console.error(`${BINARIO} no lleva la interfaz dentro: compílalo con \`pnpm tauri build\`.`);
  process.exit(2);
}

const suelo = await medir('python3', [SUELO], {}, (s) => s.includes('listo'));

const dir = mkdtempSync(join(tmpdir(), 'nb-memoria-'));
let nb;
try {
  nb = await medir(BINARIO, [], { NB_BOOKS_TEST_DIR: dir, NB_BOOKS_LOG: 'info' }, () => {
    const logs = join(dir, 'logs');
    return (
      existsSync(logs) &&
      readdirSync(logs).some((f) =>
        readFileSync(join(logs, f), 'utf8').includes('base de datos lista')
      )
    );
  });
} finally {
  rmSync(dir, { recursive: true, force: true });
}

const total = mediana(nb.series.a60s);
const nuestro = total - mediana(suelo.series.a60s);
const crecio = nuestro > REFERENCIA_NUESTRO_MB + MARGEN_MB;
console.log(
  JSON.stringify(
    {
      fecha: new Date().toISOString(),
      suelo,
      nbBooks: nb,
      a60s: {
        totalMb: total,
        sueloMb: mediana(suelo.series.a60s),
        nuestroMb: nuestro,
        referenciaNuestroMb: REFERENCIA_NUESTRO_MB,
        crecio
      }
    },
    null,
    1
  )
);
if (crecio) {
  console.error(
    `Lo nuestro gasta ${nuestro} MB sobre el suelo, ${nuestro - REFERENCIA_NUESTRO_MB} más que la referencia: mira qué lo explica.`
  );
}
process.exitCode = crecio ? 1 : 0;
