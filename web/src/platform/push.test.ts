import { describe, expect, it } from "vitest";
import { keyBytes } from "./push";

describe("keyBytes", () => {
  it("reads URL-safe base64 without padding", () => {
    expect([...keyBytes("-_8")]).toEqual([0xfb, 0xff]);
    expect([...keyBytes("AQID")]).toEqual([1, 2, 3]);
    expect([...keyBytes("AQI")]).toEqual([1, 2]);
  });

  it("gives the 65 bytes of a server key", () => {
    const text = "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8";
    const bytes = keyBytes(text);
    expect(bytes).toHaveLength(65);
    expect(bytes[0]).toBe(4);
  });
});
