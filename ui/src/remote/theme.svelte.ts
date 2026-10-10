/// The phone's light/dark preference (spec §6.5), reactive so the cover
/// palette is re-analysed for the right room when the phone switches.
class RemoteTheme {
  current = $state<"dark" | "light">("dark");
}

export const theme = new RemoteTheme();

/// Follow `prefers-color-scheme` and mirror it onto the root (the page CSP
/// forbids the inline boot script the desktop shell uses).
export function followSystemTheme() {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = () => {
    theme.current = query.matches ? "dark" : "light";
    document.documentElement.dataset.theme = theme.current;
    document.documentElement.style.colorScheme = theme.current;
  };
  apply();
  query.addEventListener("change", apply);
}
