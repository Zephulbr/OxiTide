// The last output list, shared by the Settings picker and the player bar's
// quality menu (which opens at once from it and refreshes in the background).
import { api, type OutputDeviceList } from "./api";

let list = $state.raw<OutputDeviceList | null>(null);
let loading: Promise<OutputDeviceList | null> | null = null;

export const outputDevices = {
  get list(): OutputDeviceList | null {
    return list;
  },
  set(next: OutputDeviceList) {
    list = next;
  },
  /** Re-read the list; concurrent calls share one request. */
  refresh(): Promise<OutputDeviceList | null> {
    loading ??= api
      .getOutputDevices()
      .then((next) => {
        list = next;
        return next;
      })
      .catch(() => list)
      .finally(() => {
        loading = null;
      });
    return loading;
  },
};
