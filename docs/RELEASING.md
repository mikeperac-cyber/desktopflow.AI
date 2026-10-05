# Releasing DeskFlow AI

## Direct Windows download

Run the full release build on Windows:

```powershell
npm run tauri build
```

The build creates two x64 installers:

- `src-tauri\target\release\bundle\nsis\DeskFlow AI_0.1.0_x64-setup.exe` — recommended per-user setup
- `src-tauri\target\release\bundle\msi\DeskFlow AI_0.1.0_x64_en-US.msi` — managed deployment option

Record each SHA-256 hash and upload both files to a GitHub release whose tag matches the application version. Keep `src-tauri\target`, `dist`, and `node_modules` out of Git; they are build outputs, not release source.

These direct-download packages are unsigned until a trusted Windows code-signing identity is configured. SmartScreen warnings are therefore expected for early releases. Do not describe a GitHub release as Microsoft Store distribution.

## Microsoft Store path

Microsoft Store publication requires a Partner Center developer account, a reserved product name, store listing metadata, package submission, and certification. The Store can host an EXE/MSI submission, but a Store submission has its own package and signing requirements; MSIX is the recommended Store format. Complete that submission separately after the downloadable release has been validated.
