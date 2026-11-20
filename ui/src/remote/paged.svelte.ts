import type { RowTrack } from "./convert";

export const TRACK_PAGE = 100;

/// What `TrackList` needs from a paged list: the next page, and whether any
/// of the list is still unread.
export interface TrackPaging {
  /// True once every row of the list has been read.
  readonly done: boolean;
  /// Rows the server has not been asked for yet (unavailable ones included).
  readonly left: number;
  more(): Promise<void>;
}

export interface PagedOptions {
  /// Drop ids already loaded (favorites cannot repeat); playlists keep a track
  /// that appears twice.
  dedupe?: boolean;
}

/// A track list fetched page by page (playlists, favorites, history), so a
/// long list never rides one response.
///
/// Offsets index the server's whole list: TIDAL leaves unavailable tracks out
/// of `items` but still counts them in `total`, so each request advances by
/// the page size, not by the rows it brought back, and the list ends at
/// `total`, not at the first empty page (a run of 100 unavailable tracks
/// returns one). The desktop pager and `get_all_pages` follow the same rule.
export class PagedTracks<I> implements TrackPaging {
  rows = $state<RowTrack[]>([]);
  total = $state(0);
  done = $state(false);
  /// The first row not yet requested; also the next request's offset.
  private next = $state(0);
  private readonly seen: Set<number> | null;

  constructor(
    private readonly fetchPage: (offset: number, limit: number) => Promise<{ items: I[]; total: number }>,
    private readonly map: (item: I) => RowTrack,
    { dedupe = false }: PagedOptions = {},
  ) {
    this.seen = dedupe ? new Set() : null;
  }

  get left(): number {
    return this.done ? 0 : Math.max(0, this.total - this.next);
  }

  async first(): Promise<this> {
    await this.more();
    return this;
  }

  more = async () => {
    if (this.done) return;
    const offset = this.next;
    const page = await this.fetchPage(offset, TRACK_PAGE);
    let incoming = page.items.map(this.map);
    if (this.seen) {
      const seen = this.seen;
      incoming = incoming.filter((row) => !seen.has(row.id) && (seen.add(row.id), true));
    }
    if (incoming.length > 0) this.rows = [...this.rows, ...incoming];
    // An empty page past a stale total reports 0: keep the last known total.
    if (page.total > 0) this.total = page.total;
    this.next = offset + TRACK_PAGE;
    // Without a total, a short (or empty) page is the only end marker.
    this.done = page.total > 0 ? this.next >= page.total : page.items.length < TRACK_PAGE;
  };
}
