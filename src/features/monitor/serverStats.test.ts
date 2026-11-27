import { describe, expect, it } from "vitest";
import { computeView, formatKb, formatRate, formatUptime, level, type RemoteStats } from "./serverStats";

const base: RemoteStats = {
  supported: true, os: "Linux", cpuTotal: 1000, cpuIdle: 800, cpus: 4,
  memTotalKb: 8 * 1024 * 1024, memAvailableKb: 5 * 1024 * 1024, swapTotalKb: 0, swapFreeKb: 0,
  diskTotalKb: 50 * 1024 * 1024, diskUsedKb: 10 * 1024 * 1024, uptimeSecs: 90061, load1: 1.1, rxBytes: 1000, txBytes: 500,
};

describe("server monitor maths", () => {
  it("needs two samples for CPU and network", () => {
    const v = computeView(null, base, 0);
    expect(v.cpuPct).toBeNull();
    expect(v.rxPerSec).toBeNull();
    expect(Math.round(v.memPct)).toBe(38);
    expect(Math.round(v.diskPct)).toBe(20);
  });
  it("computes CPU usage and rates from the difference", () => {
    const next = { ...base, cpuTotal: 1400, cpuIdle: 1100, rxBytes: 1000 + 3 * 2048, txBytes: 500 + 3 * 100 };
    const v = computeView(base, next, 3000);
    expect(v.cpuPct).toBe(25); // 100 busy of 400
    expect(v.rxPerSec).toBe(2048);
    expect(v.txPerSec).toBe(100);
  });
  it("shows no rate when counters reset", () => {
    expect(computeView(base, { ...base, rxBytes: 10 }, 3000).rxPerSec).toBeNull();
  });
  it("formats values", () => {
    expect(formatKb(5 * 1024 * 1024)).toBe("5.0 GB");
    expect(formatKb(512 * 1024)).toBe("512 MB");
    expect(formatRate(1536)).toBe("2 KB/s");
    expect(formatRate(3 * 1024 * 1024)).toBe("3.0 MB/s");
    expect(formatUptime(90061)).toBe("1d 1h");
    expect(formatUptime(3725)).toBe("1h 2m");
    expect([level(50), level(80), level(95)]).toEqual(["ok", "warn", "err"]);
  });
});
