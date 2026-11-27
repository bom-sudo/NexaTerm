// Platform detection from the WebView user agent (WebView2 on Windows, WebKitGTK on Linux).
const ua = typeof navigator !== "undefined" ? navigator.userAgent : "";

export const isWindows = /Windows/i.test(ua);
export const isLinux = !isWindows && /Linux|X11/i.test(ua);

/** Where secrets are kept on this OS, for UI text. */
export const secretStore = isWindows ? "Windows Credential Manager" : "the system keyring";
/** Short form for tight UI text. */
export const secretStoreShort = isWindows ? "Credential Manager" : "the system keyring";
/** Example serial device name for this OS. */
export const defaultSerialPort = isWindows ? "COM1" : "/dev/ttyUSB0";
