<script lang="ts">
  import Icon, { type IconName } from "../../lib/components/Icon.svelte";

  let { active }: { active: "now" | "home" | "search" | "library" | "queue" } = $props();

  const TABS: { id: typeof active; href: string; label: string; icon: IconName }[] = [
    { id: "now", href: "#/now", label: "Playing", icon: "disc-3" },
    { id: "home", href: "#/home", label: "Home", icon: "home" },
    { id: "search", href: "#/search", label: "Search", icon: "search" },
    { id: "library", href: "#/library", label: "Library", icon: "library" },
    { id: "queue", href: "#/queue", label: "Queue", icon: "list-music" },
  ];
</script>

<nav aria-label="Sections">
  {#each TABS as tab (tab.id)}
    <a href={tab.href} class:active={active === tab.id} aria-current={active === tab.id ? "page" : undefined}>
      <Icon name={tab.icon} size={22} />
      <span>{tab.label}</span>
    </a>
  {/each}
</nav>

<style>
  nav {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    height: calc(var(--remote-tabbar) + env(safe-area-inset-bottom));
    padding-bottom: env(safe-area-inset-bottom);
    border-top: 1px solid var(--line);
    background: var(--surface-float);
    backdrop-filter: blur(var(--glass-blur));
    -webkit-backdrop-filter: blur(var(--glass-blur));
  }
  a {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    font-size: 11px;
    color: var(--text-muted);
  }
  a.active {
    color: var(--accent);
  }
</style>
