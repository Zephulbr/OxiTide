/// The Collaborators dialog's target (spec 2026-10-22, §6.3). Any surface
/// opens it; one host in App renders it, so the sidebar tree, the Rack tree,
/// the Playlists page and the playlist page share it.
export interface CollaboratorsTarget {
  uuid: string;
  title: string;
}

function createCollaboratorsDialog() {
  let target = $state<CollaboratorsTarget | null>(null);
  /// Playlists with an invitation link this session: Reset link is offered
  /// only once there is a link to replace.
  const invited = new Set<string>();
  return {
    get target() {
      return target;
    },
    open(next: CollaboratorsTarget) {
      target = next;
    },
    close() {
      target = null;
    },
    hasLink(uuid: string): boolean {
      return invited.has(uuid);
    },
    markLink(uuid: string) {
      invited.add(uuid);
    },
    reset() {
      target = null;
      invited.clear();
    },
  };
}

export const collaboratorsDialog = createCollaboratorsDialog();

/// Up to two initials for an avatar without a picture.
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  const letters = words.length === 1 ? [...words[0]].slice(0, 2) : [[...words[0]][0], [...words[1]][0]];
  return letters.join("").toUpperCase();
}
