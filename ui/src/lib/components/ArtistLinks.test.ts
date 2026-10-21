// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import ArtistLinks from "./ArtistLinks.svelte";

afterEach(cleanup);

describe("ArtistLinks", () => {
  // Svelte trims a block's trailing whitespace, so a literal `, ` once
  // rendered as "Ado,KUROMI".
  it("separates artists with a comma and a space", () => {
    const { container } = render(ArtistLinks, {
      artists: [
        { id: 1, name: "Ado" },
        { id: 2, name: "KUROMI" },
        { id: 3, name: "Giga" },
      ],
    });
    expect(container.textContent).toBe("Ado, KUROMI, Giga");
  });
});
