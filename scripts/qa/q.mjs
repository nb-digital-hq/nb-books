// Ejecuta una o varias expresiones contra window.__qa (inyectándolo si hace falta).
// Uso: node scripts/qa/q.mjs '<expr>' ['<expr>'...] [--captura nombre]
import { readFileSync, writeFileSync } from 'node:fs';
import { Inspector } from './inspector.mjs';
const args = process.argv.slice(2);
let captura = null; const i = args.indexOf('--captura'); if (i >= 0) { captura = args[i + 1]; args.splice(i, 2); }
const insp = await Inspector.conectar();
try {
  await insp.evaluarDirecto(readFileSync(new URL('./sonda.js', import.meta.url), 'utf8'));
  for (const e of args) {
    const t0 = performance.now();
    let v; try { v = await insp.evaluar(e); } catch (err) { v = { EXCEPCION: err.message }; }
    console.log(`## ${e.slice(0, 100)}  (${Math.round(performance.now() - t0)} ms)\n` + String(JSON.stringify(v, null, 0)).replace(/\},\{/g, '},\n{'));
  }
  if (captura) {
    const { width, height } = await insp.evaluar('({width: innerWidth, height: innerHeight})');
    const r = await insp.enviar('Page.snapshotRect', { x: 0, y: 0, width, height, coordinateSystem: 'Viewport' });
    const f = `${process.env.HOME}/.cache/nb-books-qa/capturas/${captura}.png`;
    writeFileSync(f, Buffer.from(r.dataURL.split(',')[1], 'base64')); console.log('captura: ' + f);
  }
} finally { insp.cerrar(); }
