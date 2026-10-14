import type { TrackPage } from "../lib/api";
import { fromTrack, type RowTrack } from "./convert";

export const TRACK_PAGE = 100;

/// A track list fetched page by page (playlists, favorites), so a long list
/// never rides one response.
export class PagedTracks {
  rows = $state<RowTrack[]>([]);
  total = $state(0);

  constructor(private readonly fetchPage: (offset: number, limit: number) => Promise<TrackPage>) {}

  async first(): Promise<this> {
    const page = await this.fetchPage(0, TRACK_PAGE);
    this.rows = page.items.map(fromTrack);
    this.total = page.total;
    return this;
  }

  more = async () => {
    const page = await this.fetchPage(this.rows.length, TRACK_PAGE);
    this.rows = [...this.rows, ...page.items.map(fromTrack)];
    this.total = page.total;
    if (page.items.length === 0) this.total = this.rows.length;
  };
}
