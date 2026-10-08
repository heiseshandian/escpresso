# macOS application and DMG

Install Rust and Xcode Command Line Tools, then run:

```sh
bash scripts/package-macos.sh
```

The script also detects a project-local Rust installation under `.build-tools/`
when Cargo is unavailable on PATH. The local installation and build outputs are
excluded from Git.

This builds the Apple Silicon version and creates
`dist/escpresso-0.1.3-macos-arm64.dmg`. Open the image and drag the application
into Applications. Rust is needed only to build; the packaged app runs by itself.

For an Intel build, install the Rust target first:

```sh
rustup target add x86_64-apple-darwin
bash scripts/package-macos.sh x86_64-apple-darwin
```

Each image contains a native application, an Applications shortcut, the MIT
license, and installation notes recording the source commit. The application
uses ad-hoc signing. Distributing it without Gatekeeper prompts requires an Apple
Developer ID certificate and Apple notarization, which this script does not perform.

The upstream server listens on all network interfaces at TCP port 9100. Only one
instance can use that port at a time.
