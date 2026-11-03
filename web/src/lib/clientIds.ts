// Ids for commands and events made in this browser. The device id is created once and kept; the last id is kept
// too, so after a reload the sequence continues instead of restarting at 0 (which could repeat an id the server
// has already seen and make it ignore a new command as a duplicate).
import { IdGen } from "./ids";

export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const DEVICE_KEY = "op_device";
const LAST_ID_KEY = "op_last_id";

function read(store: KeyValueStore, key: string): string | null {
  try {
    return store.getItem(key);
  } catch {
    return null;
  }
}

function write(store: KeyValueStore, key: string, value: string): void {
  try {
    store.setItem(key, value);
  } catch {
    // Storage can be unavailable (private windows); ids still work for this page load.
  }
}

export function createIdSource(store: KeyValueStore, newDeviceId: () => string = () => crypto.randomUUID()) {
  let device = read(store, DEVICE_KEY);
  if (!device) {
    device = newDeviceId();
    write(store, DEVICE_KEY, device);
  }
  const last = read(store, LAST_ID_KEY);
  const gen = IdGen.restore(device, last ? [last] : []);
  return {
    deviceId: device,
    next(nowMs: number): string {
      const id = gen.next(nowMs);
      write(store, LAST_ID_KEY, id);
      return id;
    },
  };
}
