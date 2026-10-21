<script lang="ts">
  import { api, type Stats, type StatsTopTrack } from "../api";
  import { ARTIST_PICTURE_SIZE } from "../artwork";
  import { errorMessage } from "../errors";
  import { formatCount, formatListenTime } from "../format";
  import {
    barPercents,
    dayPart,
    dayTick,
    hourLabel,
    peakIndex,
    percentOf,
    WEEKDAYS,
  } from "../historyChart";
  import { getCardActions } from "../cardActions";
  import { menuFor, type MenuItem } from "../menu.svelte";
  import { historyMenuItems } from "../menus";
  import type { Nav, Route } from "../nav.svelte";
  import { playSource } from "../playSource.svelte";
  import { goToTargets } from "../queueLinks";
  import { playArtistRadio, playTrackRadio } from "../radio";
  import { resolveAlbum, resolveArtist, type ArtistTarget } from "../statsResolve";
  import type { Toasts } from "../toasts.svelte";
  import { trackFlags } from "../trackFlags.svelte";
  import Cover from "./Cover.svelte";

  /// History's Stats tab: an all-time masthead with a fan of the top album
  /// covers, the 30-day listening chart beside a "when you listen" read of the
  /// hours and weekdays, and three ranked columns (artists, tracks, albums),
  /// each led by its #1 as a feature card.
  let { stats, nav, toasts }: { stats: Stats; nav: Nav; toasts: Toasts } = $props();

  type Metric = "time" | "plays";
  let metric = $state<Metric>("time");
  let hovered = $state<number | null>(null);

  const plural = (n: number, word: string) => `${formatCount(n)} ${n === 1 ? word : `${word}s`}`;

  /// The masthead's count line; the numbers take the accent.
  const counts = $derived([
    { n: stats.total_plays, word: "play" },
    { n: stats.distinct_tracks, word: "track" },
    { n: stats.distinct_artists, word: "artist" },
    { n: stats.distinct_albums, word: "album" },
  ]);
  const fan = $derived(stats.top_albums.filter((album) => album.cover).slice(0, 5));
  const figures = $derived([
    {
      label: "Last 7 days",
      value: formatListenTime(stats.listened_ms_7d),
      note: `${formatListenTime(stats.listened_ms_7d / 7)} a day`,
    },
    {
      label: "Last 30 days",
      value: formatListenTime(stats.listened_ms_30d),
      note: `${formatListenTime(stats.listened_ms_30d / 30)} a day`,
    },
    {
      label: "Streak",
      value: plural(stats.current_streak_days, "day"),
      note: `Best ${plural(stats.longest_streak_days, "day")}`,
    },
    {
      label: "Played through",
      value: `${percentOf(stats.completed_plays, stats.total_plays)}%`,
      note: `${formatCount(stats.completed_plays)} of ${plural(stats.total_plays, "play")}`,
    },
  ]);

  const values = $derived(stats.daily.map((day) => (metric === "time" ? day.listened_ms : day.plays)));
  const heights = $derived(barPercents(values));
  const average = $derived(values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length));
  const averagePercent = $derived(Math.min(100, (average / Math.max(1, ...values)) * 100));
  const formatValue = (value: number) =>
    metric === "time" ? formatListenTime(value) : plural(Math.round(value), "play");
  const tip = $derived(hovered === null ? null : stats.daily[hovered]);

  const hourHeights = $derived(barPercents(stats.hourly));
  const peakHour = $derived(peakIndex(stats.hourly));
  const dayHeights = $derived(barPercents(stats.weekdays));
  const peakDay = $derived(peakIndex(stats.weekdays));

  // Names resolve to ids (and artist pictures) through each entry's newest play.
  let artistTargets = $state<Record<string, ArtistTarget | null>>({});
  let albumIds = $state<Record<string, number | null>>({});
  $effect(() => {
    const artists = stats.top_artists;
    const albums = stats.top_albums;
    let live = true;
    for (const entry of artists) {
      resolveArtist(entry, api)
        .catch(() => null)
        .then((target) => {
          if (live) artistTargets[entry.name] = target;
        });
    }
    for (const entry of albums) {
      resolveAlbum(entry, api)
        .catch(() => null)
        .then((id) => {
          if (live) albumIds[entry.name] = id;
        });
    }
    return () => {
      live = false;
    };
  });

  interface Row {
    key: string;
    name: string;
    by: string;
    plays: number;
    listened: number;
    /// `undefined` while an artist picture is still resolving.
    cover: string | null | undefined;
    round: boolean;
    /// An album cover: square corners, like album sleeves across the UI.
    sleeve: boolean;
    /// The cover is an artist picture (served at different sizes than albums).
    picture: boolean;
    open: (() => void) | null;
    label: string;
    /// Right-click / Shift+F10 menu, built when it opens (ids resolve late).
    menu: () => MenuItem[];
  }

  const cardActions = getCardActions();
  const go = (route: Route) => nav.goto(route);

  /// Shown while a name has not resolved to an id yet (or never will).
  function pendingItem(label: string): MenuItem[] {
    return [{ id: "pending", label, icon: "external-link", disabled: true, run: () => {} }];
  }

  function artistMenu(name: string, target: ArtistTarget | null | undefined): MenuItem[] {
    if (!target) return pendingItem(`Go to ${name}`);
    return [
      { id: "go-artist", label: "Go to artist", icon: "user", run: () => go({ name: "artist", id: target.id }) },
      { id: "radio", label: "Artist radio", icon: "radio", run: () => void playArtistRadio(target.id, toasts, name) },
    ];
  }

  function trackMenu(track: StatsTopTrack, i: number): MenuItem[] {
    const [play, ...rest] = historyMenuItems({ play: () => void playTop(i), ...goToTargets(track, go, api.getTrack) });
    return [
      play,
      { id: "radio", label: "Track radio", icon: "radio", run: () => void playTrackRadio(track.track_id, toasts, track.title) },
      ...rest,
    ];
  }

  /// Albums get the same menu as album cards (play, queue, favorite, link).
  function albumMenu(name: string, cover: string | null, id: number | null | undefined): MenuItem[] {
    if (!id || !cardActions) return pendingItem(`Go to ${name}`);
    const item = { id: String(id), title: name, item_type: "ALBUM", artwork: cover };
    return cardActions.menu(item, { kind: "album", id, title: name });
  }

  async function playTop(start: number) {
    const tracks = stats.top_tracks;
    const prepared = await trackFlags.filterPlayable(
      tracks.map((track) => ({ track, id: track.track_id, explicit: undefined })),
      start,
    );
    if (!prepared) {
      toasts.show("Everything here is hidden by your content filters.", { severity: "info" });
      return;
    }
    playSource.set(
      { kind: "history", name: "Your top tracks", route: { name: "history", tab: "stats" } },
      prepared.items.map((item) => item.id),
    );
    const items = prepared.items.map(({ track }: { track: StatsTopTrack }) => ({
      id: track.track_id,
      title: track.title,
      artists: [track.artist],
      album: track.album,
      cover: track.cover ?? null,
    }));
    void api.playPageItems(items, prepared.start).catch((e) => {
      toasts.show(errorMessage(e), { severity: "error" });
    });
  }

  const columns = $derived([
    {
      title: "Top artists",
      lead: "Most played artist",
      total: plural(stats.distinct_artists, "artist"),
      rows: stats.top_artists.map((entry): Row => {
        const target = artistTargets[entry.name];
        return {
          key: entry.name,
          name: entry.name,
          by: "",
          plays: entry.plays,
          listened: entry.listened_ms,
          cover: target === undefined ? undefined : (target?.picture ?? entry.cover ?? null),
          round: true,
          sleeve: false,
          picture: !!target?.picture,
          open: target ? () => nav.goto({ name: "artist", id: target.id }) : null,
          label: `Open ${entry.name}`,
          menu: () => artistMenu(entry.name, artistTargets[entry.name]),
        };
      }),
    },
    {
      title: "Top tracks",
      lead: "Most played track",
      total: plural(stats.distinct_tracks, "track"),
      rows: stats.top_tracks.map(
        (track, i): Row => ({
          key: String(track.track_id),
          name: track.title,
          by: track.artist,
          plays: track.plays,
          listened: track.listened_ms,
          cover: track.cover ?? null,
          round: false,
          sleeve: false,
          picture: false,
          open: () => void playTop(i),
          label: `Play ${track.title}`,
          menu: () => trackMenu(track, i),
        }),
      ),
    },
    {
      title: "Top albums",
      lead: "Most played album",
      total: plural(stats.distinct_albums, "album"),
      rows: stats.top_albums.map((entry): Row => {
        const id = albumIds[entry.name];
        return {
          key: entry.name,
          name: entry.name,
          by: "",
          plays: entry.plays,
          listened: entry.listened_ms,
          cover: entry.cover ?? null,
          round: false,
          sleeve: true,
          picture: false,
          open: id ? () => nav.goto({ name: "album", id }) : null,
          label: `Open ${entry.name}`,
          menu: () => albumMenu(entry.name, entry.cover ?? null, albumIds[entry.name]),
        };
      }),
    },
  ]);
