<script lang="ts">
  import type { LyricsDto, RemoteTrack } from "../../lib/api";
  import { activeLine } from "../../lib/lyrics";
  import { useRemote } from "../context";

  let { track, positionMs }: { track: RemoteTrack; positionMs: number } = $props();
  const { api } = useRemote();

  let lyrics = $state<LyricsDto | null>(null);
  let status = $state<"loading" | "none" | "ready" | "error">("loading");
  let list = $state<HTMLOListElement | null>(null);

  $effect(() => {
    const id = track.trackId;
    status = "loading";
    lyrics = null;
    api
      .getLyrics(id, track.title, track.artist, track.album, track.durationSecs)
      .then((found) => {
        if (id !== track.trackId) return;
        lyrics = found;
        status = found && found.lines.length > 0 ? "ready" : "none";
      })
      .catch(() => {
        if (id === track.trackId) status = "error";
      });
  });

  const active = $derived(lyrics?.synced ? activeLine(lyrics.lines, positionMs) : -1);

  $effect(() => {
    if (active < 0 || !list) return;
    list.children[active]?.scrollIntoView({ block: "center", behavior: "smooth" });
  });
</script>

<div class="lyrics" aria-live="off">
  {#if status === "loading"}
    <p class="muted">Loading lyrics…</p>
  {:else if status === "none"}
    <p class="muted">No lyrics for this track.</p>
  {:else if status === "error"}
    <p class="muted">Lyrics are unavailable right now.</p>
  {:else if lyrics}
    <ol bind:this={list} dir={lyrics.rtl ? "rtl" : "auto"} class:synced={lyrics.synced}>
      {#each lyrics.lines as line, i (i)}
        <li class:active={i === active} class:past={lyrics.synced && active >= 0 && i < active}>
          {line.text}
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .lyrics {
    height: min(52vh, 440px);
    overflow-y: auto;
    padding: 8px 4px;
    mask-image: linear-gradient(transparent, #000 12%, #000 88%, transparent);
    -webkit-mask-image: linear-gradient(transparent, #000 12%, #000 88%, transparent);
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 20% 0;
  }
  li {
    min-height: 1.3em;
    padding: 6px 0;
    font-size: 20px;
    font-weight: 600;
    line-height: 1.3;
  }
  .synced li {
    color: var(--text-muted);
    transition: color var(--dur-state) var(--ease-in);
  }
  .synced li.past {
    opacity: 0.6;
  }
  .synced li.active {
    color: var(--text);
  }
  p {
    padding-top: 30%;
    text-align: center;
  }
</style>
