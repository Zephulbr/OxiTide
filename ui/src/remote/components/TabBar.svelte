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
  a {
    position: relative;
    transition: color var(--dur-state) var(--ease-enter);
  }
  /* A soft accent pill grows behind the active tab's icon. */
  a::before {
    content: "";
    position: absolute;
    top: 7px;
    width: 52px;
    height: 30px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    opacity: 0;
    transform: scaleX(0.4);
    transition:
      opacity var(--dur-state) var(--ease-enter),
      transform var(--dur-state) var(--ease-enter);
  }
  a.active::before {
    opacity: 1;
    transform: scaleX(1);
  }
  a :global(svg) {
    position: relative;
    transition: transform var(--dur-micro) var(--ease-enter);
  }
  a.active {
    color: var(--accent);
  }
  a:active :global(svg) {
    transform: scale(0.88);
  }
</style>
