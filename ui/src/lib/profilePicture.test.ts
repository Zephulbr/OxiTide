import { beforeEach, describe, expect, it } from "vitest";
import { profilePicture } from "./profilePicture.svelte";

beforeEach(() => profilePicture.reset());

describe("profilePicture", () => {
  it("shows the uploaded picture while TIDAL's record still has the old one", () => {
    profilePicture.changed(7, "https://img/new.jpg", "old-uuid");
    expect(profilePicture.for(7, "old-uuid")).toBe("https://img/new.jpg");
    // Another artist is untouched.
    expect(profilePicture.for(8, "old-uuid")).toBe("old-uuid");
  });

  it("lets TIDAL's record win once it has changed", () => {
    profilePicture.changed(7, "https://img/new.jpg", "old-uuid");
    expect(profilePicture.for(7, "new-uuid")).toBe("new-uuid");
  });

  it("hides a removed picture until the record drops it", () => {
    profilePicture.changed(7, null, "old-uuid");
    expect(profilePicture.for(7, "old-uuid")).toBeNull();
    expect(profilePicture.for(7, null)).toBeNull();
  });

  it("covers a profile that had no picture before", () => {
    profilePicture.changed(7, "https://img/new.jpg", undefined);
    expect(profilePicture.for(7, null)).toBe("https://img/new.jpg");
    expect(profilePicture.for(7, undefined)).toBe("https://img/new.jpg");
  });
});
