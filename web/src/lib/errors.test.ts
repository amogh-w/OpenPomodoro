import { describe, expect, it } from "vitest";
import { ApiError } from "./api";
import { errorMessage } from "./errors";
import en from "../locales/en";

describe("errorMessage", () => {
  it("maps known server codes to catalog strings", () => {
    expect(errorMessage(new ApiError(401, "invalid_credentials"))).toBe(en["error.invalid_credentials"]);
    expect(errorMessage(new ApiError(409, "username_taken"))).toBe(en["error.username_taken"]);
    expect(errorMessage(new ApiError(403, "registration_closed"))).toBe(en["error.registration_closed"]);
  });

  it("says how long to wait when rate limited", () => {
    expect(errorMessage(new ApiError(429, "rate_limited", 29_500))).toContain("30");
    expect(errorMessage(new ApiError(429, "rate_limited", null))).toContain("30");
  });

  it("has a message for an unreachable server and for anything unexpected", () => {
    expect(errorMessage(new ApiError(0, "network"))).toBe(en["error.network"]);
    expect(errorMessage(new ApiError(500, "something_new"))).toBe(en["error.unknown"]);
    expect(errorMessage(new Error("boom"))).toBe(en["error.unknown"]);
  });
});
