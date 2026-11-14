/// Filtering for a searchable dropdown (`settings/Select.svelte`): an option
/// matches when its label or one of its keywords contains the query, with
/// case and accents ignored ("espanol" finds Español, "TURK" finds Türkçe).

export function foldForSearch(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLocaleLowerCase().trim();
}

export function filterOptions<O extends { label: string; keywords?: readonly string[] }>(
  options: readonly O[],
  query: string,
): O[] {
  const needle = foldForSearch(query);
  if (needle === "") return [...options];
  return options.filter((option) =>
    [option.label, ...(option.keywords ?? [])].some((term) => foldForSearch(term).includes(needle)),
  );
}
