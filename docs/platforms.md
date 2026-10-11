# Platform Work

Progred has native macOS and Linux shells plus a browser shell. They enter the
same Rust application rather than reimplementing the editor. An iPad
feasibility host was removed; see [its notes](history/ipad-host.md).

## Browser

The browser build requires HTTPS and cross-origin isolation for shared-memory
workers; plain LAN HTTP is no longer sufficient. See [the browser host](../web/README.md).
