import type { QueueItem } from "../lib/api";

/// Rows per `get_queue_page` request.
export const QUEUE_PAGE = 100;

/// The phone's paged copy of the engine queue (RM-D8). Pages are dropped
/// whenever `queueRev` (or the length) moves, and refetched on demand.
export class QueueCache {
  rev = -1;
  total = 0;
  private pages = new Map<number, QueueItem[]>();

  /// Returns true when the cache was invalidated.
  sync(rev: number, total: number): boolean {
    if (rev === this.rev && total === this.total) return false;
    this.rev = rev;
    this.total = total;
    this.pages.clear();
    return true;
  }

  pageOf(index: number): number {
    return Math.floor(index / QUEUE_PAGE);
  }

  has(page: number): boolean {
    return this.pages.has(page);
  }

  /// Store a page fetched at `rev`; a page from an older revision is dropped.
  set(page: number, items: QueueItem[], rev: number): boolean {
    if (rev !== this.rev) return false;
    this.pages.set(page, items);
    return true;
  }

  get(index: number): QueueItem | undefined {
    return this.pages.get(this.pageOf(index))?.[index % QUEUE_PAGE];
  }

  /// Unloaded pages overlapping rows `[from, to)`.
  missing(from: number, to: number): number[] {
    const out: number[] = [];
    const last = Math.min(to, this.total) - 1;
    if (last < 0) return out;
    for (let page = this.pageOf(Math.max(0, from)); page <= this.pageOf(last); page++) {
      if (!this.pages.has(page)) out.push(page);
    }
    return out;
  }
}
