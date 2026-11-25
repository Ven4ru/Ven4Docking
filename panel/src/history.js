// Graph history per data key ("net.down", "gpu:<id>.load"), kept in memory for the page's lifetime.
// Blocks are rebuilt when the layout version changes; a block that is still there picks up its
// graph where it was instead of starting empty. One value per state: several blocks showing the
// same key (or a state re-applied after a resize) do not add duplicate points.
import { pushHist } from './util.js';

const store = {};

/** Adds `value` for `key` once per state object `stamp`; returns the history array (oldest first). */
export function track(key, value, stamp, n) {
  let s = store[key];
  if (!s) { s = { v: [], stamp: null }; store[key] = s; }
  if (stamp !== s.stamp) {
    s.stamp = stamp;
    pushHist(s.v, value, n || 60);
  }
  return s.v;
}
