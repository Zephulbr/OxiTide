<script lang="ts">
  import Icon, { type IconName } from "../../lib/components/Icon.svelte";
  import { hrefFor, type RemoteRoute } from "../routes";
  import { t, type RemoteKey } from "../i18n.svelte";

  const LINKS: { label: RemoteKey; icon: IconName; route: RemoteRoute }[] = [
    { label: "library.tracks", icon: "heart", route: { name: "favorites", kind: "tracks" } },
    { label: "library.albums", icon: "disc-3", route: { name: "favorites", kind: "albums" } },
    { label: "library.artists", icon: "music", route: { name: "favorites", kind: "artists" } },
    { label: "library.playlists", icon: "list-music", route: { name: "playlists", folder: "root" } },
    { label: "library.mixes", icon: "radio", route: { name: "mixes" } },
    { label: "library.recent", icon: "history", route: { name: "history" } },
  ];
</script>

<div class="page">
  <header class="page-head"><h1>{t("tab.library")}</h1></header>
  <ul>
    {#each LINKS as link (link.label)}
      <li>
        <a href={hrefFor(link.route)}>
          <Icon name={link.icon} size={22} />
          <span>{t(link.label)}</span>
          <Icon name="chevron-right" size={18} />
        </a>
      </li>
    {/each}
  </ul>
</div>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  a {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 56px;
    border-bottom: 1px solid var(--line);
    font-size: 17px;
    font-weight: 600;
  }
  a span {
    flex: 1;
  }
  a :global(svg:first-child) {
    color: var(--accent);
  }
  a :global(svg:last-child) {
    color: var(--text-muted);
  }
</style>
