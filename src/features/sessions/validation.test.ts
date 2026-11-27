import { describe, expect, it } from "vitest";
import { fieldForServerError, validateFields } from "./validation";
import type { SessionInput } from "../../types";

const ssh = (over: Partial<SessionInput> = {}): SessionInput =>
  ({ name: "", protocol: "ssh", host: "10.0.0.5", port: 22, username: "root", authMethod: "password", keyPath: null, tags: [], options: {}, ...over }) as unknown as SessionInput;

describe("session field validation", () => {
  it("accepts a complete SSH session", () => {
    expect(validateFields(ssh())).toEqual({});
  });
  it("flags an empty host with an example of what to type", () => {
    expect(validateFields(ssh({ host: "" })).host).toMatch(/192\.168\.1\.10/);
    expect(validateFields(ssh({ host: "   " })).host).toBeTruthy();
  });
  it("explains user@host typed into the host box", () => {
    expect(validateFields(ssh({ host: "root@10.0.0.5" })).host).toMatch(/Username/);
  });
  it("needs a username and a key file for key login", () => {
    expect(validateFields(ssh({ username: "" })).username).toBeTruthy();
    expect(validateFields(ssh({ authMethod: "key", keyPath: "" } as Partial<SessionInput>)).keyPath).toBeTruthy();
  });
  it("checks the port range", () => {
    expect(validateFields(ssh({ port: 70000 })).port).toBeTruthy();
  });
  it("ignores host for local terminals", () => {
    expect(validateFields(ssh({ protocol: "local", host: "" } as Partial<SessionInput>))).toEqual({});
  });
  it("maps backend messages to fields", () => {
    expect(fieldForServerError("Host must be 1-253 characters")).toBe("host");
    expect(fieldForServerError("Username is required for SSH")).toBe("username");
    expect(fieldForServerError("Session name is required")).toBeNull();
  });
});
