/// The boot screen is static markup in index.html (`#boot`), so it paints
/// before the bundle loads. App dismisses it once startup settles: it fades
/// out over the first real frame, then leaves the DOM.
export function dismissBootScreen(doc: Document = document): void {
  const boot = doc.getElementById("boot");
  if (!boot || boot.classList.contains("boot-done")) return;
  boot.classList.add("boot-done");
  const remove = () => boot.remove();
  boot.addEventListener("transitionend", remove, { once: true });
  // No transition under reduced motion (or a hidden window): remove anyway.
  setTimeout(remove, 400);
}
