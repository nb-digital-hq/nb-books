// Evalúa una expresión en la aplicación auditada y la imprime en JSON.
// Uso: node scripts/qa/ev.mjs '<expresión>' [captura.png]
import { writeFileSync } from 'node:fs';
import { Inspector } from './inspector.mjs';

const [expresion, captura] = process.argv.slice(2);
const insp = await Inspector.conectar();
try {
  if (expresion) {
    const t0 = performance.now();
    const valor = await insp.evaluar(expresion);
    const ms = Math.round(performance.now() - t0);
    console.log(JSON.stringify(valor, null, 1));
    console.error(`(${ms} ms)`);
  }
  if (captura) {
    const { width, height } = await insp.evaluar('({width: innerWidth, height: innerHeight})');
    const r = await insp.enviar('Page.snapshotRect', { x: 0, y: 0, width, height, coordinateSystem: 'Viewport' });
    writeFileSync(captura, Buffer.from(r.dataURL.split(',')[1], 'base64'));
  }
} finally {
  insp.cerrar();
}
