// Block registry: type → module with render(el, opts, ctx) → {update(state, errors), resize?(w, h), destroy?()}.
// `req` is the name for /api/v1/state?blocks=… (null: the block needs no PC data).
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
};
