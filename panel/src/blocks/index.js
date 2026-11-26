// Block registry: type → module with render(el, opts, ctx) → {update(state, errors), resize?(w, h), destroy?()}.
// `req` is the name for /api/v1/state?blocks=… (null: the block needs no PC data), or a function
// (opts, layout) → name | null when it depends on the block's settings (keys: audio only for mic / audioOut).
import * as clock from './clock.js';
import * as weather from './weather.js';
import * as cpu from './cpu.js';
import * as gpu from './gpu.js';
import * as ram from './ram.js';
import * as disks from './disks.js';
import * as net from './net.js';
import * as ai from './ai.js';
import * as apps from './apps.js';
import * as media from './media.js';
import * as servers from './servers.js';
import * as phone from './phone.js';
import * as power from './power.js';
import * as keys from './keys.js';
import * as mixer from './mixer.js';
import * as obs from './obs.js';
import * as notify from './notify.js';

export const BLOCKS = {
  clock: { mod: clock, req: null },
  weather: { mod: weather, req: null },
  cpu: { mod: cpu, req: 'cpu' },
  gpu: { mod: gpu, req: 'gpu' },
  ram: { mod: ram, req: 'ram' },
  disks: { mod: disks, req: 'disks' },
  net: { mod: net, req: 'net' },
  ai: { mod: ai, req: 'ai' },
  apps: { mod: apps, req: null },
  media: { mod: media, req: 'media' },
  servers: { mod: servers, req: 'servers' },
  phone: { mod: phone, req: null },
  power: { mod: power, req: null },
  keys: { mod: keys, req: keys.req },
  mixer: { mod: mixer, req: 'audio' },
  obs: { mod: obs, req: 'obs' },
  notify: { mod: notify, req: 'notify' },
};

/** The state key a block needs with these settings, or null. */
export function reqOf(def, opts, layout) {
  return typeof def.req === 'function' ? def.req(opts || {}, layout || {}) : def.req;
}
