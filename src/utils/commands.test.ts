import { describe, expect, it } from "vitest";
import { applyVariables, dangerReasons, formatBytes, parseVariables } from "./commands";

describe("snippet variables", () => {
  it("parses names and defaults, ignores go-template fields", () => {
    const cmd = 'ps aux | head -n {{count:10}} && systemctl status {{service}} {{service}} --format "{{.Names}}"';
    expect(parseVariables(cmd)).toEqual([
      { name: "count", defaultValue: "10" },
      { name: "service", defaultValue: "" },
    ]);
  });

  it("applies values with defaults", () => {
    expect(applyVariables("tail -n {{n:50}} {{file}}", { file: "/var/log/syslog" })).toBe("tail -n 50 /var/log/syslog");
    expect(applyVariables('docker ps --format "{{.Names}}"', {})).toBe('docker ps --format "{{.Names}}"');
  });
});

describe("dangerous command detection", () => {
  it.each([
    "rm -rf /var/www",
    "sudo rm -fr /tmp/x",
    "mkfs.ext4 /dev/sdb1",
    "dd if=/dev/zero of=/dev/sda bs=1M",
    "sudo reboot",
    "shutdown -h now",
    "DROP TABLE users;",
    "systemctl stop nginx",
    "Remove-Item C:\\data -Recurse -Force",
  ])("flags %s", (c) => expect(dangerReasons(c).length).toBeGreaterThan(0));

  it.each(["ls -la", "df -h", "systemctl status nginx", "rm file.txt", "echo reboot-notes > x.txt", "cat /etc/hosts"])("allows %s", (c) =>
    expect(dangerReasons(c)).toEqual([]),
  );
});

describe("formatBytes", () => {
  it("formats", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024 * 1024)).toBe("5.0 GB");
  });
});
