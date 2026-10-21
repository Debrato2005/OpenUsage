import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { waitFor } from './webdriver.mjs';

export async function pasteNative(driver, fixture, selector, text) {
  assert.equal(process.env.OPENUSAGE_NATIVE_PRIVATE_XVFB, '1', 'Clipboard checks require an explicitly private Xvfb display.');
  assert.match(fixture.environment.DISPLAY ?? '', /^:\d+(?:\.\d+)?$/);
  assert.ok(Buffer.byteLength(text) <= 1024);
  const child = spawn('python3', ['-I', '-c', `
import sys
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('Gdk', '3.0')
from gi.repository import Gtk, Gdk, GLib
assert Gtk.init_check()[0], 'No private display.'
text = sys.stdin.buffer.read(1025)
assert len(text) <= 1024
clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
clipboard.set_text(text.decode('utf-8'), -1)
print('clipboard-ready', flush=True)
GLib.timeout_add_seconds(10, Gtk.main_quit)
Gtk.main()
`], { env: fixture.environment, stdio: ['pipe', 'pipe', 'pipe'] });
  let ready = '';
  let failure;
  const errors = [];
  child.stdout.on('data', chunk => { ready += chunk.toString(); });
  child.stderr.on('data', chunk => errors.push(chunk));
  child.once('error', error => { failure = error; });
  child.stdin.once('error', error => { failure = error; });
  child.once('exit', code => { failure = new Error(`Private clipboard owner exited: ${code}. ${Buffer.concat(errors).toString()}`); });
  child.stdin.end(text);
  try {
    await waitFor(() => { if (failure) throw failure; return ready.includes('clipboard-ready'); }, 'private GTK clipboard owner');
    await driver.click(selector);
    await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'native-clipboard-paste', actions: [
      { type: 'keyDown', value: '\uE009' }, { type: 'keyDown', value: 'v' },
      { type: 'keyUp', value: 'v' }, { type: 'keyUp', value: '\uE009' },
    ] }] });
    await waitFor(async () => { if (failure) throw failure; return driver.execute('return document.querySelector(arguments[0])?.value===arguments[1];', [selector, text]); }, 'native clipboard paste into the real input');
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = new Promise(done => child.once('exit', done));
      child.kill('SIGTERM');
      await Promise.race([exited, delay(1000)]);
      if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await exited; }
    }
  }
}