</script>

{#snippet art(row: Row, size: 80 | 160 | 320)}
  <span class="art" class:round={row.round} class:square={row.sleeve}>
    {#if row.cover !== undefined}
      <Cover uuid={row.cover} size={row.picture ? ARTIST_PICTURE_SIZE : size} radius={row.round ? 999 : row.sleeve ? 0 : 6} alt="" />
    {/if}
  </span>
{/snippet}

{#if stats.total_plays === 0}
  <p class="muted">No plays yet.</p>
{:else}
  <section class="hero lit-tint pane" aria-label="All-time listening">
    <div class="headline">
      <span class="eyebrow">All time</span>
      <strong class="total tnum">{formatListenTime(stats.total_listened_ms)}</strong>
      <dl class="figures">
        {#each figures as figure (figure.label)}
          <div>
            <dt class="muted">{figure.label}</dt>
            <dd class="tnum">{figure.value}</dd>
            <dd class="muted tnum note">{figure.note}</dd>
          </div>
        {/each}
      </dl>
    </div>
    <!-- All-time counts, stacked in the middle of the masthead. -->
    <ul class="counts" aria-label="All-time counts">
      {#each counts as count (count.word)}
        <li class="tnum">
          <span class="n">{formatCount(count.n)}</span>
          <span class="muted">{count.n === 1 ? count.word : `${count.word}s`}</span>
        </li>
      {/each}
    </ul>
    {#if fan.length > 0}
      <div class="fan" aria-hidden="true" style:--count={fan.length}>
        {#each fan as album, i (album.name)}
          <span class="sleeve" style:--i={i} title={album.name}><Cover uuid={album.cover} size={320} radius={0} alt="" /></span>
        {/each}
      </div>
    {/if}
  </section>

  <div class="charts">
    <section class="panel lit pane daily">
      <header class="panel-head">
        <div>
          <h3>Last 30 days</h3>
          <span class="muted tnum sub">Average {formatValue(average)} a day</span>
        </div>
        <div class="segmented" role="group" aria-label="Chart value">
          <button type="button" aria-pressed={metric === "time"} onclick={() => (metric = "time")}>Time</button>
          <button type="button" aria-pressed={metric === "plays"} onclick={() => (metric = "plays")}>Plays</button>
        </div>
      </header>
      <div
        class="chart"
        role="img"
        aria-label={`${metric === "time" ? "Listening time" : "Plays"} per day over the last 30 days`}
        onpointerleave={() => (hovered = null)}
      >
        <div class="grid-lines" aria-hidden="true"><i></i><i></i><i></i><i></i></div>
        {#if average > 0}
          <div class="average" style:bottom={`${averagePercent}%`} aria-hidden="true"></div>
        {/if}
        {#each stats.daily as day, i (day.date)}
          <div
            class="bar"
            class:today={i === stats.daily.length - 1}
            class:dim={hovered !== null && hovered !== i}
            onpointerenter={() => (hovered = i)}
            aria-hidden="true"
          >
            <span class="fill" class:empty={values[i] === 0} style:height={`${heights[i]}%`}></span>
          </div>
        {/each}
        {#if tip && hovered !== null}
          <div class="tip tnum" style:--x={(hovered + 0.5) / stats.daily.length}>
            <strong>{hovered === stats.daily.length - 1 ? "Today" : dayTick(tip.date)}</strong>
            <span>{formatListenTime(tip.listened_ms)} · {plural(tip.plays, "play")}</span>
          </div>
        {/if}
      </div>
      {#if stats.daily.length > 0}
        <div class="axis muted tnum" aria-hidden="true">
          <span>{dayTick(stats.daily[0].date)}</span>
          <span>{dayTick(stats.daily[Math.floor(stats.daily.length / 2)].date)}</span>
          <span>Today</span>
        </div>
      {/if}
    </section>

    <section class="panel lit pane rhythm">
      <header class="panel-head">
        <div>
          <h3>When you listen</h3>
          {#if peakHour >= 0}
            <span class="muted sub">Mostly {dayPart(peakHour)} · peak around {hourLabel(peakHour)}</span>
          {/if}
        </div>
      </header>
      <div class="hours" role="img" aria-label="Plays by hour of day">
        {#each stats.hourly as plays, hour (hour)}
          <span class="bar" class:peak={hour === peakHour} title={`${hourLabel(hour)}: ${plural(plays, "play")}`}>
            <i class:empty={plays === 0} style:height={`${hourHeights[hour]}%`}></i>
          </span>
        {/each}
      </div>
      <div class="axis hours-axis muted tnum" aria-hidden="true">
        <span>00</span><span>06</span><span>12</span><span>18</span><span>24</span>
      </div>
      <div class="week">
        {#each stats.weekdays as plays, day (day)}
          <div class="weekday" class:peak={day === peakDay} title={`${WEEKDAYS[day]}: ${plural(plays, "play")}`}>
            <span class="muted day">{WEEKDAYS[day].slice(0, 3)}</span>
            <span class="track"><i style:width={`${dayHeights[day]}%`}></i></span>
          </div>
        {/each}
      </div>
    </section>
  </div>

  <div class="tops">
    {#each columns as column (column.title)}
      {@const top = column.rows[0]}
      {@const max = Math.max(1, ...column.rows.map((row) => row.plays))}
      <section class="panel lit pane column">
        <header class="panel-head">
          <h3>{column.title}</h3>
          <span class="muted tnum">{column.total}</span>
        </header>
        {#if top}
          <button
            type="button"
            class="lead"
            disabled={!top.open}
            onclick={() => top.open?.()}
            aria-label={top.label}
            use:menuFor={() => top.menu()}
          >
            {@render art(top, 320)}
            <span class="lead-text">
              <span class="eyebrow">{column.lead}</span>
              <strong class="lead-name">{top.name}</strong>
              {#if top.by}<span class="lead-by">{top.by}</span>{/if}
              <span class="muted tnum lead-meta">{plural(top.plays, "play")} · {formatListenTime(top.listened)}</span>
            </span>
          </button>
        {/if}
        <ol start="2">
          {#each column.rows.slice(1) as row, i (row.key)}
            <li>
              <button
                type="button"
                class="row"
                disabled={!row.open}
                onclick={() => row.open?.()}
                aria-label={row.label}
                use:menuFor={() => row.menu()}
                style:--share={row.plays / max}
              >
                <span class="rank tnum">{i + 2}</span>
                {@render art(row, 80)}
                <span class="name">
                  <span class="primary">{row.name}</span>
                  {#if row.by}<span class="muted by">{row.by}</span>{/if}
                </span>
                <span class="meter" aria-hidden="true"><i></i></span>
                <span class="muted tnum plays">{row.plays}</span>
              </button>
            </li>
          {/each}
        </ol>
      </section>
    {/each}
  </div>
{/if}

<style>
  .eyebrow {
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
  }

  /* Masthead: the all-time figure in the display weight, four secondary figures under
     it, and the top albums fanned out on the right like sleeves on a table. */
  .hero {
    position: relative;
    display: grid;
    /* Headline | counts | album fan. Equal side tracks put the counts on the
       card's centre line (an `auto 1fr auto` split centres them between the
       headline and the fan instead, which drifts with their widths). */
    grid-template-columns: 1fr auto 1fr;
    gap: 32px;
    align-items: center;
    margin-bottom: 16px;
    padding: 26px 28px;
    border-radius: var(--radius-sheet);
    box-shadow: var(--shadow-panel);
    overflow: hidden;
  }
  .headline {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .total {
    font-family: var(--font-display);
    font-size: 56px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    line-height: 1.05;
  }
  /* The masthead sits on an accent wash (`.lit-tint`) that lifts and tints
     the backdrop, so the global muted grey loses contrast here. Secondary
     text uses a stronger share of the text colour instead, and the figure
     dividers a firmer line. */
  .hero {
    --hero-muted: color-mix(in srgb, var(--text) 74%, transparent);
  }
  .hero .muted {
    color: var(--hero-muted);
  }
  /* One row across the card's centre: each number over its word. */
  .counts {
    grid-column: 2;
    display: flex;
    gap: 40px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .counts li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .counts .n {
    color: var(--accent);
    font-size: 32px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    line-height: 1.1;
  }
  .counts .muted {
    font-size: 14.5px;
  }
  .figures {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, max-content));
    gap: 0;
    margin: 22px 0 0;
  }
  .figures div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 28px;
    border-left: 1px solid color-mix(in srgb, var(--text) 16%, transparent);
  }
  .figures div:first-child {
    padding-left: 0;
    border-left: 0;
  }
  .figures dt {
    font-size: 12.5px;
    font-weight: 600;
  }
  .figures dd {
    margin: 0;
    font-size: 19px;
    font-weight: 650;
    white-space: nowrap;
  }
  .figures dd.note {
    font-size: 12.5px;
    font-weight: 500;
  }
  .fan {
    --size: 150px;
    --step: 62px;
    grid-column: 3;
    justify-self: end;
    position: relative;
    width: calc(var(--size) + (var(--count) - 1) * var(--step));
    height: var(--size);
    margin-right: 6px;
  }
  /* #1 in front on the left; later sleeves sit further back: smaller and dimmer. */
  .sleeve {
    position: absolute;
    top: 0;
    left: calc(var(--i) * var(--step));
    z-index: calc(10 - var(--i));
    width: var(--size);
    height: var(--size);
    border-radius: 0;
    box-shadow: var(--shadow-cover), 0 0 0 1px var(--line);
    scale: calc(1 - var(--i) * 0.08);
    transform-origin: right center;
    filter: brightness(calc(1 - var(--i) * 0.12));
    transition:
      translate var(--dur-view) var(--ease-enter),
      scale var(--dur-view) var(--ease-enter),
      filter var(--dur-state) var(--ease-enter);
  }
  /* Hover: the stack fans out to the left (the back sleeve stays put, each
     one nearer the front slides further), and the sleeve under the pointer
     comes forward at full size and brightness, lifted a little. */
  .fan:hover .sleeve {
    translate: calc((var(--i) - var(--count) + 1) * 34px) 0;
  }
  .fan .sleeve:hover {
    z-index: 20;
    scale: 1;
    translate: calc((var(--i) - var(--count) + 1) * 34px) -6px;
    filter: none;
  }
  .sleeve :global(.cover) {
    width: 100%;
    height: 100%;
  }

  .panel {
    padding: 18px 20px 20px;
    border-radius: var(--radius-card);
  }
  .panel-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
    font-size: 12.5px;
  }
  .panel-head h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    display: block;
    margin-top: 2px;
    font-size: 12.5px;
  }

  .charts {
    display: grid;
    grid-template-columns: minmax(0, 1.9fr) minmax(300px, 1fr);
    gap: 16px;
    margin-bottom: 16px;
  }
  /* The chart grows to the height of the "when you listen" panel beside it. */
  .daily {
    display: flex;
    flex-direction: column;
  }
  .segmented {
    display: inline-flex;
    padding: 2px;
    border-radius: 99px;
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .segmented button {
    height: 26px;
    padding: 0 12px;
    border-radius: 99px;
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
  }
  .segmented button:hover {
    color: var(--text);
  }
  .segmented button[aria-pressed="true"] {
    background: var(--surface-3);
    color: var(--text);
  }

  .chart {
    position: relative;
    display: flex;
    align-items: flex-end;
    flex: 1;
    gap: 5px;
    min-height: 180px;
  }
  .grid-lines {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    pointer-events: none;
  }
  .grid-lines i {
    height: 1px;
    background: var(--line);
    opacity: 0.5;
  }
  .average {
    position: absolute;
    right: 0;
    left: 0;
    z-index: 1;
    border-top: 1px dashed color-mix(in srgb, var(--text) 35%, transparent);
    pointer-events: none;
  }
  .daily .bar {
    position: relative;
    flex: 1;
    height: 100%;
    display: flex;
    align-items: flex-end;
    transition: opacity var(--dur-micro) var(--ease-enter);
  }
  .daily .bar.dim {
    opacity: 0.45;
  }
  .fill {
    width: 100%;
    min-height: 3px;
    border-radius: 5px 5px 2px 2px;
    background: linear-gradient(
      180deg,
      color-mix(in srgb, var(--accent) 80%, transparent),
      color-mix(in srgb, var(--accent) 30%, transparent)
    );
  }
  .today .fill {
    background: linear-gradient(180deg, var(--accent), color-mix(in srgb, var(--accent) 55%, transparent));
  }
  .fill.empty {
    min-height: 2px;
    background: var(--line);
  }
  .tip {
    position: absolute;
    top: -6px;
    left: clamp(70px, calc(var(--x) * 100%), calc(100% - 70px));
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-float);
    box-shadow: var(--shadow-panel);
    font-size: 12px;
    white-space: nowrap;
    transform: translateX(-50%);
    pointer-events: none;
  }
  .tip span {
    color: var(--text-muted);
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 11.5px;
  }

  .hours {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 92px;
  }
  .hours .bar,
  .weekday .track {
    flex: 1;
    display: flex;
  }
  .hours .bar {
    height: 100%;
    align-items: flex-end;
  }
  .hours i,
  .weekday i {
    background: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .hours i {
    width: 100%;
    min-height: 2px;
    border-radius: 3px 3px 1px 1px;
  }
  .hours i.empty {
    background: var(--line);
  }
  :is(.hours .bar, .weekday).peak i {
    background: var(--accent);
  }
  .hours-axis {
    margin-bottom: 18px;
  }
  .week {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .weekday {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
  }
  .weekday .track {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .weekday i {
    display: block;
    border-radius: 3px;
  }
  .day {
    font-size: 12px;
  }
  .weekday.peak .day {
    color: var(--text);
    font-weight: 600;
  }

  .tops {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }
  /* The #1 of each column: a feature card with large art. */
  .lead {
    display: grid;
    grid-template-columns: 104px minmax(0, 1fr);
    gap: 16px;
    align-items: center;
    width: 100%;
    margin-bottom: 10px;
    padding: 10px;
    border-radius: var(--radius-card);
    /* No fill at rest: the #1 sits open on the panel and only takes the
       rows' hover surface. */
    background: transparent;
    color: inherit;
    text-align: left;
    transition: background-color var(--dur-micro) var(--ease-enter);
  }
  .lead:not(:disabled):hover {
    background: var(--surface-2);
  }
  .lead .art {
    width: 104px;
    height: 104px;
    box-shadow: var(--shadow-cover);
  }
  .lead-text {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .lead-name {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: var(--display-weight);
    letter-spacing: var(--display-tracking);
    line-height: 1.15;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .lead-by {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lead-meta {
    margin-top: 4px;
    font-size: 12.5px;
  }
  .art {
    display: block;
    flex: none;
    border-radius: 6px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .art.round {
    border-radius: 50%;
  }
  .art.square {
    border-radius: 0;
  }
  .art :global(.cover) {
    width: 100%;
    height: 100%;
  }
  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    position: relative;
    display: grid;
    grid-template-columns: 20px 36px minmax(0, 1fr) 44px 24px;
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 5px 8px;
    border-radius: var(--radius-control);
    color: inherit;
    text-align: left;
    transition: background-color var(--dur-micro) var(--ease-enter);
  }
  .row:not(:disabled):hover {
    background: var(--surface-2);
  }
  /* Disabled only while ids resolve; the row should not look faded. */
  .row:disabled,
  .lead:disabled {
    opacity: 1;
    cursor: default;
  }
  /* A small meter before the count: plays against the column's #1. */
  .meter {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .meter i {
    display: block;
    width: calc(var(--share) * 100%);
    height: 100%;
    border-radius: 2px;
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .plays {
    text-align: right;
  }
  .row .art {
    width: 36px;
    height: 36px;
  }
  .rank {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    text-align: right;
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .primary,
  .by {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .primary {
    font-weight: 500;
  }
  .by,
  .plays {
    font-size: 12.5px;
  }

  @media (max-width: 1200px) {
    .figures {
      grid-template-columns: repeat(2, minmax(0, max-content));
      row-gap: 16px;
    }
    .figures div:nth-child(3) {
      padding-left: 0;
      border-left: 0;
    }
    .charts,
    .tops {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (max-width: 900px) {
    .fan {
      display: none;
    }
    .hero {
      grid-template-columns: minmax(0, 1fr);
    }
    .total {
      font-size: 42px;
    }
    /* Under the headline: the counts run across in one wrapping row. */
    /* Under the headline, left-aligned, wrapping if it must. */
    .counts {
      grid-column: auto;
      justify-self: start;
      flex-wrap: wrap;
      gap: 12px 28px;
    }
    .counts li {
      align-items: flex-start;
    }
  }
</style>
