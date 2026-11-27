import { describe, expect, it } from "vitest";
import { appendStep, describeSteps } from "./macros";

describe("macro recording", () => {
  it("merges typing and records real pauses", () => {
    let s = appendStep([], "l", 0);
    s = appendStep(s, "s", 100);
    s = appendStep(s, "\r", 120);
    s = appendStep(s, "df -h\r", 2500);
    s = appendStep(s, "x", 60_000);
    expect(s).toEqual([
      { kind: "text", text: "ls\r" },
      { kind: "wait", ms: 2500 },
      { kind: "text", text: "df -h\r" },
      { kind: "wait", ms: 10_000 },
      { kind: "text", text: "x" },
    ]);
  });

  it("shows control keys and Enter in the preview", () => {
    expect(describeSteps([{ kind: "text", text: "a\x03b\r" }, { kind: "wait", ms: 1500 }])).toBe("a^Cb⏎\n\n[wait 1.5 s]");
  });
});
