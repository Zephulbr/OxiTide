import { untrack } from "svelte";

/// Open-dialog registry: the app background goes `inert` and global
/// shortcuts treat an overlay as open while `count > 0`.
export function createDialogs() {
  let count = $state(0);
  return {
    get count() {
      return count;
    },
    /// Returns the unregister cleanup; call it from an effect return. The
    /// update is untracked: `count += 1` reads `count`, which would otherwise
    /// make the registering effect depend on (and re-run from) its own write.
    register() {
      untrack(() => (count += 1));
      return () => {
        untrack(() => (count -= 1));
      };
    },
  };
}

export const dialogs = createDialogs();
