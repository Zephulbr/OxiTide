/// "Show the playing track" from outside a page (the player bar's "From …"
/// link when its page is already open). Each request bumps a counter; a
/// mounted TrackTable that holds the playing track scrolls to it. Tables
/// mounted later ignore requests made before them.
export function createLocatePlaying() {
  let requests = $state(0);
  return {
    get requests() {
      return requests;
    },
    request() {
      requests += 1;
    },
  };
}

export const locatePlaying = createLocatePlaying();
