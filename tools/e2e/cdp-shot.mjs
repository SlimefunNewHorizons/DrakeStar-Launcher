// Maneja y captura la interfaz de una app Tauri en Windows (WebView2) por el protocolo de depuracion de Chrome.
//
// Uso (PowerShell):
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
//   $env:WEBVIEW2_USER_DATA_FOLDER = "$env:TEMP\ds-webview"   # perfil aislado
//   $env:DRAKESTAR_DATA_DIR = "$env:TEMP\ds-e2e"              # datos aislados
//   Start-Process .\drakestar-launcher.exe
//   node tools/e2e/cdp-shot.mjs 9333 captura.png "click:Empezar" "type:input[placeholder=Steve]@@Tester" "click:Agregar" "wait:1500"
//
// Pasos (en orden): click:<texto del boton> | type:<selector css>@@<texto> | wait:<ms> | eval:<js>
// Al final imprime el texto visible de la pagina y guarda la captura.
import { writeFileSync } from 'node:fs';

const [port = '9333', out = 'captura.png', ...steps] = process.argv.slice(2);

const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === 'page');
if (!page) throw new Error('no hay ninguna pagina; targets: ' + JSON.stringify(targets.map((t) => t.type + ' ' + t.url)));

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('no se pudo abrir el WebSocket')); });
let id = 0;
const pending = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
const send = (method, params = {}) => new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const js = async (expression) => (await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result?.result?.value;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

for (const step of steps) {
  const i = step.indexOf(':');
  const kind = step.slice(0, i);
  const arg = step.slice(i + 1);
  if (kind === 'click') {
    const ok = await js(`(() => { const b = [...document.querySelectorAll('button')].find((x) => x.textContent.includes(${JSON.stringify(arg)}) && !x.disabled); if (b) { b.click(); return true; } return false; })()`);
    console.log('click', JSON.stringify(arg), '->', ok);
    await sleep(700);
  } else if (kind === 'type') {
    const j = arg.indexOf('@@');
    const sel = arg.slice(0, j), text = arg.slice(j + 2);
    const ok = await js(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return false; e.focus(); e.value = ${JSON.stringify(text)}; e.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
    console.log('type', sel, '->', ok);
    await sleep(300);
  } else if (kind === 'wait') {
    await sleep(Number(arg));
  } else if (kind === 'eval') {
    console.log('eval ->', JSON.stringify(await js(arg)));
  } else {
    console.log('paso desconocido:', step);
  }
}
console.log('url:', page.url);
console.log('texto:', ((await js('document.body.innerText')) ?? '').replace(/\s+/g, ' ').slice(0, 500));
const shot = await send('Page.captureScreenshot', { format: 'png' });
writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
console.log('captura guardada en', out);
ws.close();
